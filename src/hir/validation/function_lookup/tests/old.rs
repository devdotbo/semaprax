//! Original source-order owned map constructor, saved independently.
use super::*;
pub(super) fn authenticate(
    program: &ResolvedProgram,
) -> Result<BTreeMap<DeclarationId, &ResolvedFunction>, Diagnostic> {
    let mut functions = BTreeMap::new();
    for function in &program.functions {
        if crate::host_io_ops::by_id(function.id.as_str()).is_some()
            || crate::command_io_ops::by_id(function.id.as_str()).is_some()
        {
            return Err(hir_error(format!(
                "resolved function `{}` aliases a compiler-owned host I/O operation",
                function.id
            )));
        }
        if functions.insert(function.id.clone(), function).is_some() {
            return Err(hir_error(format!(
                "duplicate resolved function identity `{}`",
                function.id
            )));
        }
        match program.declarations.declaration(&function.id) {
            Some(declaration)
                if declaration.kind == DeclarationKind::Function
                    && declaration.name == function.name => {}
            Some(_) => {
                return Err(hir_error(format!(
                    "resolved function `{}` disagrees with its declaration index entry",
                    function.id
                )));
            }
            None => {
                return Err(hir_error(format!(
                    "resolved function `{}` is absent from the declaration index",
                    function.id
                )));
            }
        }
    }
    Ok(functions)
}
