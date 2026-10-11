//! One exact static iterator epoch, independently scoped to each certified latch.
use super::*;
impl Emitter<'_> {
    fn latch_epoch(
        expression: &ResolvedExpr,
    ) -> Result<crate::cleanup_plan::StorageId, Diagnostic> {
        let ResolvedExprKind::Call {
            callee,
            instance: None,
            args,
            ..
        } = &expression.kind
        else {
            return Err(error("owned latch tail has no exact Next call"));
        };
        let [argument] = args.as_slice() else {
            return Err(error("owned latch Next arity differs"));
        };
        if callee.as_str() != crate::iterator_ops::NEXT_ID {
            return Err(error("owned latch callee differs"));
        }
        Ok(crate::cleanup_plan::StorageId::CallArgument {
            call: expression.id.clone(),
            parameter_index: 0,
            value_expression: argument.id.clone(),
        })
    }
    pub(in crate::wasm::aggregate) fn take_latch_epoch(
        &mut self,
        expression: &ResolvedExpr,
        remainder: &ValueId,
    ) -> Result<Option<Value>, Diagnostic> {
        use crate::cleanup_plan::{CleanupTransition as T, StorageId};
        let entry = self
            .lexical_resume
            .ok_or_else(|| error("owned latch has no certified resume context"))?;
        if self.cleanup_plan.lexical_resume(entry).is_none() {
            return Err(error("owned latch resume is absent"));
        }
        let epoch = Self::latch_epoch(expression)?;
        let StorageId::CallArgument {
            value_expression, ..
        } = &epoch
        else {
            unreachable!()
        };
        let mut entered = 0usize;
        let mut committed = 0usize;
        for block in self.cleanup_plan.blocks.iter().filter(|b| {
            self.cleanup_plan
                .lexical_block_in_context(Some(entry), b.id)
        }) {
            for transition in &block.transitions {
                match transition {
                    T::Transfer {
                        at,
                        source,
                        destination,
                    } if destination.storage == epoch => {
                        if at != value_expression
                            || !destination.projections.is_empty()
                            || !source.projections.is_empty()
                            || source.storage != StorageId::Value(remainder.clone())
                        {
                            return Err(error(
                                "owned latch epoch transfer differs from exact remainder",
                            ));
                        }
                        entered += 1;
                    }
                    T::CallCommit { call, arguments } if call == &expression.id => {
                        let [argument] = arguments.as_slice() else {
                            return Err(error("owned latch commit arity differs"));
                        };
                        if argument.parameter_index != 0
                            || argument.source.storage != epoch
                            || !argument.source.projections.is_empty()
                        {
                            return Err(error("owned latch commit differs from exact epoch"));
                        }
                        committed += 1;
                    }
                    _ => {}
                }
            }
        }
        if entered != 1 || committed != 1 {
            return Err(error(
                "owned latch epoch must enter and commit exactly once in its resume",
            ));
        }
        // Remove only this tail's epoch. Every duplicate within the current
        // domain still reaches iterator_argument's original uniqueness guard.
        Ok(self.call_argument_values.remove(&epoch))
    }
    pub(in crate::wasm::aggregate) fn restore_latch_epoch(
        &mut self,
        expression: &ResolvedExpr,
        previous: Option<Value>,
    ) -> Result<(), Diagnostic> {
        let epoch = Self::latch_epoch(expression)?;
        let current = self
            .call_argument_values
            .remove(&epoch)
            .ok_or_else(|| error("owned latch emitted no exact epoch carrier"))?;
        if let Some(previous) = previous {
            let agrees = match (&previous, &current) {
                (
                    Value::Aggregate { pointer: a, ty: at },
                    Value::Aggregate { pointer: b, ty: bt },
                ) => a.local == b.local && a.offset == b.offset && at == bt,
                _ => false,
            };
            if !agrees {
                return Err(error(
                    "owned latch carrier differs across certified emissions",
                ));
            }
            self.call_argument_values.insert(epoch, previous);
        }
        Ok(())
    }
}
