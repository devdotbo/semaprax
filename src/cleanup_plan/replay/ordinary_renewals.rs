//! Independent replay directory queries spend the existing ReplayBudget.
use super::*;
use crate::cleanup_plan::renewal_profile::ordinary::{Directory, Family};
pub(super) fn lookup<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    at: &ExpressionId,
    index: Option<&crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    budget: &mut ReplayBudget,
) -> Result<Option<(&'a ResolvedBinding, u8)>, Diagnostic> {
    if let Some(sites) = program.primitive_renewals(function) {
        return Ok(sites.get(at).map(|binding| {
            (
                binding,
                if sites.is_helper(at) {
                    1 << (Family::Helper as u8)
                } else {
                    0
                },
            )
        }));
    }
    let Some(index) = index else {
        // Standalone legacy fixtures remain an independent old-route oracle;
        // selected primitive contexts were handled by their actual receiver.
        let mut families = 0;
        for (family, present) in [
            (
                Family::Vec,
                crate::hir::vec_loop_renewal::binding_in(program, function, at).is_some(),
            ),
            (
                Family::String,
                crate::string_ops::replacement::binding(function, at).is_some(),
            ),
            (
                Family::ByteSet,
                crate::byte_ops::same_owner_set_binding(function, at).is_some(),
            ),
            (
                Family::Helper,
                crate::cleanup_plan::renewal_profile::helper_binding_with_index(
                    program, function, at, None,
                )
                .is_some(),
            ),
        ] {
            if present {
                families |= 1 << (family as u8);
            }
        }
        return Ok(crate::cleanup_plan::renewal_profile::binding_with_index(
            program, function, at, None,
        )
        .map(|binding| (binding, families)));
    };
    let full = program.full_program().ok_or_else(|| {
        replay_error(function, "ordinary renewal index has no exact full program")
    })?;
    let mut first = None;
    let result = index.ordinary_at_with_work(full, function, at, &mut |units| match budget.charge(
        function,
        units,
        "ordinary renewal directory query",
    ) {
        Ok(()) => true,
        Err(error) => {
            first = Some(error);
            false
        }
    });
    match result {
        Ok(value) => Ok(value),
        Err(_) => Err(first.unwrap_or_else(|| {
            replay_error(function, "ordinary renewal directory receiver refused")
        })),
    }
}
pub(super) fn has(families: u8, family: Family) -> bool {
    Directory::contains(families, family)
}

pub(super) fn whole(
    function: &ResolvedFunction,
    place: &CleanupPlace,
    binding: &ResolvedBinding,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    budget.charge(function, 1, "ordinary renewal destination shape")?;
    let StorageId::Value(value) = &place.storage else {
        return Ok(false);
    };
    let units = value
        .as_str()
        .len()
        .min(binding.id.as_str().len())
        .checked_add(2)
        .ok_or_else(|| {
            replay_error(
                function,
                "ordinary renewal destination comparison overflowed",
            )
        })?;
    budget.charge(function, units, "ordinary renewal destination comparison")?;
    Ok(value == &binding.id && place.projections.is_empty())
}
