//! Complete saved pre-view frame layout; no estimated variant layout.
#![allow(dead_code)]
use crate::cleanup_plan::replay::continuation_storage::OwnedStep;
use crate::cleanup_plan::replay::*;
enum SavedFrame<'a> {
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

pub(super) fn bytes() -> usize {
    std::mem::size_of::<SavedFrame<'_>>()
}
