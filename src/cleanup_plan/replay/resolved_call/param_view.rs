//! Private type/ownership projection. Display names and IDs are not replay inputs.
use super::*;
use crate::hir::OwnershipMode;

pub(in crate::cleanup_plan::replay) enum Params<'a> {
    Borrowed(&'a [ResolvedParam]),
    Owned(Vec<ResolvedParam>),
    Byte(crate::byte_ops::ByteOp),
    String(crate::string_ops::StringOp),
    Str(crate::str_ops::StrOp),
}
#[derive(Clone, Copy)]
pub(in crate::cleanup_plan::replay) struct Param<'a> {
    pub(in crate::cleanup_plan::replay) ty: &'a ResolvedType,
    pub(in crate::cleanup_plan::replay) ownership: OwnershipMode,
}
impl<'a> From<&'a ResolvedParam> for Param<'a> {
    fn from(parameter: &'a ResolvedParam) -> Self {
        Self {
            ty: &parameter.ty,
            ownership: parameter.ownership,
        }
    }
}
impl<'a> From<Vec<ResolvedParam>> for Params<'a> {
    fn from(params: Vec<ResolvedParam>) -> Self {
        Self::Owned(params)
    }
}
impl Params<'_> {
    pub(in crate::cleanup_plan::replay) fn len(&self) -> usize {
        match self {
            Self::Borrowed(params) => params.len(),
            Self::Owned(params) => params.len(),
            Self::Byte(op) => op.param_types().len().min(op.param_names().len()),
            Self::String(op) => op.param_types().len().min(op.param_names().len()),
            Self::Str(op) => op.param_types().len().min(op.param_names().len()),
        }
    }
    pub(in crate::cleanup_plan::replay) fn get(&self, index: usize) -> Option<Param<'_>> {
        if index >= self.len() {
            return None;
        }
        Some(match self {
            Self::Borrowed(params) => (&params[index]).into(),
            Self::Owned(params) => (&params[index]).into(),
            Self::Byte(op) => Param {
                ty: &op.param_types()[index],
                ownership: op.param_ownership(index),
            },
            Self::String(op) => Param {
                ty: &op.param_types()[index],
                ownership: op.param_ownership(index),
            },
            Self::Str(op) => {
                let ty = &op.param_types()[index];
                Param {
                    ty,
                    ownership: if *ty == ResolvedType::Str {
                        OwnershipMode::Borrow
                    } else {
                        OwnershipMode::Value
                    },
                }
            }
        })
    }
    pub(in crate::cleanup_plan::replay) fn iter(&self) -> impl ExactSizeIterator<Item = Param<'_>> {
        (0..self.len()).map(|index| self.get(index).expect("parameter view ordinal"))
    }
}

/// Compare only replay ordinals, without materializing either ordinal vector.
/// A mismatch is retained until every parameter has completed the old fallible
/// classification pass: a later type error still precedes ordinal disagreement.
pub(in crate::cleanup_plan::replay) fn owned_parameter_indices_match(
    params: &Params<'_>,
    arguments: &[crate::cleanup_plan::CallArgumentTransfer],
    function: &ResolvedFunction,
    mut needs_drop: impl FnMut(&ResolvedType) -> Result<bool, Diagnostic>,
) -> Result<bool, Diagnostic> {
    #[cfg(test)]
    if SAVED_COMMIT_INDICES.with(std::cell::Cell::get) || !ordinary_borrows_enabled() {
        return saved_owned_parameter_indices_match(params, arguments, function, needs_drop);
    }
    let mut actual = arguments.iter();
    let mut equal = true;
    for (index, parameter) in params.iter().enumerate() {
        if parameter.ownership == OwnershipMode::Own && needs_drop(parameter.ty)? {
            let expected = u32::try_from(index)
                .map_err(|_| replay_error(function, "too many call parameters"))?;
            // Do not short circuit iteration or classification after mismatch.
            equal &= actual.next().map(|argument| argument.parameter_index) == Some(expected);
        }
    }
    Ok(equal && actual.next().is_none())
}

/// Complete original receiver, including both independent owned vectors.
#[cfg(test)]
fn saved_owned_parameter_indices_match(
    params: &Params<'_>,
    arguments: &[crate::cleanup_plan::CallArgumentTransfer],
    function: &ResolvedFunction,
    mut needs_drop: impl FnMut(&ResolvedType) -> Result<bool, Diagnostic>,
) -> Result<bool, Diagnostic> {
    let mut expected_parameters = Vec::new();
    for (index, parameter) in params.iter().enumerate() {
        if parameter.ownership == OwnershipMode::Own && needs_drop(parameter.ty)? {
            expected_parameters.push(
                u32::try_from(index)
                    .map_err(|_| replay_error(function, "too many call parameters"))?,
            );
        }
    }
    let actual_parameters = arguments
        .iter()
        .map(|argument| argument.parameter_index)
        .collect::<Vec<_>>();
    Ok(actual_parameters == expected_parameters)
}
#[cfg(test)]
thread_local! {
    static SAVED_COMMIT_INDICES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
fn with_saved_commit_indices<T>(operation: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            SAVED_COMMIT_INDICES.with(|cell| cell.set(self.0));
        }
    }
    let _reset = Reset(SAVED_COMMIT_INDICES.with(|cell| cell.replace(true)));
    operation()
}

#[cfg(test)]
thread_local! {
    static OWNED_ORACLE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub(in crate::cleanup_plan::replay) fn ordinary_borrows_enabled() -> bool {
    #[cfg(test)]
    if OWNED_ORACLE.with(std::cell::Cell::get) {
        return false;
    }
    true
}
#[cfg(test)]
fn with_owned_oracle<T>(operation: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            OWNED_ORACLE.with(|cell| cell.set(self.0));
        }
    }
    let _reset = Reset(OWNED_ORACLE.with(|cell| cell.replace(true)));
    operation()
}

/// Keep excluded providers on the complete legacy validation/construction route.
/// This predicate only identifies providers; it never reconstructs their params.
fn owned_provider(callee: &DeclarationId) -> bool {
    use crate::hir::closure::once;
    matches!(
        callee.as_str(),
        once::CONSTRUCT_ID
            | once::INVOKE_ID
            | once::DROP_ID
            | once::MIXED_CONSTRUCT_ID
            | once::MIXED_INVOKE_ID
            | once::MIXED_DROP_ID
            | once::PAIR_CONSTRUCT_ID
            | once::PAIR_INVOKE_ID
            | once::PAIR_DROP_ID
    ) || callee == &*crate::hir::function_value::INVOKE_ID
        || crate::map_ops::by_id(callee.as_str()).is_some()
        || callee.as_str() == crate::stdin_stream_ops::EOF_ID
        || crate::host_io_ops::by_id(callee.as_str()).is_some()
        || crate::command_io_ops::by_id(callee.as_str()).is_some()
        || crate::list_ops::by_id(callee.as_str()).is_some()
        || crate::iterator_ops::by_id(callee.as_str()).is_some()
        || crate::vec_ops::by_id(callee.as_str()).is_some()
        || crate::box_ops::by_id(callee.as_str()).is_some()
}
pub(in crate::cleanup_plan::replay) fn resolved_call_param_view<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    expression: &crate::hir::ResolvedExpr,
    callee: &DeclarationId,
    instance: Option<&FunctionInstanceId>,
    type_arguments: &[ResolvedType],
) -> Result<Params<'a>, Diagnostic> {
    if !ordinary_borrows_enabled() {
        return super::resolved_call_params(
            program,
            function,
            expression,
            callee,
            instance,
            type_arguments,
        )
        .map(Params::Owned);
    }
    if callee.as_str() == crate::vec_field::ID || callee.as_str() == crate::literal_format::ID {
        return super::resolved_call_params(
            program,
            function,
            expression,
            callee,
            instance,
            type_arguments,
        )
        .map(Params::Owned);
    }
    if instance.is_none() {
        if owned_provider(callee) {
            return super::resolved_call_params(
                program,
                function,
                expression,
                callee,
                instance,
                type_arguments,
            )
            .map(Params::Owned);
        }
        // As in the old resolver these closed signatures ignore type arguments.
        if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
            return Ok(Params::String(op));
        }
        if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
            return Ok(Params::Str(op));
        }
        if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
            return Ok(Params::Byte(op));
        }
        if program
            .interfaces()
            .iter()
            .flat_map(|interface| &interface.imports)
            .any(|import| import.native_rust && &import.id == callee)
        {
            return super::resolved_call_params(
                program,
                function,
                expression,
                callee,
                instance,
                type_arguments,
            )
            .map(Params::Owned);
        }
    }
    let target = program
        .resolve_call_target(callee, instance)
        .ok_or_else(|| {
            replay_error(
                function,
                format!("cleanup call has unknown callee `{callee}`"),
            )
        })?;
    Ok(Params::Borrowed(&target.params))
}

#[cfg(test)]
mod tests;
