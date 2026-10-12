//! Invocation-held empty continuation backing; never HIR proof or a quota.
use super::*;
pub(super) const FRAME_CAPACITY: usize = 516;
pub(super) struct Storage<'hir> {
    program: &'hir ResolvedProgram,
    pub(super) frames: Option<Vec<Frame<'hir>>>,
    pub(super) leased: bool,
}
impl<'hir> Storage<'hir> {
    pub(super) fn new(program: &'hir ResolvedProgram) -> Self {
        Self {
            program,
            frames: None,
            leased: false,
        }
    }
    pub(super) fn matches(&self, program: &dyn crate::cleanup::ReplayContext) -> bool {
        program
            .full_program()
            .is_some_and(|current| std::ptr::eq(current, self.program))
    }
}
/// Same old lazy requested layout for a missing top-level lease or an active
/// nested guard. Caller reserves before allocation; no pool directory exists.
pub(super) fn fresh<'hir>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<Frame<'hir>>, Diagnostic> {
    budget.reserve_array::<Frame<'hir>>(
        function,
        FRAME_CAPACITY,
        "typed HIR continuation stack",
    )?;
    Ok(Vec::with_capacity(FRAME_CAPACITY))
}
/// An owning traversal's Step assignment: the match, its renewed binding and
/// the Yield arm block that evaluates the certified Next tail.
pub(super) type OwnedStep<'a> = Option<(
    &'a ResolvedExpr,
    &'a crate::hir::ResolvedBinding,
    &'a ResolvedExpr,
)>;
pub(super) enum Frame<'a> {
    Eval(&'a ResolvedExpr),
    FinishCopyRegion {
        previous: Option<ExpressionId>,
    },
    RenewalPrefix(&'a ResolvedExpr),
    UpcastPassthrough,
    Unary {
        expression: &'a ResolvedExpr,
        op: UnaryOp,
    },
    BinaryLeft {
        expression: &'a ResolvedExpr,
        op: BinaryOp,
        right: &'a ResolvedExpr,
    },
    BinaryRight {
        expression: &'a ResolvedExpr,
        op: BinaryOp,
        left_paths: Vec<ExprSkeletonPath>,
    },
    LazyRight {
        expression: &'a ResolvedExpr,
        op: BinaryOp,
        left: &'a ResolvedExpr,
        left_paths: Vec<ExprSkeletonPath>,
    },
    CallArgument {
        expression: &'a ResolvedExpr,
        params: Vec<crate::hir::ResolvedParam>,
        args: &'a [ResolvedExpr],
        index: usize,
        states: Vec<CallSkeletonState>,
    },
    ByteRangeArgument {
        expression: &'a ResolvedExpr,
        index: usize,
        states: Vec<CallSkeletonState>,
    },
    NativeArgument {
        args: &'a [ResolvedExpr],
        index: usize,
        paths: Vec<ExprSkeletonPath>,
    },
    BlockValue {
        expression: &'a ResolvedExpr,
        statements: &'a [ResolvedStatement],
        tail: &'a ResolvedExpr,
        index: usize,
        paths: Vec<ExprSkeletonPath>,
    },
    BlockTail {
        expression: &'a ResolvedExpr,
        paths: Vec<ExprSkeletonPath>,
    },
    WhileAfterCondition {
        prefixes: Vec<ExprSkeletonPath>,
        expression: &'a ResolvedExpr,
        statements: &'a [ResolvedStatement],
        tail: &'a ResolvedExpr,
        index: usize,
        condition: &'a ResolvedExpr,
        body: &'a ResolvedExpr,
    },
    WhileAfterBody {
        expression: &'a ResolvedExpr,
        statements: &'a [ResolvedStatement],
        tail: &'a ResolvedExpr,
        index: usize,
        results: Vec<ExprSkeletonPath>,
        true_prefixes: Vec<ExprSkeletonPath>,
        false_prefixes: Vec<ExprSkeletonPath>,
    },
    LexicalReturn {
        expression: &'a ResolvedExpr,
    },
    ContinueAdvance {
        expression: &'a ResolvedExpr,
        step: OwnedStep<'a>,
    },
    LexicalLoopCondition {
        prefixes: Vec<ExprSkeletonPath>,
        expression: &'a ResolvedExpr,
        statements: &'a [ResolvedStatement],
        tail: &'a ResolvedExpr,
        index: usize,
        condition: &'a ResolvedExpr,
        body: &'a ResolvedExpr,
        latch: &'a crate::hir::LoopLatchKind,
    },
    LexicalLoopBody {
        expression: &'a ResolvedExpr,
        statements: &'a [ResolvedStatement],
        tail: &'a ResolvedExpr,
        index: usize,
        results: Vec<ExprSkeletonPath>,
        true_prefixes: Vec<ExprSkeletonPath>,
        false_prefixes: Vec<ExprSkeletonPath>,
        condition: &'a ResolvedExpr,
    },
    VariantField {
        expression: &'a ResolvedExpr,
        case: &'a DeclarationId,
        fields: &'a [crate::hir::ResolvedFieldInitializer],
        index: usize,
        paths: Vec<ExprSkeletonPath>,
    },
    RecordField {
        expression: &'a ResolvedExpr,
        fields: &'a [crate::hir::ResolvedFieldInitializer],
        index: usize,
        paths: Vec<ExprSkeletonPath>,
    },
    UpdateBase {
        expression: &'a ResolvedExpr,
        base: &'a ResolvedExpr,
        record: &'a DeclarationId,
        fields: &'a [crate::hir::ResolvedFieldInitializer],
    },
    UpdateField {
        expression: &'a ResolvedExpr,
        base: &'a ResolvedExpr,
        record: &'a DeclarationId,
        fields: &'a [crate::hir::ResolvedFieldInitializer],
        index: usize,
        paths: Vec<ExprSkeletonPath>,
        replaced: BTreeSet<DeclarationId>,
        needs_cleanup: bool,
    },
    Try {
        expression: &'a ResolvedExpr,
    },
    TryOption {
        expression: &'a ResolvedExpr,
    },
    Project {
        expression: &'a ResolvedExpr,
        field: &'a DeclarationId,
    },
    IfCondition {
        expression: &'a ResolvedExpr,
        condition: &'a ResolvedExpr,
        then_branch: &'a ResolvedExpr,
        else_branch: &'a ResolvedExpr,
    },
    IfThen {
        expression: &'a ResolvedExpr,
        else_branch: &'a ResolvedExpr,
        true_prefixes: Vec<ExprSkeletonPath>,
        false_prefixes: Vec<ExprSkeletonPath>,
        results: Vec<ExprSkeletonPath>,
    },
    IfElse {
        expression: &'a ResolvedExpr,
        false_prefixes: Vec<ExprSkeletonPath>,
        results: Vec<ExprSkeletonPath>,
    },
    ScalarGuard(scalar_match::Guard<'a>),
    ScalarValue(scalar_match::Value<'a>),
    MatchScrutinee {
        expression: &'a ResolvedExpr,
        scrutinee: &'a ResolvedExpr,
        arms: &'a [ResolvedMatchArm],
    },
    MatchArm {
        expression: &'a ResolvedExpr,
        scrutinee: &'a ResolvedExpr,
        arms: &'a [ResolvedMatchArm],
        index: usize,
        remaining: Vec<ExprSkeletonPath>,
        results: Vec<ExprSkeletonPath>,
        is_record: bool,
    },
}

#[cfg(test)]
mod tests;
