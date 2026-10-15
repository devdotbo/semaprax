//! Selective append-only cleanup cache projection; legacy bytes stay exact.
use super::{codec_enum, codec_struct, Codec, Decoder, Encoder, Result};
use crate::cleanup_plan::{
    CleanupPlan, ExecutionBoundaryCertificate, ExecutionBoundaryKind, LexicalControlPlan,
    LexicalExitCertificate, LoopCertificate, ResumeCertificate, CLEANUP_PLAN_SCHEMA_V18,
    CLEANUP_PLAN_SCHEMA_V19, CLEANUP_PLAN_SCHEMA_V20,
};

codec_struct!(LoopCertificate {
    target,
    condition,
    body,
    latch,
    region,
    condition_entry,
    latch_entry,
    iteration_end,
    after,
    entry_order
});
codec_struct!(LexicalExitCertificate {
    site,
    target,
    kind,
    exit,
    resume
});
codec_struct!(ResumeCertificate { entry, blocks });
codec_struct!(LexicalControlPlan {
    schema,
    loops,
    exits,
    resumes,
    abrupt_only_regions,
    function_success_exits,
    execution_boundaries
});

impl Codec for CleanupPlan {
    fn encode(&self, out: &mut Encoder) -> Result<()> {
        super::checked_allocation::validate(self)?;
        if !matches!(
            self.schema,
            CLEANUP_PLAN_SCHEMA_V19 | CLEANUP_PLAN_SCHEMA_V20
        ) && (self.schema == CLEANUP_PLAN_SCHEMA_V18) != self.lexical_control.is_some()
        {
            return Err(super::grammar(
                "cleanup lexical certificate/schema mismatch",
            ));
        }
        out.nested(|out| {
            self.schema.encode(out)?;
            self.entry.encode(out)?;
            self.entry_state.encode(out)?;
            self.slots.encode(out)?;
            self.status_sources.encode(out)?;
            self.blocks.encode(out)?;
            self.edges.encode(out)?;
            self.regions.encode(out)?;
            self.exits.encode(out)?;
            if matches!(
                self.schema,
                CLEANUP_PLAN_SCHEMA_V19 | CLEANUP_PLAN_SCHEMA_V20
            ) {
                // v19 composes helper renewal with optional lexical authority.
                // Earlier schemas retain their exact append-only encoding.
                self.lexical_control.encode(out)?;
            } else if let Some(control) = &self.lexical_control {
                control.encode(out)?;
            }
            Ok(())
        })
    }

    fn decode(input: &mut Decoder<'_>) -> Result<Self> {
        input.nested(|input| {
            let schema = Codec::decode(input)?;
            let entry = Codec::decode(input)?;
            let entry_state = Codec::decode(input)?;
            let slots = Codec::decode(input)?;
            let status_sources = Codec::decode(input)?;
            let blocks = Codec::decode(input)?;
            let edges = Codec::decode(input)?;
            let regions = Codec::decode(input)?;
            let exits = Codec::decode(input)?;
            let lexical_control =
                if matches!(schema, CLEANUP_PLAN_SCHEMA_V19 | CLEANUP_PLAN_SCHEMA_V20) {
                    Codec::decode(input)?
                } else if schema == CLEANUP_PLAN_SCHEMA_V18 {
                    Some(Codec::decode(input)?)
                } else {
                    None
                };
            let plan = Self {
                schema,
                entry,
                entry_state,
                slots,
                status_sources,
                blocks,
                edges,
                regions,
                exits,
                lexical_control,
            };
            super::checked_allocation::validate(&plan)?;
            Ok(plan)
        })
    }
}

codec_enum!(ExecutionBoundaryKind {0=>FunctionEntry,1=>LoopIteration});
codec_struct!(ExecutionBoundaryCertificate {
    kind,
    site,
    block,
    region,
    leaves_regions,
    finalize_in_order
});

#[cfg(test)]
mod byte_helper_tests;
