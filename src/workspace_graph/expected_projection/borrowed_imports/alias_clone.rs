//! Exact alias-aware clones of imported declarations. The original name is
//! never cloned; the alias is paid last, where the old overwrite paid it.
use super::import_stub;
use crate::ast::{Function, TypeDeclaration};

/// Clone every payload but the name. The caller installs the paid alias after
/// directory admission, preserving the original first-refusal ordering.
pub(super) fn type_payload(original: &TypeDeclaration) -> TypeDeclaration {
    let stable_id = original.stable_id.clone();
    let type_parameters = original.type_parameters.clone();
    let kind = original.kind.clone();
    let extends = original.extends.clone();
    let invariants = original.invariants.clone();
    TypeDeclaration {
        stable_id,
        explicit_id: original.explicit_id,
        name: String::new(),
        name_span: original.name_span,
        type_parameters,
        kind,
        extends,
        invariants,
        span: original.span,
    }
}

/// Retained wrapper: a complete function clone with the alias as its name.
pub(super) fn retained_function(original: &Function, alias: &str) -> Function {
    let stable_id = original.stable_id.clone();
    let type_parameters = original.type_parameters.clone();
    let params = original.params.clone();
    let return_type = original.return_type.clone();
    let effects = original.effects.clone();
    let yields = original.yields.clone();
    let follows = original.follows.clone();
    let requires = original.requires.clone();
    let ensures = original.ensures.clone();
    let body = original.body.clone();
    let name = crate::bounded_output::budgeted_clone(alias);
    Function {
        stable_id,
        explicit_id: original.explicit_id,
        name,
        name_span: original.name_span,
        type_parameters,
        params,
        return_type,
        effects,
        yields,
        follows,
        requires,
        ensures,
        body,
        span: original.span,
    }
}

/// Ordinary import signature (drops contracts and body) named by the alias.
pub(super) fn signature_function(original: &Function, alias: &str) -> Function {
    import_stub::named_signature(original, alias)
}
