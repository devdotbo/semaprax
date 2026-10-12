//! Genuine owning cache publication boundary for the exact cold source view.
//! Cached full-clone forecasts remain unchanged. Retained authored declarations
//! are still copied once here; this does not claim a shared AST cache owner.
use super::reserve_builder_structure;
use crate::ast::program_source::SyntheticProgramView;
use crate::ast::Program;
use crate::diagnostic::Diagnostic;
// The sibling test module reaches these parent carriers through this module.
#[cfg(test)]
use super::{construct, private_layout, synthetic_program_with_registry, Input};
#[cfg(test)]
use crate::ast::Span;
#[cfg(test)]
use crate::hir;

pub(super) fn materialize(view: SyntheticProgramView<'_>) -> Result<Program, Vec<Diagnostic>> {
    reserve_builder_structure(0)?;
    let (authored, generated_types, generated) = view.into_overlay_parts();
    // Existing complete cached construction precharge retains the original
    // owning AST/function backing and generated-main allowance. This operation
    // occurs after the resolver receipt, before the owning cache key publishes.
    let imported = !authored.module_uses.is_empty();
    let (functions, types) = if imported {
        (append_copies(&authored.functions, generated)?,
            append_copies(&authored.types, generated_types)?)
    } else {
        // Preserve the separately qualified leaf receiver.
        let mut functions = authored.functions.clone();
        functions.extend(generated);
        (functions, authored.types.clone())
    };
    Ok(Program {
        path: authored.path.clone(),
        module: authored.module.clone(),
        module_uses: Vec::new(),
        permits: authored.permits.clone(),
        types,
        interfaces: authored.interfaces.clone(),
        protocols: authored.protocols.clone(),
        implementations: Vec::new(),
        session_protocols: authored.session_protocols.clone(),
        agents: authored.agents.clone(),
        functions,
    })
}

// Owning publication copies authored payloads under its existing complete cache
// precharge. A real new combined directory is prepaid in full while generated
// backing remains live; generated declarations themselves are moved once.
fn append_copies<T: Clone>(authored: &[T], generated: Vec<T>) -> Result<Vec<T>, Vec<Diagnostic>> {
    if authored.is_empty() { return Ok(generated); }
    if generated.is_empty() { return Ok(authored.to_vec()); }
    let count = authored.len().checked_add(generated.len())
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    let bytes = count.checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    reserve_builder_structure(bytes)?;
    let mut combined = Vec::with_capacity(count);
    if combined.capacity() != count {
        return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
    }
    combined.extend_from_slice(authored);
    combined.extend(generated);
    Ok(combined)
}

#[cfg(test)]
#[path = "checked_key/tests.rs"]
mod tests;
