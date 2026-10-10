//! Finite lexical-control certificates. These carry proof data, not authority.
use super::{BlockId, CleanupPlan, CleanupRegionId, ExitTarget, ExitTargetId};
use crate::ast::LexicalExitKind;
use crate::cleanup::LivenessFlagId;
use crate::hir::{ControlTarget, ExpressionId, LoopLatchKind};

pub const SCHEMA: &str = "semaprax.lexical-control-flow.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopCertificate {
    pub target: ControlTarget,
    pub condition: ExpressionId,
    pub body: ExpressionId,
    pub latch: LoopLatchKind,
    pub region: CleanupRegionId,
    /// Concrete condition evaluation entry for physical structured execution.
    pub condition_entry: BlockId,
    /// Generated advancement entry; absent for an ordinary While condition.
    pub latch_entry: Option<BlockId>,
    /// Normal completed iteration; distinct from an authored Continue exit.
    pub iteration_end: ExitTargetId,
    pub after: BlockId,
    /// Structural entry inventory. Replay additionally checks its full private
    /// ownership state, including conditional cases and reserved/staged epochs.
    pub entry_order: Vec<LivenessFlagId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexicalExitCertificate {
    pub site: ExpressionId,
    pub target: ControlTarget,
    pub kind: LexicalExitKind,
    pub exit: ExitTargetId,
    /// Exact finite generated-advance or postcondition proof entry.
    pub resume: Option<BlockId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResumeCertificate {
    pub entry: BlockId,
    /// Exact structural block inventory of this finite proof fragment. Replay
    /// reconstructs membership; backends never union or repair it.
    pub blocks: Vec<BlockId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexicalControlPlan {
    pub schema: &'static str,
    pub loops: Vec<LoopCertificate>,
    pub exits: Vec<LexicalExitCertificate>,
    pub resumes: Vec<ResumeCertificate>,
}

impl CleanupPlan {
    pub fn lexical_resume(&self, entry: BlockId) -> Option<&ResumeCertificate> {
        let control = self.lexical_control.as_deref()?;
        if self.schema != super::CLEANUP_PLAN_SCHEMA_V18 || control.schema != SCHEMA {
            return None;
        }
        let mut matches = control.resumes.iter().filter(|item| item.entry == entry);
        let certificate = matches.next()?;
        if matches.next().is_some() || !certificate.blocks.contains(&entry) {
            return None;
        }
        Some(certificate)
    }

    /// Filter producer transitions by their independently validated finite
    /// proof domain. Ordinary emission excludes all duplicated resume bodies.
    pub fn lexical_block_in_context(&self, entry: Option<BlockId>, block: BlockId) -> bool {
        if let Some(entry) = entry {
            return self.lexical_resume(entry).is_some_and(|resume| resume.blocks.contains(&block));
        }
        !self.lexical_control.as_deref().is_some_and(|control|
            control.resumes.iter().any(|resume| resume.blocks.contains(&block)))
    }

    pub fn lexical_loop(&self, condition: &ExpressionId) -> Option<&LoopCertificate> {
        let control = self.lexical_control.as_deref()?;
        if self.schema != super::CLEANUP_PLAN_SCHEMA_V18 || control.schema != SCHEMA {
            return None;
        }
        let mut matches = control.loops.iter().filter(|item| &item.condition == condition);
        let certificate = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        match &certificate.target {
            ControlTarget::Loop { site, .. } if site == condition => Some(certificate),
            _ => None,
        }
    }

    /// Exact lookup after ordinary HIR/plan validation. A duplicate site or
    /// noncanonical exit index fails closed even for an unvalidated caller.
    pub fn lexical_exit(&self, site: &ExpressionId) -> Option<(&LexicalExitCertificate, &ExitTarget)> {
        let control = self.lexical_control.as_deref()?;
        if self.schema != super::CLEANUP_PLAN_SCHEMA_V18 || control.schema != SCHEMA {
            return None;
        }
        let mut matches = control.exits.iter().filter(|exit| &exit.site == site);
        let certificate = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        let exit = self.exits.get(certificate.exit.0 as usize)?;
        (exit.id == certificate.exit).then_some((certificate, exit))
    }
}
