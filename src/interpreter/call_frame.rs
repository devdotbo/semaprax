//! Function result staging and the single postcondition/publication epilogue.
use super::*;

impl Evaluator<'_> {
    fn call_frame_inner(
        &mut self,
        function: &ResolvedFunction,
        values: Vec<(ValueId, Value)>,
        depth: usize,
    ) -> Result<Value, Flow> {
        // Stage semantic work v1: one unit per admitted source frame.
        self.semantic_charge()?;
        let mut frame = Environment::from(values);
        frame.control = lexical_control::frame_control(function)?;
        self.set_trace_phase(ResolvedTracePhase::Requires);
        for (index, clause) in function.requires.iter().enumerate() {
            self.charge()?;
            match self.evaluate(clause, &mut frame, depth)? {
                Value::Bool(true) => {}
                Value::Bool(false) => {
                    return Err(self.contract_failure(
                        function,
                        &frame,
                        ContractPhase::Requires,
                        index,
                    ))
                }
                _ => return Err(Flow::Guard("non-boolean requires clause")),
            }
        }
        self.set_trace_phase(ResolvedTracePhase::Body);
        let mut value = match self.evaluate(&function.body, &mut frame, depth) {
            Ok(value) | Err(Flow::Residual(value)) => value,
            Err(Flow::Lexical(lexical_control::ExitFlow::Return {
                target,
                value,
                resume,
            })) if frame.control.as_deref().is_some_and(|control| {
                target == hir::ControlTarget::Function(control.owner.clone())
            }) =>
            {
                frame.control.as_deref_mut().unwrap().resume = resume;
                value
            }
            Err(Flow::Lexical(_)) => {
                return Err(Flow::Guard(
                    "lexical control escaped its exact runtime target",
                ))
            }
            Err(flow) => return Err(flow),
        };
        let retained_result = !function.ensures.is_empty()
            && (matches!(value, Value::OnceClosure(_))
                || matches!(&value, Value::Closure(closure) if closure.mutable.is_some()));
        if !function.ensures.is_empty() {
            let result_value = if retained_result {
                std::mem::replace(&mut value, Value::Moved)
            } else {
                self.clone_value(&value)?
            };
            frame.push((function.result_id.clone(), result_value));
        }
        self.set_trace_phase(ResolvedTracePhase::Ensures);
        for (index, clause) in function.ensures.iter().enumerate() {
            self.charge()?;
            match self.evaluate(clause, &mut frame, depth)? {
                Value::Bool(true) => {}
                Value::Bool(false) => {
                    let failure =
                        self.contract_failure(function, &frame, ContractPhase::Ensures, index);
                    if frame.control.is_some() {
                        // Remove the contract-only alias before settling the canonical
                        // provisional result; it is never a second owning epoch.
                        if let Some(result) = frame.get_mut(&function.result_id) {
                            let result = std::mem::replace(result, Value::Moved);
                            if retained_result {
                                value = result;
                            } else {
                                drop(result);
                            }
                        }
                        self.settle_lexical_epilogue(
                            function,
                            &mut frame,
                            Some(&clause.id),
                            Some(&mut value),
                        )?;
                    }
                    return Err(failure);
                }
                _ => return Err(Flow::Guard("non-boolean ensures clause")),
            }
        }
        self.settle_lexical_epilogue(function, &mut frame, None, None)?;
        if retained_result {
            take_owned_place(
                &mut frame,
                &crate::hir::Place {
                    root: function.result_id.clone(),
                    projections: Vec::new(),
                },
            )
            .ok_or(Flow::Guard(
                "affine provisional result was consumed by a contract",
            ))
        } else {
            Ok(value)
        }
    }
}
