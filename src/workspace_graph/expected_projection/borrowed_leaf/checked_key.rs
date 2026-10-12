//! Genuine owning cache publication boundary for the exact cold source view.
//! Cached full-clone forecasts remain unchanged. Retained authored declarations
//! are still copied once here; this does not claim a shared AST cache owner.
use super::*;

pub(super) fn materialize(view: SyntheticProgramView<'_>) -> Result<Program, Vec<Diagnostic>> {
    reserve_builder_structure(0)?;
    let (authored, generated) = view.into_parts();
    // Existing complete cached construction precharge retains the original
    // owning AST/function backing and generated-main allowance. This operation
    // occurs after the resolver receipt, before the owning cache key publishes.
    let mut functions = authored.functions.clone();
    functions.extend(generated);
    Ok(Program {
        path: authored.path.clone(),
        module: authored.module.clone(),
        module_uses: Vec::new(),
        permits: authored.permits.clone(),
        types: authored.types.clone(),
        interfaces: authored.interfaces.clone(),
        protocols: authored.protocols.clone(),
        implementations: Vec::new(),
        session_protocols: authored.session_protocols.clone(),
        agents: authored.agents.clone(),
        functions,
    })
}

#[cfg(test)]
#[path = "checked_key/tests.rs"]
mod tests;
