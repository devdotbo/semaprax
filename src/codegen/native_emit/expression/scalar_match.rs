use super::*;
impl<'a, O: COutput> CEmitter<'a, O> {
    /// Refutable Match v1 native lowering: the scrutinee stages once, then
    /// every arm tests `!matched && (<literal equality>)` with an optional
    /// inner guard branch. `&&` short-circuits so a guard evaluates exactly
    /// once per reached arm whose pattern matched; failing guards leave
    /// `matched` false and fall through to the following arms. The resolver
    /// guarantees one trailing irrefutable guard-free catch-all, but the
    /// defensive no-arm check mirrors exhaustive matches.
    pub(super) fn emit_scalar_match(
        &mut self,
        expr: &ResolvedExpr,
        scalar_scrutinee: &ExpressionId,
        scrutinee: &CValue,
        arms: &[hir::ResolvedMatchArm],
    ) -> Result<CValue, Diagnostic> {
        let staged = self.temporary(&scrutinee.ty)?;
        self.line(&format!("{staged} = {};", scrutinee.code));
        let result = if expr.ty == ResolvedType::Never {
            String::new()
        } else if is_direct_plan_owned(self.program, &expr.ty) {
            self.bytes_plan
                .ok_or_else(|| backend_error("owned Bytes match has no cleanup plan"))?
                .value(&crate::cleanup_plan::StorageId::Temporary(expr.id.clone()))?
                .to_owned()
        } else {
            self.temporary(&expr.ty)?
        };
        let matched = self.temporary(&ResolvedType::Bool)?;
        self.line(&format!("{matched} = false;"));
        // The builder lowers the one admitted guard-free catch-all directly
        // in its parent region. Every guarded or multi-arm scalar match has
        // the canonical arm-value child region selected below.
        let arm_value_has_child_region = arms.len() != 1 || arms[0].guard.is_some();
        for arm in arms {
            let saved = self.variables.clone();
            if let hir::ResolvedMatchPattern::Binding(binding) = &arm.pattern {
                self.variables.insert(
                    binding.id.clone(),
                    CBinding {
                        name: staged.clone(),
                        ty: binding.ty.clone(),
                    },
                );
            }
            let test = match &arm.pattern {
                hir::ResolvedMatchPattern::Wildcard | hir::ResolvedMatchPattern::Binding(_) => None,
                hir::ResolvedMatchPattern::Literal(value) => {
                    Some(format!("{staged} == {}", c_pattern_literal(*value)))
                }
                hir::ResolvedMatchPattern::Or(alternatives) => Some(
                    alternatives
                        .iter()
                        .map(|alternative| match alternative {
                            hir::ResolvedMatchPattern::Literal(value) => {
                                format!("{staged} == {}", c_pattern_literal(*value))
                            }
                            _ => unreachable!("or-pattern alternatives are literals"),
                        })
                        .collect::<Vec<_>>()
                        .join(" || "),
                ),
                hir::ResolvedMatchPattern::Variant { .. }
                | hir::ResolvedMatchPattern::Record { .. } => {
                    return Err(backend_error(
                        "aggregate pattern has a Copy-scalar match scrutinee",
                    ));
                }
            };
            match &test {
                Some(test) => self.line(&format!("if (!{matched} && ({test})) {{")),
                None => self.line(&format!("if (!{matched}) {{")),
            }
            self.indent += 1;
            if let Some(guard) = &arm.guard {
                // The guard evaluates once here, after the pattern matched
                // and before any part of the arm value; a false guard leaves
                // `matched` untouched and falls through to the next arm.
                let flag = self.emit_expr(guard)?;
                self.require_type(&flag.ty, &ResolvedType::Bool, "match guard")?;
                self.emit_scalar_match_guard_scope_exit(guard)?;
                self.line(&format!("if ({}) {{", flag.code));
                self.indent += 1;
                self.line(&format!("{matched} = true;"));
                let value = self.emit_expr(&arm.value)?;
                self.require_flow_type(&value.ty, &expr.ty, "match arm result")?;
                if value.ty != ResolvedType::Never {
                    if is_direct_plan_owned(self.program, &expr.ty) {
                        let transfer = self
                            .bytes_plan
                            .expect("checked above")
                            .transfer_scalar_match_arm(&expr.id, &arm.value.id)?;
                        for line in transfer.lines() {
                            self.line(line);
                        }
                    } else {
                        self.line(&format!("{result} = {};", value.code));
                    }
                    if arm_value_has_child_region {
                        self.emit_scalar_match_value_scope_exit(scalar_scrutinee, &arm.value)?;
                    }
                }
                self.indent -= 1;
                self.line("}");
            } else {
                self.line(&format!("{matched} = true;"));
                let value = self.emit_expr(&arm.value)?;
                self.require_flow_type(&value.ty, &expr.ty, "match arm result")?;
                if value.ty != ResolvedType::Never {
                    if is_direct_plan_owned(self.program, &expr.ty) {
                        let transfer = self
                            .bytes_plan
                            .expect("checked above")
                            .transfer_scalar_match_arm(&expr.id, &arm.value.id)?;
                        for line in transfer.lines() {
                            self.line(line);
                        }
                    } else {
                        self.line(&format!("{result} = {};", value.code));
                    }
                    if arm_value_has_child_region {
                        self.emit_scalar_match_value_scope_exit(scalar_scrutinee, &arm.value)?;
                    }
                }
            }
            self.variables = saved;
            self.indent -= 1;
            self.line("}");
        }
        self.line(&format!(
            "if (!{matched}) spx_runtime_invariant_failure(\"refutable match selected no arm\");"
        ));
        Ok(CValue {
            code: result,
            ty: expr.ty.clone(),
        })
    }
}
