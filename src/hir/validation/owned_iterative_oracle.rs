//! Independently saved complete original iterative validator.
use super::*;
include!("owned_iterative_oracle/dispatch_3.rs");
include!("owned_iterative_oracle/dispatch_2.rs");
include!("owned_iterative_oracle/dispatch_1.rs");
include!("owned_iterative_oracle/dispatch_0.rs");
impl HirValidator<'_> {
    pub(in crate::hir) fn validate_expr_iterative_owned_reference_body(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
        path: &str,
        allow_moves: bool,
        allowed_effects: Option<effect_keys::Allowed<'_>>,
    ) -> Result<(), Diagnostic> {
        let program = self.program;
        enum Frame<'e> {
            LexicalReturn {
                expression: &'e ResolvedExpr,
                operand: &'e ResolvedExpr,
            },
            InvokeNext {
                expression: &'e ResolvedExpr,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: &'e str,
            },
            InvokeAfter {
                expression: &'e ResolvedExpr,
                index: usize,
                path: &'e str,
            },
            RestorePublication(bool),
            Enter {
                expression: &'e ResolvedExpr,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: String,
            },
            Unary {
                expression: &'e ResolvedExpr,
                op: UnaryOp,
            },
            ByteRangeAfterSource {
                expression: &'e ResolvedExpr,
                start: &'e ResolvedExpr,
                end: &'e ResolvedExpr,
                path: &'e str,
            },
            ByteRangeAfterStart {
                expression: &'e ResolvedExpr,
                end: &'e ResolvedExpr,
                path: &'e str,
            },
            ByteRangeAfterEnd {
                expression: &'e ResolvedExpr,
            },
            BinaryLeft {
                expression: &'e ResolvedExpr,
                op: BinaryOp,
                right: &'e ResolvedExpr,
                path: &'e str,
            },
            BinaryRight {
                expression: &'e ResolvedExpr,
                op: BinaryOp,
                left: &'e ResolvedExpr,
                baseline: Option<(Vec<ValueId>, BTreeMap<ValueId, ValidationBinding>)>,
            },
            IfCondition {
                expression: &'e ResolvedExpr,
                then_branch: &'e ResolvedExpr,
                else_branch: &'e ResolvedExpr,
                path: &'e str,
            },
            IfThen {
                expression: &'e ResolvedExpr,
                else_branch: &'e ResolvedExpr,
                path: &'e str,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
            },
            IfElse {
                expression: &'e ResolvedExpr,
                then_scope: BTreeMap<ValueId, ValidationBinding>,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
            },
            Project {
                expression: &'e ResolvedExpr,
                field: &'e DeclarationId,
            },
            Upcast {
                expression: &'e ResolvedExpr,
            },
            /// Resumable Effects v1 (issue #204). See `hir::resolve_yield`.
            Yield {
                expression: &'e ResolvedExpr,
            },
            Try {
                expression: &'e ResolvedExpr,
                path: &'e str,
                option: bool,
            },
            CallNext {
                expression: &'e ResolvedExpr,
                args: &'e [ResolvedExpr],
                params: CallParameters<'e>,
                return_type: call_signature::ResultType<'e>,
                return_ownership: OwnershipMode,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: std::borrow::Cow<'e, str>,
            },
            CallAfterArg {
                expression: &'e ResolvedExpr,
                args: &'e [ResolvedExpr],
                params: CallParameters<'e>,
                return_type: call_signature::ResultType<'e>,
                return_ownership: OwnershipMode,
                index: usize,
                path: std::borrow::Cow<'e, str>,
            },
            NativeNext {
                expression: &'e ResolvedExpr,
                args: &'e [ResolvedExpr],
                params: &'e [ResolvedImportParameter],
                result: ResolvedType,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: &'e str,
            },
            NativeAfterArg {
                expression: &'e ResolvedExpr,
                args: &'e [ResolvedExpr],
                params: &'e [ResolvedImportParameter],
                result: ResolvedType,
                index: usize,
                path: &'e str,
            },
            BlockNext {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: &'e str,
            },
            BlockAfterLet {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: &'e str,
            },
            BlockAfterAssign {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: &'e str,
            },
            BlockAfterUnsafe {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: &'e str,
            },
            BlockAfterWhileCondition {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: &'e str,
                body: &'e ResolvedExpr,
                entry: BTreeMap<ValueId, ValidationBinding>,
            },
            BlockAfterWhileBody {
                expression: &'e ResolvedExpr,
                statements: &'e [ResolvedStatement],
                tail: &'e ResolvedExpr,
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: &'e str,
                entry: BTreeMap<ValueId, ValidationBinding>,
            },
            BlockTail {
                expression: &'e ResolvedExpr,
                outer_ids: Vec<ValueId>,
                outer: BTreeMap<ValueId, ValidationBinding>,
            },
            RecordNext {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: &'e [ResolvedFieldDeclaration],
                record: &'e DeclarationId,
                arguments: &'e [ResolvedType],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: &'e str,
            },
            RecordAfterField {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: &'e [ResolvedFieldDeclaration],
                record: &'e DeclarationId,
                arguments: &'e [ResolvedType],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                path: &'e str,
            },
            VariantNext {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: &'e [ResolvedFieldDeclaration],
                variant: &'e DeclarationId,
                case: &'e DeclarationId,
                arguments: &'e [ResolvedType],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: &'e str,
            },
            VariantAfterField {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: &'e [ResolvedFieldDeclaration],
                variant: &'e DeclarationId,
                case: &'e DeclarationId,
                arguments: &'e [ResolvedType],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                path: &'e str,
            },
            UpdateBase {
                expression: &'e ResolvedExpr,
                record: &'e DeclarationId,
                fields: &'e [ResolvedFieldInitializer],
                path: &'e str,
            },
            UpdateNext {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: &'e [ResolvedFieldDeclaration],
                record: &'e DeclarationId,
                arguments: &'e [ResolvedType],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: &'e str,
                ownership: OwnershipMode,
            },
            UpdateAfterField {
                expression: &'e ResolvedExpr,
                fields: &'e [ResolvedFieldInitializer],
                expected: &'e [ResolvedFieldDeclaration],
                record: &'e DeclarationId,
                arguments: &'e [ResolvedType],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                path: &'e str,
                ownership: OwnershipMode,
            },
            MatchScrutinee {
                expression: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                path: &'e str,
            },
            RecordMatchArm {
                expression: &'e ResolvedExpr,
                arm: &'e ResolvedMatchArm,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                borrowed_root: Option<ValueId>,
            },
            VariantMatchNext {
                expression: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                cases: &'e [ResolvedVariantCaseDeclaration],
                variant: &'e DeclarationId,
                arguments: &'e [ResolvedType],
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                arm_scopes: Vec<BTreeMap<ValueId, ValidationBinding>>,
                covered: BTreeSet<DeclarationId>,
                wildcard_seen: bool,
                result: Option<(ResolvedType, OwnershipMode)>,
                path: &'e str,
            },
            VariantMatchAfterArm {
                expression: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                cases: &'e [ResolvedVariantCaseDeclaration],
                variant: &'e DeclarationId,
                arguments: &'e [ResolvedType],
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                arm_scopes: Vec<BTreeMap<ValueId, ValidationBinding>>,
                covered: BTreeSet<DeclarationId>,
                wildcard_seen: bool,
                result: Option<(ResolvedType, OwnershipMode)>,
                path: &'e str,
            },
            /// Refutable Match v1 decision-chain twins. They mirror the
            /// resolver's Copy-scalar admission structurally, fail-closed.
            ScalarMatchNext {
                expression: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                arm_scopes: Vec<BTreeMap<ValueId, ValidationBinding>>,
                result: Option<(ResolvedType, OwnershipMode)>,
                path: &'e str,
            },
            ScalarMatchAfterGuard {
                expression: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                arm_scopes: Vec<BTreeMap<ValueId, ValidationBinding>>,
                result: Option<(ResolvedType, OwnershipMode)>,
                path: &'e str,
            },
            ScalarMatchAfterArm {
                expression: &'e ResolvedExpr,
                arms: &'e [ResolvedMatchArm],
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                arm_scopes: Vec<BTreeMap<ValueId, ValidationBinding>>,
                result: Option<(ResolvedType, OwnershipMode)>,
                path: &'e str,
            },
        }

        const { assert!(std::mem::size_of::<Frame<'static>>() == super::VALIDATOR_FRAME_BYTES) };
        #[cfg(test)]
        fn frame_owned_capacity(frame: &Frame<'_>) -> usize {
            let ids = |values: &Vec<ValueId>| {
                values.capacity() * std::mem::size_of::<ValueId>()
                    + values.iter().map(|id| id.as_str().len()).sum::<usize>()
            };
            let scope = |scope: &BTreeMap<ValueId, ValidationBinding>| {
                validation_scope_owned_capacity(scope)
            };
            let path = match frame {
                Frame::Enter { path, .. } => path.capacity(),
                Frame::CallNext {
                    path: std::borrow::Cow::Owned(path),
                    ..
                }
                | Frame::CallAfterArg {
                    path: std::borrow::Cow::Owned(path),
                    ..
                } => path.capacity(),
                _ => 0,
            };
            let retained = match frame {
                Frame::InvokeNext { scope: value, .. } | Frame::Enter { scope: value, .. } => {
                    scope(value)
                }
                Frame::BinaryRight { baseline, .. } => baseline
                    .as_ref()
                    .map_or(0, |(outer_ids, value)| ids(outer_ids) + scope(value)),
                Frame::IfThen {
                    outer, outer_ids, ..
                } => scope(outer) + ids(outer_ids),
                Frame::IfElse {
                    then_scope,
                    outer,
                    outer_ids,
                    ..
                } => scope(then_scope) + scope(outer) + ids(outer_ids),
                Frame::CallNext {
                    params,
                    return_type,
                    scope: value,
                    ..
                } => {
                    params.owned_capacity()
                        + call_signature::owned_capacity(return_type)
                        + scope(value)
                }
                Frame::CallAfterArg {
                    params,
                    return_type,
                    ..
                } => params.owned_capacity() + call_signature::owned_capacity(return_type),
                Frame::NativeNext {
                    result,
                    scope: value,
                    ..
                } => resolved_type_owned_capacity(result) + scope(value),
                Frame::NativeAfterArg { result, .. } => resolved_type_owned_capacity(result),
                Frame::BlockNext {
                    scope: value,
                    outer,
                    outer_ids,
                    ..
                }
                | Frame::BlockAfterAssign {
                    scope: value,
                    outer,
                    outer_ids,
                    ..
                } => scope(value) + scope(outer) + ids(outer_ids),
                Frame::BlockAfterLet {
                    outer, outer_ids, ..
                }
                | Frame::BlockTail {
                    outer, outer_ids, ..
                } => scope(outer) + ids(outer_ids),
                Frame::RecordNext {
                    seen, scope: value, ..
                }
                | Frame::VariantNext {
                    seen, scope: value, ..
                }
                | Frame::UpdateNext {
                    seen, scope: value, ..
                } => {
                    seen.len()
                        * (std::mem::size_of::<DeclarationId>()
                            + std::mem::size_of::<BTreeSet<DeclarationId>>())
                        + seen.iter().map(|id| id.as_str().len()).sum::<usize>()
                        + scope(value)
                }
                Frame::RecordAfterField { seen, .. }
                | Frame::VariantAfterField { seen, .. }
                | Frame::UpdateAfterField { seen, .. } => {
                    seen.len()
                        * (std::mem::size_of::<DeclarationId>()
                            + std::mem::size_of::<BTreeSet<DeclarationId>>())
                        + seen.iter().map(|id| id.as_str().len()).sum::<usize>()
                }
                Frame::RecordMatchArm {
                    outer, outer_ids, ..
                } => scope(outer) + ids(outer_ids),
                Frame::VariantMatchNext {
                    outer,
                    outer_ids,
                    arm_scopes,
                    covered,
                    result,
                    ..
                }
                | Frame::VariantMatchAfterArm {
                    outer,
                    outer_ids,
                    arm_scopes,
                    covered,
                    result,
                    ..
                } => {
                    scope(outer)
                        + ids(outer_ids)
                        + arm_scopes.capacity()
                            * std::mem::size_of::<BTreeMap<ValueId, ValidationBinding>>()
                        + arm_scopes.iter().map(scope).sum::<usize>()
                        + covered.len()
                            * (std::mem::size_of::<DeclarationId>()
                                + std::mem::size_of::<BTreeSet<DeclarationId>>())
                        + covered.iter().map(|id| id.as_str().len()).sum::<usize>()
                        + result
                            .as_ref()
                            .map_or(0, |(ty, _)| resolved_type_owned_capacity(ty))
                }
                Frame::ScalarMatchNext {
                    outer,
                    outer_ids,
                    arm_scopes,
                    result,
                    ..
                }
                | Frame::ScalarMatchAfterGuard {
                    outer,
                    outer_ids,
                    arm_scopes,
                    result,
                    ..
                }
                | Frame::ScalarMatchAfterArm {
                    outer,
                    outer_ids,
                    arm_scopes,
                    result,
                    ..
                } => {
                    scope(outer)
                        + ids(outer_ids)
                        + arm_scopes.capacity()
                            * std::mem::size_of::<BTreeMap<ValueId, ValidationBinding>>()
                        + arm_scopes.iter().map(scope).sum::<usize>()
                        + result
                            .as_ref()
                            .map_or(0, |(ty, _)| resolved_type_owned_capacity(ty))
                }
                _ => 0,
            };
            path.saturating_add(retained)
        }
        let initial_scope = std::mem::take(scope);
        let mut publication = ValidationScopePublication {
            target: scope,
            published: initial_scope.clone(),
            enabled: true,
        };
        let mut frames = vec![Frame::Enter {
            expression,
            scope: initial_scope,
            path: path.to_owned(),
        }];
        let mut scopes = Vec::new();
        while let Some(frame) = frames.pop() {
            #[cfg(test)]
            note_iterative_phase_capacity(
                1,
                frames.capacity() * std::mem::size_of::<Frame<'_>>()
                    + scopes.capacity()
                        * std::mem::size_of::<BTreeMap<ValueId, ValidationBinding>>()
                    + scopes
                        .iter()
                        .map(validation_scope_owned_capacity)
                        .sum::<usize>()
                    + validation_scope_owned_capacity(&publication.published)
                    + frames.iter().map(frame_owned_capacity).sum::<usize>()
                    + frame_owned_capacity(&frame)
                    + self.nominal_facts.owned_capacity()
                    + self
                        .byte_helpers
                        .map_or(0, |index| index.owned_capacity_bytes())
                    + self.expression_ids.len()
                        * (std::mem::size_of::<ExpressionId>()
                            + std::mem::size_of::<BTreeSet<ExpressionId>>())
                    + self
                        .expression_ids
                        .iter()
                        .map(|id| id.as_str().len())
                        .sum::<usize>()
                    + self.value_ids.len()
                        * (std::mem::size_of::<ValueId>()
                            + std::mem::size_of::<BTreeSet<ValueId>>())
                    + self
                        .value_ids
                        .iter()
                        .map(|id| id.as_str().len())
                        .sum::<usize>()
                    + self.functions.len()
                        * (std::mem::size_of::<(DeclarationId, &ResolvedFunction)>()
                            + std::mem::size_of::<BTreeMap<DeclarationId, &ResolvedFunction>>())
                    + self
                        .functions
                        .keys()
                        .map(|id| id.as_str().len())
                        .sum::<usize>(),
            );
            owned_dispatch_0!(
                frame,
                self,
                function,
                allow_moves,
                allowed_effects,
                program,
                publication,
                frames,
                scopes,
                Frame
            );
        }
        if scopes.len() != 1 {
            return Err(hir_error("iterative HIR validator lost its scope stack"));
        }
        publication.publish(&scopes.pop().expect("root validation scope retained"));
        Ok(())
    }
}
