//! Exact whole-vector slot loans share the existing immutable owner backing.
use super::*;
impl Evaluator<'_> {
    pub(super) fn evaluate_string_slot(
        &mut self,
        expression: &ResolvedExpr,
        slot: crate::vec_ops::string_slot::Slot<'_>,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        self.charge()?;
        let ResolvedExprKind::Place(place) = &slot.source.kind else {
            unreachable!()
        };
        self.charge()?; // evaluating the named source precedes the index
        let Value::Vec(vector) = environment
            .get(&place.root)
            .ok_or(Flow::Guard("String-slot owner is unavailable"))?
        else {
            return Err(Flow::Guard("String-slot owner is not a live vector"));
        };
        if vector.element != ResolvedType::String {
            return Err(Flow::Guard("String-slot vector element mismatch"));
        }
        let vector = Arc::clone(vector);
        let Value::Usize(index) = self.evaluate(slot.index, environment, depth)? else {
            return Err(Flow::Guard("String-slot index is not usize"));
        };
        let index = usize::try_from(index)
            .ok()
            .filter(|i| *i < vector.values.len())
            .ok_or_else(|| {
                Flow::Failure(owned_vec::normalize_vec(
                    crate::vec_ops::GET_OUT_OF_BOUNDS_CODE,
                ))
            })?;
        let Value::String(text) = &vector.values[index] else {
            return Err(Flow::Guard(
                "String-slot payload is not live String storage",
            ));
        };
        if slot.bytes && text.len() as u64 > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES {
            return Err(Flow::Guard(
                "String-slot bytes exceed the existing internal slice bound",
            ));
        }
        let length = text.len();
        let generation = vector.generation;
        let root = BorrowedRoot::VectorStringSlot {
            root: place.root.clone(),
            producer: expression.id.clone(),
            index,
            generation,
        };
        let backing = Utf8Backing::VectorStringSlot {
            vector,
            index,
            generation,
        };
        environment.slot_views = true;
        Ok(if slot.bytes {
            Value::BorrowedSlice(BorrowedSliceValue {
                invocation_root: root,
                backing,
                start: 0,
                end: length,
            })
        } else {
            Value::BorrowedStr(BorrowedStrValue {
                invocation_root: root,
                bytes: backing,
            })
        })
    }
}

/// The shared internal Slice ceiling is independent of an owned Str's extent.
pub(super) fn validate_slice_extent(backing: &Utf8Backing) -> Result<(), Flow> {
    if matches!(backing, Utf8Backing::VectorStringSlot { .. })
        && backing.as_ref().len() as u64 > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES
    {
        return Err(Flow::Guard(
            "String-slot bytes exceed the existing internal slice bound",
        ));
    }
    Ok(())
}

impl Evaluator<'_> {
    pub(super) fn evaluate_slot_place(
        &mut self,
        expression: &ResolvedExpr,
        place: &hir::Place,
        environment: &mut Environment,
    ) -> Result<Value, Flow> {
        let moves_storage = expression.ownership == hir::OwnershipMode::Own
            && matches!(
                &expression.ty,
                ResolvedType::Bytes
                    | ResolvedType::StringMap
                    | ResolvedType::OnceFunction
                    | ResolvedType::OnceFunctionI64
                    | ResolvedType::OnceFunctionI64Pair
                    | ResolvedType::MutFunctionI64
                    | ResolvedType::Nominal { .. }
            );
        if moves_storage {
            self.release_dead_slot_views(expression, place, environment)?;
            take_owned_place(environment, place).ok_or(Flow::Guard("use of moved owned storage"))
        } else {
            self.lookup_place(environment, place)?
                .ok_or(Flow::Guard("unresolved scalar place"))
        }
    }

    /// The current independently validated loan plan is the only expiry authority.
    /// Releasing a dead reference is not an owning String/Vec finalizer.
    fn release_dead_slot_views(
        &self,
        expression: &ResolvedExpr,
        place: &hir::Place,
        environment: &mut Environment,
    ) -> Result<(), Flow> {
        if !environment.slot_views
            || !place.projections.is_empty()
            || !environment
                .bindings
                .iter()
                .any(|(_, value)| slot_origin(value).is_some_and(|(root, _)| root == &place.root))
        {
            return Ok(());
        }
        let address = environment
            .slot_function
            .ok_or(Flow::Guard("String-slot frame has no execution identity"))?;
        let function = match &self.admitted {
            FunctionLookup::Borrowed(functions) => functions
                .values()
                .copied()
                .find(|function| *function as *const ResolvedFunction as usize == address),
            FunctionLookup::Prepared {
                functions,
                function_instances,
                ..
            } => functions
                .iter()
                .chain(function_instances.iter().map(|instance| &instance.function))
                .find(|function| *function as *const ResolvedFunction as usize == address),
        }
        .or_else(|| {
            self.closure_functions
                .values()
                .find(|function| *function as *const ResolvedFunction as usize == address)
        })
        .ok_or(Flow::Guard("String-slot execution identity is not held"))?;
        let endpoint = function
            .loan_plan
            .endpoints
            .iter()
            .find(|endpoint| {
                endpoint.point.expression == expression.id
                    && endpoint.point.phase == crate::loan_plan::LoanPointPhase::Before
            })
            .ok_or(Flow::Guard(
                "String-slot transfer lacks an authenticated endpoint",
            ))?;
        // The union contains all predecessor-edge live loans. Include child and
        // alias loans, not merely the original producer interval.
        if function
            .loan_plan
            .loans
            .iter()
            .any(|loan| loan.origin.root == place.root && endpoint.live_before.contains(&loan.id))
        {
            return Err(Flow::Guard(
                "String-slot transfer has a live whole-vector loan",
            ));
        }
        for (_, value) in &environment.bindings {
            let Some((root, producer)) = slot_origin(value) else {
                continue;
            };
            if root != &place.root {
                continue;
            }
            if !function.loan_plan.loans.iter().any(|loan| {
                loan.site == *producer
                    && loan.origin.root == *root
                    && matches!(
                        loan.cause,
                        crate::loan_plan::LoanCause::StrView
                            | crate::loan_plan::LoanCause::SliceView
                    )
            }) {
                return Err(Flow::Guard(
                    "String-slot reference has no authenticated loan producer",
                ));
            }
        }
        for (_, value) in &mut environment.bindings {
            if slot_origin(value).is_some_and(|(root, _)| root == &place.root) {
                *value = Value::Moved;
            }
        }
        environment.slot_views = environment
            .bindings
            .iter()
            .any(|(_, value)| slot_origin(value).is_some());
        Ok(())
    }
}
fn slot_origin(value: &Value) -> Option<(&ValueId, &hir::ExpressionId)> {
    let root = match value {
        Value::BorrowedStr(view) => &view.invocation_root,
        Value::BorrowedSlice(view) => &view.invocation_root,
        _ => return None,
    };
    match root {
        BorrowedRoot::VectorStringSlot { root, producer, .. } => Some((root, producer)),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
