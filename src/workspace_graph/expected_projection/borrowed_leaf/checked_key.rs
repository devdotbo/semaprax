//! Genuine owning cache publication boundary for the exact cold source view.
//! Cached full-clone forecasts remain unchanged. Retained authored declarations
//! are still copied once here; this does not claim a shared AST cache owner.
use super::{active_builder_limit, limit_error, reserve_builder_structure};
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
    let (authored, mut roots) = view.into_overlay_owner();
    // Existing complete cached construction precharge retains the original
    // owning AST/function backing and generated-main allowance. This operation
    // occurs after the resolver receipt, before the owning cache key publishes.
    let imported = !authored.module_uses.is_empty();
    let (functions, types) = if imported {
        // Both real replacement directories are paid while recursive roots
        // remain under their iterative owner, including short/sticky refusal.
        let functions = combined_directory::<crate::ast::Function>(
            authored.functions.len(),
            roots.functions.len(),
        )?;
        let types = combined_directory::<crate::ast::TypeDeclaration>(
            authored.types.len(),
            roots.types.len(),
        )?;
        (
            append_prepaid(
                &authored.functions,
                std::mem::take(&mut roots.functions),
                functions,
            ),
            append_prepaid(&authored.types, std::mem::take(&mut roots.types), types),
        )
    } else {
        // Preserve the separately qualified leaf receiver.
        let mut functions = authored.functions.clone();
        functions.extend(std::mem::take(&mut roots.functions));
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
fn combined_directory<T>(
    authored: usize,
    generated: usize,
) -> Result<Option<Vec<T>>, Vec<Diagnostic>> {
    if authored == 0 || generated == 0 {
        return Ok(None);
    }
    let count = authored
        .checked_add(generated)
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    reserve_builder_structure(bytes)?;
    let directory = Vec::with_capacity(count);
    if directory.capacity() != count {
        return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
    }
    Ok(Some(directory))
}
fn append_prepaid<T: Clone>(
    authored: &[T],
    generated: Vec<T>,
    directory: Option<Vec<T>>,
) -> Vec<T> {
    if authored.is_empty() {
        return generated;
    }
    if generated.is_empty() {
        return authored.to_vec();
    }
    let mut combined = directory.expect("both source slices have a prepaid checked directory");
    combined.extend_from_slice(authored);
    combined.extend(generated);
    combined
}
#[cfg(test)]
fn append_copies<T: Clone>(authored: &[T], generated: Vec<T>) -> Result<Vec<T>, Vec<Diagnostic>> {
    let directory = combined_directory::<T>(authored.len(), generated.len())?;
    Ok(append_prepaid(authored, generated, directory))
}

#[cfg(test)]
#[path = "checked_key/tests.rs"]
mod tests;
