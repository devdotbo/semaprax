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

        const { frame_layout::check::<Frame<'static>>() };
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

// The saved validator executes against live HIR. Its old fixed tariff remains
// 248 bytes, while its actual inline result payload follows DeclarationId ABI.
// Both complete enum shapes below are layout witnesses, never replay producers.
#[rustfmt::skip] // Preserve complete historical enum/type source in this witness.
mod frame_layout {
    use super::*;
    #[derive(Clone)]
    #[allow(dead_code)] // Historical ABI witness; no value is manufactured.
    struct HistoricalDeclarationId(HistoricalDeclarationText);
    #[derive(Clone)]
    #[allow(dead_code)] // Exact pre-891 private payload alternatives.
    enum HistoricalDeclarationText {
        CompilerStatic(&'static str),
        Shared(std::sync::Arc<str>),
    }
    #[derive(Clone)]
    #[allow(dead_code)] // Whole original type layout, independent of live HIR.
    enum HistoricalResolvedType {
    /// Compiler-owned absence of normal completion; never a stored/ABI value.
    Never,
    /// A retained affine zero-argument, i64-result callable.
    OnceFunction,
    OnceFunctionI64,
    OnceFunctionI64Pair,
    /// Same-thread transactional i64 callback; unlike ordinary function
    /// values it owns one mutable receiver state cell.
    MutFunctionI64,
    Function {
        parameters: Vec<HistoricalResolvedType>,
        result: Box<HistoricalResolvedType>,
    },
    Unit,
    I64,
    /// A checked signed 32-bit integer.
    I32,
    /// One Unicode scalar value.
    Char,
    /// One unsigned 8-bit integer value.
    U8,
    /// A target-independent checked unsigned 64-bit semantic integer.
    Usize,
    /// Inline Copy byte storage with exact target-independent length.
    ArrayU8(u32),
    /// IEEE-754 single precision.
    F32,
    /// IEEE-754 double precision.
    F64,
    Bool,
    /// An owned heap UTF-8 string value; never `Copy`.
    String,
    /// Uniquely owned immutable bytes. This needs drop but is not a resource.
    Bytes,
    /// A non-owning UTF-8 view rooted in the current invocation.
    Str,
    /// A non-owning byte view rooted in the current invocation.
    SliceU8,
    /// String Collections v1: the uniquely owned `Map<string, i64>` carrier.
    /// It needs drop, is never `Copy`, and is admitted only for local
    /// bindings threaded through the compiler-owned `map_*` operations.
    StringMap,
    TypeParameter {
        owner: HistoricalDeclarationId,
        index: u32,
    },
    Nominal {
        declaration: HistoricalDeclarationId,
        arguments: Vec<HistoricalResolvedType>,
    },
}
    macro_rules! frame {
        ($name:ident, $life:lifetime, $ty:ty) => {
            #[allow(dead_code)] // Complete layout witness, not an executable oracle.
        enum $name<$life> {
            LexicalReturn {
                expression: &$life ResolvedExpr,
                operand: &$life ResolvedExpr,
            },
            InvokeNext {
                expression: &$life ResolvedExpr,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: &$life str,
            },
            InvokeAfter {
                expression: &$life ResolvedExpr,
                index: usize,
                path: &$life str,
            },
            RestorePublication(bool),
            Enter {
                expression: &$life ResolvedExpr,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: String,
            },
            Unary {
                expression: &$life ResolvedExpr,
                op: UnaryOp,
            },
            ByteRangeAfterSource {
                expression: &$life ResolvedExpr,
                start: &$life ResolvedExpr,
                end: &$life ResolvedExpr,
                path: &$life str,
            },
            ByteRangeAfterStart {
                expression: &$life ResolvedExpr,
                end: &$life ResolvedExpr,
                path: &$life str,
            },
            ByteRangeAfterEnd {
                expression: &$life ResolvedExpr,
            },
            BinaryLeft {
                expression: &$life ResolvedExpr,
                op: BinaryOp,
                right: &$life ResolvedExpr,
                path: &$life str,
            },
            BinaryRight {
                expression: &$life ResolvedExpr,
                op: BinaryOp,
                left: &$life ResolvedExpr,
                baseline: Option<(Vec<ValueId>, BTreeMap<ValueId, ValidationBinding>)>,
            },
            IfCondition {
                expression: &$life ResolvedExpr,
                then_branch: &$life ResolvedExpr,
                else_branch: &$life ResolvedExpr,
                path: &$life str,
            },
            IfThen {
                expression: &$life ResolvedExpr,
                else_branch: &$life ResolvedExpr,
                path: &$life str,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
            },
            IfElse {
                expression: &$life ResolvedExpr,
                then_scope: BTreeMap<ValueId, ValidationBinding>,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
            },
            Project {
                expression: &$life ResolvedExpr,
                field: &$life DeclarationId,
            },
            Upcast {
                expression: &$life ResolvedExpr,
            },
            /// Resumable Effects v1 (issue #204). See `hir::resolve_yield`.
            Yield {
                expression: &$life ResolvedExpr,
            },
            Try {
                expression: &$life ResolvedExpr,
                path: &$life str,
                option: bool,
            },
            CallNext {
                expression: &$life ResolvedExpr,
                args: &$life [ResolvedExpr],
                params: CallParameters<$life>,
                return_type: std::borrow::Cow<$life, $ty>,
                return_ownership: OwnershipMode,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: std::borrow::Cow<$life, str>,
            },
            CallAfterArg {
                expression: &$life ResolvedExpr,
                args: &$life [ResolvedExpr],
                params: CallParameters<$life>,
                return_type: std::borrow::Cow<$life, $ty>,
                return_ownership: OwnershipMode,
                index: usize,
                path: std::borrow::Cow<$life, str>,
            },
            NativeNext {
                expression: &$life ResolvedExpr,
                args: &$life [ResolvedExpr],
                params: &$life [ResolvedImportParameter],
                result: $ty,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: &$life str,
            },
            NativeAfterArg {
                expression: &$life ResolvedExpr,
                args: &$life [ResolvedExpr],
                params: &$life [ResolvedImportParameter],
                result: $ty,
                index: usize,
                path: &$life str,
            },
            BlockNext {
                expression: &$life ResolvedExpr,
                statements: &$life [ResolvedStatement],
                tail: &$life ResolvedExpr,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: &$life str,
            },
            BlockAfterLet {
                expression: &$life ResolvedExpr,
                statements: &$life [ResolvedStatement],
                tail: &$life ResolvedExpr,
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: &$life str,
            },
            BlockAfterAssign {
                expression: &$life ResolvedExpr,
                statements: &$life [ResolvedStatement],
                tail: &$life ResolvedExpr,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: &$life str,
            },
            BlockAfterUnsafe {
                expression: &$life ResolvedExpr,
                statements: &$life [ResolvedStatement],
                tail: &$life ResolvedExpr,
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: &$life str,
            },
            BlockAfterWhileCondition {
                expression: &$life ResolvedExpr,
                statements: &$life [ResolvedStatement],
                tail: &$life ResolvedExpr,
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: &$life str,
                body: &$life ResolvedExpr,
                entry: BTreeMap<ValueId, ValidationBinding>,
            },
            BlockAfterWhileBody {
                expression: &$life ResolvedExpr,
                statements: &$life [ResolvedStatement],
                tail: &$life ResolvedExpr,
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                path: &$life str,
                entry: BTreeMap<ValueId, ValidationBinding>,
            },
            BlockTail {
                expression: &$life ResolvedExpr,
                outer_ids: Vec<ValueId>,
                outer: BTreeMap<ValueId, ValidationBinding>,
            },
            RecordNext {
                expression: &$life ResolvedExpr,
                fields: &$life [ResolvedFieldInitializer],
                expected: &$life [ResolvedFieldDeclaration],
                record: &$life DeclarationId,
                arguments: &$life [$ty],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: &$life str,
            },
            RecordAfterField {
                expression: &$life ResolvedExpr,
                fields: &$life [ResolvedFieldInitializer],
                expected: &$life [ResolvedFieldDeclaration],
                record: &$life DeclarationId,
                arguments: &$life [$ty],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                path: &$life str,
            },
            VariantNext {
                expression: &$life ResolvedExpr,
                fields: &$life [ResolvedFieldInitializer],
                expected: &$life [ResolvedFieldDeclaration],
                variant: &$life DeclarationId,
                case: &$life DeclarationId,
                arguments: &$life [$ty],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: &$life str,
            },
            VariantAfterField {
                expression: &$life ResolvedExpr,
                fields: &$life [ResolvedFieldInitializer],
                expected: &$life [ResolvedFieldDeclaration],
                variant: &$life DeclarationId,
                case: &$life DeclarationId,
                arguments: &$life [$ty],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                path: &$life str,
            },
            UpdateBase {
                expression: &$life ResolvedExpr,
                record: &$life DeclarationId,
                fields: &$life [ResolvedFieldInitializer],
                path: &$life str,
            },
            UpdateNext {
                expression: &$life ResolvedExpr,
                fields: &$life [ResolvedFieldInitializer],
                expected: &$life [ResolvedFieldDeclaration],
                record: &$life DeclarationId,
                arguments: &$life [$ty],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                scope: BTreeMap<ValueId, ValidationBinding>,
                path: &$life str,
                ownership: OwnershipMode,
            },
            UpdateAfterField {
                expression: &$life ResolvedExpr,
                fields: &$life [ResolvedFieldInitializer],
                expected: &$life [ResolvedFieldDeclaration],
                record: &$life DeclarationId,
                arguments: &$life [$ty],
                seen: BTreeSet<DeclarationId>,
                index: usize,
                path: &$life str,
                ownership: OwnershipMode,
            },
            MatchScrutinee {
                expression: &$life ResolvedExpr,
                arms: &$life [ResolvedMatchArm],
                path: &$life str,
            },
            RecordMatchArm {
                expression: &$life ResolvedExpr,
                arm: &$life ResolvedMatchArm,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                borrowed_root: Option<ValueId>,
            },
            VariantMatchNext {
                expression: &$life ResolvedExpr,
                arms: &$life [ResolvedMatchArm],
                cases: &$life [ResolvedVariantCaseDeclaration],
                variant: &$life DeclarationId,
                arguments: &$life [$ty],
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                arm_scopes: Vec<BTreeMap<ValueId, ValidationBinding>>,
                covered: BTreeSet<DeclarationId>,
                wildcard_seen: bool,
                result: Option<($ty, OwnershipMode)>,
                path: &$life str,
            },
            VariantMatchAfterArm {
                expression: &$life ResolvedExpr,
                arms: &$life [ResolvedMatchArm],
                cases: &$life [ResolvedVariantCaseDeclaration],
                variant: &$life DeclarationId,
                arguments: &$life [$ty],
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                arm_scopes: Vec<BTreeMap<ValueId, ValidationBinding>>,
                covered: BTreeSet<DeclarationId>,
                wildcard_seen: bool,
                result: Option<($ty, OwnershipMode)>,
                path: &$life str,
            },
            /// Refutable Match v1 decision-chain twins. They mirror the
            /// resolver's Copy-scalar admission structurally, fail-closed.
            ScalarMatchNext {
                expression: &$life ResolvedExpr,
                arms: &$life [ResolvedMatchArm],
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                arm_scopes: Vec<BTreeMap<ValueId, ValidationBinding>>,
                result: Option<($ty, OwnershipMode)>,
                path: &$life str,
            },
            ScalarMatchAfterGuard {
                expression: &$life ResolvedExpr,
                arms: &$life [ResolvedMatchArm],
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                arm_scopes: Vec<BTreeMap<ValueId, ValidationBinding>>,
                result: Option<($ty, OwnershipMode)>,
                path: &$life str,
            },
            ScalarMatchAfterArm {
                expression: &$life ResolvedExpr,
                arms: &$life [ResolvedMatchArm],
                index: usize,
                outer: BTreeMap<ValueId, ValidationBinding>,
                outer_ids: Vec<ValueId>,
                arm_scopes: Vec<BTreeMap<ValueId, ValidationBinding>>,
                result: Option<($ty, OwnershipMode)>,
                path: &$life str,
            },
        }
        };
    }
    frame!(HistoricalFrame, 'e, HistoricalResolvedType);
    frame!(CurrentFrame, 'e, ResolvedType);

    // The genuine historical whole frame keeps its original exact contract.
    // No old receiver is rebound to a different implementation or repr padding.
    const _: () = {
        assert!(std::mem::size_of::<HistoricalDeclarationId>() == 24);
        assert!(std::mem::size_of::<HistoricalFrame<'static>>() == super::super::VALIDATOR_FRAME_BYTES);
    };
    pub(super) const fn check<T>() {
        use std::mem::{align_of, size_of};
        // Every variant and field in CurrentFrame mirrors the actual local Frame.
        assert!(size_of::<T>() == size_of::<CurrentFrame<'static>>());
        assert!(align_of::<T>() == align_of::<CurrentFrame<'static>>());
        // The maximum match continuation owns exactly one result payload. The
        // exact equality fails if another variant/padding changes the maximum.
        assert!(size_of::<T>() + size_of::<Option<(HistoricalResolvedType, OwnershipMode)>>()
            == size_of::<HistoricalFrame<'static>>() + size_of::<Option<(ResolvedType, OwnershipMode)>>());
        assert!(align_of::<HistoricalFrame<'static>>() == align_of::<CurrentFrame<'static>>());
        // Generic collections contain their element types behind stable headers;
        // the full current witness still checks every actual header in Frame.
        assert!(size_of::<Vec<HistoricalResolvedType>>() == size_of::<Vec<ResolvedType>>());
        assert!(align_of::<Vec<HistoricalResolvedType>>() == align_of::<Vec<ResolvedType>>());
        assert!(size_of::<BTreeSet<HistoricalDeclarationId>>() == size_of::<BTreeSet<DeclarationId>>());
        assert!(align_of::<BTreeSet<HistoricalDeclarationId>>() == align_of::<BTreeSet<DeclarationId>>());
        assert!(size_of::<T>() <= super::super::VALIDATOR_FRAME_BYTES);
    }
    #[test]
    fn historical_and_current_full_frames_keep_exact_typed_payload_equation_and_tariff() {
        check::<CurrentFrame<'static>>();
        use std::mem::{align_of, size_of};
        eprintln!("OPT891 validator_frame old={} current={} old_align={} current_align={} old_type={} current_type={} old_type_align={} current_type_align={} old_result={} current_result={} old_id={} current_id={} old_id_align={} current_id_align={} old_cow={} current_cow={} tariff={}",
            size_of::<HistoricalFrame<'static>>(), size_of::<CurrentFrame<'static>>(),
            align_of::<HistoricalFrame<'static>>(), align_of::<CurrentFrame<'static>>(),
            size_of::<HistoricalResolvedType>(), size_of::<ResolvedType>(),
            align_of::<HistoricalResolvedType>(), align_of::<ResolvedType>(),
            size_of::<Option<(HistoricalResolvedType, OwnershipMode)>>(),
            size_of::<Option<(ResolvedType, OwnershipMode)>>(),
            size_of::<HistoricalDeclarationId>(), size_of::<DeclarationId>(),
            align_of::<HistoricalDeclarationId>(), align_of::<DeclarationId>(),
            size_of::<std::borrow::Cow<'static, HistoricalResolvedType>>(),
            size_of::<call_signature::ResultType<'static>>(), super::super::VALIDATOR_FRAME_BYTES);
    }
    #[test]
    fn whole_saved_frame_and_type_sources_have_only_declared_layout_substitutions() {
        let old = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/declaration-tag-identity-20261018/validator-frame-abi/owned_iterative_oracle.rs.whole-before.txt"));
        let current = include_str!("owned_iterative_oracle.rs");
        let boundary = current.find("\n// The saved validator executes against live HIR.").unwrap();
        let restored_body = current[..boundary].replace(
            "        const { frame_layout::check::<Frame<'static>>() };",
            "        const { assert!(std::mem::size_of::<Frame<'static>>() == super::VALIDATOR_FRAME_BYTES) };");
        assert_eq!(restored_body.trim_end(), old.trim_end(), "entire saved behavior and dispatch remain original");
        let old_start = old.find("        enum Frame<'e> {").unwrap();
        let old_end = old[old_start..].find("\n\n        const { assert!").unwrap() + old_start;
        let actual_start = current.find("        enum Frame<'e> {").unwrap();
        let actual_end = current[actual_start..].find("\n\n        const { frame_layout::check").unwrap() + actual_start;
        assert_eq!(&current[actual_start..actual_end], &old[old_start..old_end]);
        let pattern = current.find("        enum $name<$life> {").unwrap();
        let end = current[pattern..].find("\n        };\n    }").unwrap() + pattern;
        let restored = current[pattern..end].replace("enum $name<$life>", "enum Frame<'e>")
            .replace("std::borrow::Cow<$life, $ty>", "call_signature::ResultType<'e>")
            .replace("$life", "'e").replace("$ty", "ResolvedType");
        assert_eq!(restored.trim_end(), old[old_start..old_end].trim_end());
        let original = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/declaration-tag-identity-20261018/validator-frame-abi/resolved-type-530073.txt"));
        let start = current.find("    enum HistoricalResolvedType {").unwrap();
        let end = current[start..].find("\n    macro_rules! frame").unwrap() + start;
        let restored = current[start..end].trim_end().replace("    enum HistoricalResolvedType", "pub enum ResolvedType")
            .replace("HistoricalDeclarationId", "DeclarationId").replace("HistoricalResolvedType", "ResolvedType");
        assert_eq!(restored, original.trim_end());
        let identity = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/declaration-tag-identity-20261018/validator-frame-abi/identity-layout-530073.txt"));
        let declaration = "    struct HistoricalDeclarationId(HistoricalDeclarationText);";
        assert!(current.contains(declaration));
        assert!(identity.contains("pub struct DeclarationId(DeclarationText);"));
        let start = current.find("    enum HistoricalDeclarationText {").unwrap();
        let end = current[start..].find("\n    }").unwrap() + start + 6;
        let restored = current[start..end].lines().map(|line| line.strip_prefix("    ").unwrap_or(line))
            .collect::<Vec<_>>().join("\n").replace("HistoricalDeclarationText", "DeclarationText")
            .replace("std::sync::Arc", "Arc");
        let start = identity.find("enum DeclarationText {").unwrap();
        assert_eq!(restored, identity[start..].trim_end());
    }
}
