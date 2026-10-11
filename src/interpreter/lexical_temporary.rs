//! Actual completed borrowed String operands remain owned until canonical exit.
//! Owned transfers keep their ordinary binding/callee carrier; only borrowed
//! consumers leave a live temporary, including normally scoped Call results.
use super::*;
use crate::cleanup_plan::{ExitContinuation, StorageId};

pub(super) struct CompletedString {
    pub(super) expression: hir::ExpressionId,
    pub(super) flag: crate::cleanup::LivenessFlagId,
    pub(super) epoch: usize,
    pub(super) value: Value,
}
pub(super) fn inventory(function: &ResolvedFunction) -> Result<Vec<CompletedString>, Flow> {
    let count = function
        .cleanup_plan
        .slots
        .iter()
        .filter(|slot| {
            slot.ty == ResolvedType::String && matches!(slot.storage, StorageId::Temporary(_))
        })
        .count();
    let bytes = std::alloc::Layout::array::<CompletedString>(count)
        .map_err(|_| Flow::Guard("completed String inventory size overflow"))?
        .size();
    if !crate::bounded_output::reserve_active_required(bytes) {
        return Err(Flow::Guard(
            "completed String inventory exceeds the active allocation budget",
        ));
    }
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(count)
        .map_err(|_| Flow::Guard("completed String inventory allocation failed"))?;
    // No subsequent growth: each independently validated temporary has one slot.
    if slots.capacity() != count {
        return Err(Flow::Guard("completed String inventory capacity differs"));
    }
    for slot in &function.cleanup_plan.slots {
        if slot.ty != ResolvedType::String {
            continue;
        }
        let StorageId::Temporary(expression) = &slot.storage else {
            continue;
        };
        let crate::cleanup::FieldLivenessShape::Leaf { flag, .. } = &slot.field_liveness_shape
        else {
            return Err(Flow::Guard("String temporary has no exact leaf flag"));
        };
        slots.push(CompletedString {
            expression: expression.clone(),
            flag: *flag,
            epoch: 0,
            value: Value::Moved,
        });
    }
    Ok(slots)
}
pub(super) fn checkpoint(environment: &Environment) -> usize {
    environment
        .control
        .as_deref()
        .map_or(0, |control| control.completed_epoch)
}
pub(super) fn truncate(environment: &mut Environment, checkpoint: usize) {
    if let Some(control) = environment.control.as_deref_mut() {
        for slot in &mut control.completed_strings {
            if slot.epoch > checkpoint {
                slot.value = Value::Moved;
            }
        }
    }
}
pub(super) fn finalize(
    environment: &mut Environment,
    expression: &hir::ExpressionId,
    flag: crate::cleanup::LivenessFlagId,
) -> Result<(), Flow> {
    let slot = environment
        .control
        .as_deref_mut()
        .and_then(|control| {
            control
                .completed_strings
                .iter_mut()
                .find(|slot| &slot.expression == expression && slot.flag == flag)
        })
        .ok_or(Flow::Guard(
            "completed String temporary has no authenticated flag",
        ))?;
    if !matches!(slot.value, Value::String(_) | Value::Moved) {
        return Err(Flow::Guard("completed String carrier differs"));
    }
    slot.value = Value::Moved;
    Ok(())
}
impl Evaluator<'_> {
    pub(super) fn retain_completed_string_arguments(
        &self,
        operation: crate::string_ops::StringOp,
        arguments: &[ResolvedExpr],
        values: &mut [Value],
        environment: &mut Environment,
    ) -> Result<(), Flow> {
        if environment.control.is_none() {
            return Ok(());
        }
        for (index, (argument, value)) in arguments.iter().zip(values).enumerate() {
            if operation.param_ownership(index) != hir::OwnershipMode::Borrow {
                continue;
            }
            self.retain_completed_string_value(argument, value, environment)?;
        }
        Ok(())
    }
    fn retain_completed_string_value(
        &self,
        argument: &ResolvedExpr,
        value: &mut Value,
        environment: &mut Environment,
    ) -> Result<(), Flow> {
        if environment.control.is_none()
            || argument.ty != ResolvedType::String
            || argument.ownership != hir::OwnershipMode::Own
        {
            return Ok(());
        }
        let function = self.lexical_function(environment)?;
        let storage = StorageId::Temporary(argument.id.clone());
        if !function
            .cleanup_plan
            .slots
            .iter()
            .any(|slot| slot.storage == storage && slot.ty == ResolvedType::String)
            || !matches!(value, Value::String(_))
        {
            return Err(Flow::Guard(
                "completed String operand has no exact temporary",
            ));
        }
        let control = environment.control.as_deref_mut().unwrap();
        let slot = control
            .completed_strings
            .iter_mut()
            .find(|slot| slot.expression == argument.id)
            .ok_or(Flow::Guard(
                "completed String temporary inventory is absent",
            ))?;
        if !matches!(slot.value, Value::Moved) {
            return Err(Flow::Guard("completed String temporary is already live"));
        }
        control.completed_epoch = control
            .completed_epoch
            .checked_add(1)
            .ok_or(Flow::Guard("completed String generation overflowed"))?;
        slot.epoch = control.completed_epoch;
        slot.value = std::mem::replace(value, Value::Moved);
        Ok(())
    }

    pub(super) fn combine_completed(
        &self,
        expression: &ResolvedExpr,
        lhs: Value,
        rhs: Value,
        environment: &mut Environment,
    ) -> Result<Value, Flow> {
        let ResolvedExprKind::Binary {
            op, left, right, ..
        } = &expression.kind
        else {
            return Err(Flow::Guard("completed comparison is not binary"));
        };
        if environment.control.is_none()
            || left.ty != ResolvedType::String
            || right.ty != ResolvedType::String
        {
            return match combine(*op, lhs, rhs) {
                Some(Ok(value)) => Ok(value),
                Some(Err(status)) => Err(Flow::Failure(status)),
                None => Err(Flow::Guard("ill-typed operands")),
            };
        }
        let (mut lhs, mut rhs) = (lhs, rhs);
        let (Value::String(a), Value::String(b)) = (&lhs, &rhs) else {
            return Err(Flow::Guard("ill-typed String comparison"));
        };
        let result = match op {
            BinaryOp::Eq => a == b,
            BinaryOp::Ne => a != b,
            BinaryOp::Lt => a.as_bytes() < b.as_bytes(),
            BinaryOp::Le => a.as_bytes() <= b.as_bytes(),
            BinaryOp::Gt => a.as_bytes() > b.as_bytes(),
            BinaryOp::Ge => a.as_bytes() >= b.as_bytes(),
            _ => return Err(Flow::Guard("unsupported String comparison")),
        };
        self.retain_completed_string_value(left, &mut lhs, environment)?;
        self.retain_completed_string_value(right, &mut rhs, environment)?;
        Ok(Value::Bool(result))
    }

    pub(super) fn settle_completed_string_scope(
        &self,
        environment: &mut Environment,
        checkpoint: usize,
    ) -> Result<(), Flow> {
        if checkpoint == super::lexical_temporary::checkpoint(environment) {
            return Ok(());
        }
        let function = self.lexical_function(environment)?;
        loop {
            let control = environment.control.as_deref().unwrap();
            let Some(slot) = control
                .completed_strings
                .iter()
                .find(|slot| slot.epoch > checkpoint && !matches!(slot.value, Value::Moved))
            else {
                return Ok(());
            };
            let storage = StorageId::Temporary(slot.expression.clone());
            let mut regions = function
                .cleanup_plan
                .regions
                .iter()
                .filter(|region| region.slots.contains(&storage));
            let region = regions
                .next()
                .ok_or(Flow::Guard("completed String scope is absent"))?;
            if regions.next().is_some() {
                return Err(Flow::Guard("completed String scope is ambiguous"));
            }
            // Root temporaries are settled with the function result, not by
            // syntactic block completion. Nested return operands cannot grant
            // permission to finalize a root temporary early.
            if region.parent.is_none() {
                return Err(Flow::Guard("nested String completion names root storage"));
            }
            let mut exits = function.cleanup_plan.exits.iter().filter(|exit| {
                function
                    .cleanup_plan
                    .lexical_block_in_context(control.resume, exit.from)
                    && exit.leaves_regions.as_slice() == [region.id]
                    && matches!(exit.continuation, ExitContinuation::Continue(_))
            });
            let exit = exits.next().ok_or(Flow::Guard(
                "completed String scope has no exact normal exit",
            ))?;
            if exits.next().is_some() {
                return Err(Flow::Guard("completed String normal exit is ambiguous"));
            }
            if !exit
                .finalize_in_order
                .iter()
                .any(|action| action.source.storage == storage)
            {
                return Err(Flow::Guard(
                    "completed String is absent from canonical scope finalizers",
                ));
            }
            super::lexical_control::settle_actions(environment, &exit.finalize_in_order, None)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_completed_string_finalizer_owns_the_actual_clone_and_refuses_foreign_storage() {
        let source = r#"module completed.string;
@id("run") fn run()->i64 ensures result==29 {
 let text="body";let mut index=0;
 while index<1 {index=index+1;if index==1 {return string_len(text)+25;}else{0}0}
 29
}
@id("main") fn main()->i64 {0}
"#;
        let program =
            crate::hir::resolve(&crate::check(source, "completed-string.spx").unwrap()).unwrap();
        crate::hir::validate(&program).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "run")
            .unwrap();
        let count = function
            .cleanup_plan
            .slots
            .iter()
            .filter(|slot| {
                slot.ty == ResolvedType::String && matches!(slot.storage, StorageId::Temporary(_))
            })
            .count();
        let bytes = std::mem::size_of::<super::super::lexical_control::FrameControl>()
            + std::mem::size_of::<CompletedString>() * count;
        let (exact, refused, used) = crate::bounded_output::with_limit_usage(bytes, || {
            super::super::lexical_control::frame_control(function)
        });
        assert!(exact.is_ok());
        assert!(!refused);
        assert_eq!(used, bytes);
        let (short, refused, _) = crate::bounded_output::with_limit_usage(bytes - 1, || {
            super::super::lexical_control::frame_control(function)
        });
        assert!(short.is_err());
        assert!(refused);
        let action = function
            .cleanup_plan
            .exits
            .iter()
            .filter(|exit| matches!(exit.continuation, ExitContinuation::FunctionReturn { .. }))
            .flat_map(|exit| &exit.finalize_in_order)
            .find(|action| matches!(action.source.storage, StorageId::Temporary(_)))
            .unwrap();
        let StorageId::Temporary(id) = &action.source.storage else {
            unreachable!()
        };
        let mut environment = Environment::from(Vec::new());
        environment.control = super::super::lexical_control::frame_control(function).unwrap();
        // An authenticated inactive flag is distinct from a missing carrier.
        super::super::lexical_control::settle_actions(
            &mut environment,
            std::slice::from_ref(action),
            None,
        )
        .unwrap();
        environment
            .control
            .as_deref_mut()
            .unwrap()
            .completed_strings
            .iter_mut()
            .find(|slot| &slot.expression == id)
            .unwrap()
            .value = Value::String("body".to_owned());
        let mut forged = action.clone();
        forged.guard_flag =
            crate::cleanup::LivenessFlagId(action.guard_flag.0.checked_add(1).unwrap());
        assert!(
            super::super::lexical_control::settle_actions(&mut environment, &[forged], None)
                .is_err()
        );
        let mut forged = action.clone();
        forged
            .source
            .projections
            .push(hir::DeclarationId::new("foreign.field"));
        assert!(
            super::super::lexical_control::settle_actions(&mut environment, &[forged], None)
                .is_err()
        );
        assert!(matches!(
            environment
                .control
                .as_deref()
                .unwrap()
                .completed_strings
                .iter()
                .find(|slot| &slot.expression == id)
                .unwrap()
                .value,
            Value::String(_)
        ));
        let mut forged = action.clone();
        forged.source.storage = StorageId::Temporary(function.body.id.clone());
        assert!(
            super::super::lexical_control::settle_actions(&mut environment, &[forged], None)
                .is_err()
        );
        super::super::lexical_control::settle_actions(
            &mut environment,
            std::slice::from_ref(action),
            None,
        )
        .unwrap();
        assert!(matches!(
            environment
                .control
                .as_deref()
                .unwrap()
                .completed_strings
                .iter()
                .find(|slot| &slot.expression == id)
                .unwrap()
                .value,
            Value::Moved
        ));
        truncate(&mut environment, 0);
        assert_eq!(checkpoint(&environment), 0);
    }
}

#[cfg(test)]
mod runtime_tests {
    #[test]
    fn taken_and_skipped_string_temporaries_settle_at_lexical_return_and_contract_refusal() {
        let source = r#"module completed.runtime;
@id("run") fn run(gate:bool,fail:bool)->i64 ensures result==29 && !fail {
 let text="body";let lazy=gate && string_len("unused")==6;let equal=text=="body";
 let scoped={string_len(string_concat("a","b"))};
 let mut index=0;
 while index<1 {index=index+1;if index==1 {return if scoped==2 && equal && lazy==gate {string_len(text)+25}else{0};}else{0}0}
 29
}
@id("app.main") fn main()->i64 {run(false,false)}
"#;
        let path = std::env::temp_dir().join(format!(
            "completed-string-runtime-{}.spx",
            std::process::id()
        ));
        for (call, returned) in [
            ("run(false,false)", true),
            ("run(true,false)", true),
            ("run(false,true)", false),
            ("run(true,true)", false),
        ] {
            std::fs::write(&path, source.replace("run(false,false)", call)).unwrap();
            let observed = crate::interpreter::internal_strings::interpret(
                &path,
                "app.main",
                &[],
                &Default::default(),
            )
            .unwrap();
            crate::interpreter::internal_strings::verify_envelope(&observed.envelope).unwrap();
            assert_eq!(observed.returned, returned, "{call}");
            let envelope: serde_json::Value = serde_json::from_str(&observed.envelope).unwrap();
            let outcome = &envelope["payload"]["outcome"];
            if returned {
                assert_eq!(outcome["value"], "29");
            } else {
                assert_eq!(outcome["status"]["domain_id"], "semaprax.contract.v1");
                assert_eq!(outcome["status"]["code"], 2);
            }
        }
        std::fs::remove_file(path).unwrap();
    }
}
