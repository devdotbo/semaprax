//! Internal whole-record pattern transfer; no public aggregate ABI admission.
use super::{
    DeclarationIndex, DeclarationKind, IdentityOrigin, OwnershipMode, ResolvedBinding, ResolvedType,
};

fn monomorphic_record(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments }
        if arguments.is_empty()
            && index.type_parameters(declaration).is_some_and(|p| p.is_empty())
            && index.declaration(declaration).is_some_and(|d|
                d.kind == DeclarationKind::Record && d.identity_origin == IdentityOrigin::Explicit))
}

/// Full inherited closure validation includes descendants hidden by the binding.
/// This is type admission only; current field/binding/path and loan checks remain
/// the independent responsibility of each pattern receiver.
pub(crate) fn admitted(
    index: &DeclarationIndex,
    root: &ResolvedType,
    child: &ResolvedType,
) -> bool {
    monomorphic_record(index, root)
        && monomorphic_record(index, child)
        && (super::is_admitted_nested_owned_byte_record(index, root)
            || super::owned_text_record::runtime_admitted(root, index))
        && (super::is_admitted_nested_owned_byte_record(index, child)
            || super::owned_text_record::runtime_admitted(child, index))
}

/// Discovery only, before the independent declared-field receiver authenticates
/// the type and binding. Compiler Vec and generic/resource cases stay separate.
pub(crate) fn candidate(binding: &ResolvedBinding) -> bool {
    binding.ownership == OwnershipMode::Own
        && matches!(&binding.ty, ResolvedType::Nominal { arguments, .. } if arguments.is_empty())
}

/// Frozen public profiles do not adopt the additive internal binding rule.
/// Already-validated patterns have depth <=64; excessive input still refuses.
pub(crate) fn pattern_requires_profile(pattern: &super::ResolvedMatchPattern) -> bool {
    let super::ResolvedMatchPattern::Record { fields, .. } = pattern else {
        return false;
    };
    let mut stack = [None; 64];
    let mut length = 1usize;
    stack[0] = Some((fields.as_slice(), 0usize));
    while length != 0 {
        let (fields, index) = stack[length - 1].expect("held pattern frame");
        let Some(field) = fields.get(index) else {
            length -= 1;
            continue;
        };
        stack[length - 1] = Some((fields, index + 1));
        match &field.pattern {
            super::ResolvedRecordMatchFieldPattern::Binding(binding) if candidate(binding) => {
                return true
            }
            super::ResolvedRecordMatchFieldPattern::Record { fields, .. } => {
                if length == stack.len() {
                    return true;
                }
                stack[length] = Some((fields.as_slice(), 0));
                length += 1;
            }
            _ => {}
        }
    }
    false
}
