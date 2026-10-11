use super::*;
use crate::cleanup_plan::{
    BlockId, CleanupResultSource, ExitTargetId, FinalizeAction, LexicalControlPlan, StatusLane,
    StatusSourceId,
};

fn slot(name: &str) -> ByteSlot {
    ByteSlot {
        place: CleanupPlace {
            storage: StorageId::ProvisionalResult,
            projections: Vec::new(),
        },
        value: format!("value_{name}"),
        flag: format!("flag_{name}"),
        kind: OwnedLeafKind::Bytes,
    }
}

fn function_ids() -> (DeclarationId, ExpressionId, ExpressionId) {
    let source = crate::parse(
        "module test.native_terminal_exits; fn probe(value: i64) -> i64 { value } fn main() -> i64 { 0 }",
        std::path::Path::new("native-terminal-exits.spx"),
    )
    .unwrap();
    let resolved = crate::hir::resolve(&source).unwrap();
    let probe = resolved
        .functions
        .iter()
        .find(|function| function.name == "probe")
        .unwrap();
    let main = resolved
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    (probe.id.clone(), probe.body.id.clone(), main.body.id.clone())
}

fn flags() -> BTreeMap<LivenessFlagId, ByteSlot> {
    BTreeMap::from([(LivenessFlagId(0), slot("a")), (LivenessFlagId(1), slot("b"))])
}

fn exit(id: u32, order: &[u32], continuation: ExitContinuation) -> ExitTarget {
    ExitTarget {
        id: ExitTargetId(id),
        from: BlockId(id),
        leaves_regions: Vec::new(),
        finalize_in_order: order
            .iter()
            .map(|flag| FinalizeAction {
                source: CleanupPlace {
                    storage: StorageId::ProvisionalResult,
                    projections: Vec::new(),
                },
                lifecycle_id: DeclarationId::new(crate::cleanup::BYTES_DROP_LIFECYCLE_ID),
                guard_flag: LivenessFlagId(*flag),
                active_case: None,
            })
            .collect(),
        continuation,
    }
}

fn failure(expression: &ExpressionId) -> ExitContinuation {
    ExitContinuation::ReturnFailure {
        source: StatusSourceId {
            expression: expression.clone(),
            lane: StatusLane::OperationFailure,
        },
    }
}

fn commit(expression: &ExpressionId) -> ExitContinuation {
    ExitContinuation::CommitResult {
        source: CleanupResultSource::Scalar {
            expression: expression.clone(),
        },
    }
}

fn lexical(success: &[u32]) -> Option<Box<LexicalControlPlan>> {
    Some(Box::new(LexicalControlPlan {
        schema: "semaprax.lexical-control-flow.v1",
        loops: Vec::new(),
        exits: Vec::new(),
        resumes: Vec::new(),
        abrupt_only_regions: Vec::new(),
        function_success_exits: success.iter().copied().map(ExitTargetId).collect(),
        execution_boundaries: Vec::new(),
    }))
}

fn plan(exits: Vec<ExitTarget>, control: Option<Box<LexicalControlPlan>>) -> CleanupPlan {
    let mut plan = CleanupPlan::unresolved();
    plan.exits = exits;
    plan.lexical_control = control;
    plan
}

#[test]
fn finalizer_order_accepts_reordered_subset_before_superset() {
    let a = slot("a");
    let b = slot("b");
    assert_eq!(
        canonical_finalizer_order(&[vec![a.clone()], vec![b.clone(), a.clone()]]).unwrap(),
        Some(vec![b, a])
    );
}

#[test]
fn finalizer_order_rejects_contradictory_exit_precedence() {
    let a = slot("a");
    let b = slot("b");
    assert_eq!(
        canonical_finalizer_order(&[vec![a.clone(), b.clone()], vec![b, a],]).unwrap(),
        None
    );
}

#[test]
fn consistent_exits_retain_one_shared_epilogue_order() {
    let (function, failed, result) = function_ids();
    let plan = plan(
        vec![
            exit(0, &[0], failure(&failed)),
            exit(1, &[1, 0], commit(&result)),
        ],
        None,
    );
    let order = terminal_order(&function, &plan, &flags()).unwrap();
    assert!(!order.exit_selected);
    assert_eq!(order.shared, [slot("b"), slot("a")]);
    assert_eq!(order.slots, BTreeSet::from([slot("a"), slot("b")]));
}

#[test]
fn nonterminal_exit_orders_never_join_the_terminal_union() {
    let (function, failed, result) = function_ids();
    let plan = plan(
        vec![
            exit(0, &[0, 1], failure(&failed)),
            exit(1, &[1, 0], ExitContinuation::Continue(crate::cleanup_plan::EdgeId(0))),
            exit(2, &[0, 1], commit(&result)),
        ],
        None,
    );
    let order = terminal_order(&function, &plan, &flags()).unwrap();
    assert!(!order.exit_selected);
    assert_eq!(order.shared, [slot("a"), slot("b")]);
}

#[test]
fn source_selected_lexical_exits_keep_each_exact_contradictory_order() {
    let (function, failed, result) = function_ids();
    let plan = plan(
        vec![
            exit(0, &[0, 1], failure(&failed)),
            exit(1, &[1, 0], commit(&result)),
        ],
        lexical(&[1]),
    );
    let order = terminal_order(&function, &plan, &flags()).unwrap();
    assert!(order.exit_selected);
    assert!(order.shared.is_empty());
    assert_eq!(order.slots, BTreeSet::from([slot("a"), slot("b")]));
    // The exact vectors themselves are untouched for the per-site router.
    assert_eq!(plan.exits[0].finalize_in_order[0].guard_flag, LivenessFlagId(0));
    assert_eq!(plan.exits[1].finalize_in_order[0].guard_flag, LivenessFlagId(1));
}

#[test]
fn contradictory_shared_epilogue_names_function_exits_and_slots() {
    let (function, failed, result) = function_ids();
    let plan = plan(
        vec![
            exit(0, &[0, 1], failure(&failed)),
            exit(1, &[1, 0], commit(&result)),
        ],
        None,
    );
    let error = terminal_order(&function, &plan, &flags()).unwrap_err();
    assert_eq!(error.code, "SPX-B104");
    assert!(error
        .message
        .starts_with("Bytes cleanup exits contain contradictory finalizer precedence"));
    for expected in [
        format!("function `{function}`"),
        format!("exit 0 (failure from `{failed}` OperationFailure) finalizes `flag_a`"),
        "before `flag_b`".to_owned(),
        "exit 1 (result commit) finalizes them in the opposite order".to_owned(),
        "no lexical exit selection".to_owned(),
    ] {
        assert!(error.message.contains(&expected), "{expected}: {error:?}");
    }
}

#[test]
fn hostile_ambiguous_failure_selection_still_refuses_contradiction() {
    let (function, failed, result) = function_ids();
    let plan = plan(
        vec![
            exit(0, &[0, 1], failure(&failed)),
            exit(1, &[1, 0], failure(&failed)),
            exit(2, &[], commit(&result)),
        ],
        lexical(&[2]),
    );
    let error = terminal_order(&function, &plan, &flags()).unwrap_err();
    assert!(error.message.contains("exit 0 (failure from"), "{error:?}");
    assert!(error
        .message
        .contains("is not exactly selected at its source site"));
}

#[test]
fn hostile_uncertified_commit_still_refuses_contradiction() {
    let (function, failed, result) = function_ids();
    let certified: [&[u32]; 3] = [&[], &[0], &[1, 1]];
    for success in certified {
        let plan = plan(
            vec![
                exit(0, &[0, 1], failure(&failed)),
                exit(1, &[1, 0], commit(&result)),
            ],
            lexical(success),
        );
        let error = terminal_order(&function, &plan, &flags()).unwrap_err();
        assert!(
            error
                .message
                .contains("exit 1 (result commit) is not exactly selected"),
            "{success:?}: {error:?}"
        );
    }
}
