//! Defining-source flat-record facts for explicit type imports. Demand selects
//! evidence only; the receiving context authenticates current declarations.
use super::*;
use primitive::{body_digest, records::Records};

pub(in crate::workspace_graph) struct Inventory {
    pub(in crate::workspace_graph) records: Box<Records>,
    revision: [u8; 32],
    pub(in crate::workspace_graph) capture_work: usize,
}
impl Inventory {
    pub(in crate::workspace_graph) fn revision(&self) -> [u8; 32] {
        self.revision
    }
    pub(in crate::workspace_graph) fn owned_capacity_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>().checked_add(self.records.owned_capacity_bytes()?)
    }
}
fn debit(work: &mut usize, n: usize) -> Result<(), Vec<Diagnostic>> {
    *work = work.checked_sub(n).ok_or_else(fail)?;
    Ok(())
}
fn equal(work: &mut usize, a: &str, b: &str) -> Result<bool, Vec<Diagnostic>> {
    debit(work, 1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.bytes().zip(b.bytes()) {
        debit(work, 1)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
pub(in crate::workspace_graph) fn capture(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    sources: Option<&[WorkspaceSource]>,
) -> Result<Option<Box<Inventory>>, Vec<Diagnostic>> {
    let Some(sources) = sources else {
        return Ok(None);
    };
    if program.types.is_empty() {
        return Ok(None);
    }
    let mut work = WORK;
    let mut demanded = false;
    for caller in programs {
        debit(&mut work, 1)?;
        for import in &caller.module_uses {
            debit(&mut work, 1)?;
            if import.kind == crate::ast::ModuleUseKind::Type
                && equal(&mut work, &import.target_module, &program.module)?
            {
                for source in &program.types {
                    if equal(&mut work, &import.persistent_id, &source.stable_id)? {
                        demanded = true;
                    }
                }
            }
        }
    }
    if !demanded || !primitive::records::preflight(program, resolved, &mut work)? {
        return Ok(None);
    }
    // Registry entries must refer to this exact held authored declaration;
    // an import projection or same-signature resolved stub is not a producer.
    for source in &program.types {
        let mut owner = None;
        for (id, candidate) in authored {
            if equal(&mut work, id, &source.stable_id)? {
                owner = Some(candidate);
                break;
            }
        }
        let owner = owner.ok_or_else(fail)?;
        if !equal(&mut work, owner.path, &program.path)?
            || !equal(&mut work, owner.module, &program.module)?
            || !owner.ty.is_some_and(|ty| std::ptr::eq(ty, source))
            || owner.explicit != source.explicit_id
        {
            return Err(fail());
        }
    }
    let mut held_source = None;
    for source in sources {
        if equal(&mut work, &source.path, &program.path)? && held_source.replace(source).is_some() {
            return Err(fail());
        }
    }
    let held_source = held_source.ok_or_else(fail)?;
    reserve_builder_structure(body_digest::scratch_bytes())?;
    debit(
        &mut work,
        held_source
            .source
            .len()
            .checked_add(program.path.len())
            .and_then(|n| n.checked_add(program.module.len()))
            .and_then(|n| n.checked_add(24 + b"semaprax.retained-primitive-source.v1\0".len()))
            .ok_or_else(fail)?,
    )?;
    let revision = body_digest::source(&program.path, &program.module, &held_source.source);
    let records = primitive::records::capture(program, resolved, &mut work)?.ok_or_else(fail)?;
    reserve_builder_structure(std::mem::size_of::<Inventory>())?;
    Ok(Some(Box::new(Inventory {
        records,
        revision,
        capture_work: WORK.checked_sub(work).ok_or_else(fail)?,
    })))
}
