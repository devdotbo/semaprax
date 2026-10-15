//! Authored controls: no compilation/runtime evidence is claimed by this file.
use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};
use std::cell::{Cell, RefCell};
const SOURCE: &str = include_str!("../../../../../tests/fixtures/nested-byte-slice-traversal.spx");
thread_local! {
    static REQUESTS: RefCell<Option<Vec<usize>>> = const { RefCell::new(None) };
    static FAILED_AT: Cell<Option<usize>> = const { Cell::new(None) };
}
pub(super) fn record_request(bytes: usize) {
    REQUESTS.with(|slot| {
        if let Some(requests) = slot.borrow_mut().as_mut() {
            assert!(
                requests.len() < requests.capacity(),
                "test request inventory is exact/preowned"
            );
            requests.push(bytes);
        }
    });
}
pub(super) fn record_failure() {
    FAILED_AT.with(|slot| {
        if slot.get().is_none() {
            slot.set(Some(current_metrics().allocated));
        }
    });
}
fn resolved(source: &str) -> ResolvedProgram {
    let ast = crate::check(source, "nested-bytes.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    program
}
fn function<'a>(program: &'a ResolvedProgram, name: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.name == name)
        .unwrap()
}
fn outer(function: &mut ResolvedFunction) -> &mut ResolvedExpr {
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("function block")
    };
    statements
        .iter_mut()
        .find_map(|statement| match statement {
            ResolvedStatement::Let { binding, value, .. } if binding.name == "#for-bytes" => {
                Some(value)
            }
            _ => None,
        })
        .unwrap()
}
fn authored(region: &mut ResolvedExpr) -> &mut Vec<ResolvedStatement> {
    let ResolvedExprKind::Block { statements, .. } = &mut region.kind else {
        panic!("region")
    };
    let (ResolvedStatement::While { body, .. } | ResolvedStatement::LexicalWhile { body, .. }) =
        &mut statements[2]
    else {
        panic!("loop")
    };
    let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("steps")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[1] else {
        panic!("authored")
    };
    let ResolvedExprKind::Block { statements, .. } = &mut value.kind else {
        panic!("authored block")
    };
    statements
}
fn inner(region: &mut ResolvedExpr) -> &mut ResolvedExpr {
    authored(region)
        .iter_mut()
        .find_map(|statement| match statement {
            ResolvedStatement::Let { binding, value, .. } if binding.name == "#for-bytes" => {
                Some(value)
            }
            _ => None,
        })
        .unwrap()
}
#[test]
fn nested_byte_full_source_hir_graph_and_cache_replay_preserves_v1_receiver_independence() {
    let ast = crate::check(SOURCE, "nested-bytes.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    assert_eq!(
        crate::format::canonical(&crate::check(&canonical, "nested-bytes.spx").unwrap()),
        canonical
    );
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&ast, &graph).unwrap();
    let program = resolved(SOURCE);
    let wire = crate::cache_codec::encode(&program).unwrap();
    let decoded: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    crate::hir::validate(&decoded).unwrap();
    crate::loan_plan::validate_program(&decoded).unwrap();
    assert_eq!(crate::cache_codec::encode(&decoded).unwrap(), wire);
    let sum = function(&program, "sum");
    assert!(super::super::before_nested::validate_function(&program.declarations, sum).is_err());
    let manual = function(&program, "manual");
    assert_eq!(
        super::super::before_nested::validate_function(&program.declarations, manual),
        super::super::validate_function(&program.declarations, manual)
    );
    // A cache or context clone is a different immutable invocation member.
    let census = Census::optional(&program, sum).unwrap().unwrap();
    let row = census.rows[0].shape.root;
    let first = census.region(&decoded, sum, row).err().unwrap();
    assert!(first.message.contains("function differs"));
    let foreign_function = function(&decoded, "sum");
    assert!(Census::optional(&program, foreign_function).is_err());
}
#[test]
fn nested_byte_ancestor_freezes_cover_every_inner_subtree_and_same_root_after_inner_exit() {
    let source = SOURCE.replace("for inner in right { total", "for inner in left { total");
    let program = resolved(&source);
    let sum = function(&program, "sum");
    let census = Census::optional(&program, sum).unwrap().unwrap();
    let outer = census.rows.iter().min_by_key(|row| row.start).unwrap();
    let inner = census.rows.iter().max_by_key(|row| row.start).unwrap();
    let freezes = sum
        .loan_plan
        .loans
        .iter()
        .filter(|loan| loan.cause == crate::loan_plan::LoanCause::TraversalFreeze)
        .collect::<Vec<_>>();
    assert_eq!(freezes.len(), 2);
    assert_eq!(freezes[0].origin.root, freezes[1].origin.root);
    let outer_loan = freezes
        .iter()
        .find(|loan| loan.site == outer.shape.root.id)
        .unwrap();
    let inner_loan = freezes
        .iter()
        .find(|loan| loan.site == inner.shape.root.id)
        .unwrap();
    assert_ne!(outer_loan.id, inner_loan.id);
    let after_inner = sum
        .loan_plan
        .endpoints
        .iter()
        .find(|endpoint| {
            endpoint.point.expression == inner.shape.root.id
                && endpoint.point.phase == crate::loan_plan::LoanPointPhase::After
        })
        .unwrap();
    assert!(after_inner.live_after.contains(&outer_loan.id));
    let after_outer = sum
        .loan_plan
        .endpoints
        .iter()
        .find(|endpoint| {
            endpoint.point.expression == outer.shape.root.id
                && endpoint.point.phase == crate::loan_plan::LoanPointPhase::After
        })
        .unwrap();
    assert!(after_outer.live_before.contains(&outer_loan.id));
    for id in outer.shape.frozen {
        let mut forged = program.clone();
        let sum = forged
            .functions
            .iter_mut()
            .find(|function| function.name == "sum")
            .unwrap();
        let owner = FunctionExecutionId::Monomorphic(sum.id.clone());
        let mut binding = shape::shape(super::tests::outer(sum))
            .unwrap()
            .unwrap()
            .payload
            .clone();
        binding.id = id.clone();
        let value = ResolvedExpr {
            id: ExpressionId::new(&owner, "forged.outer.freeze.assignment"),
            ty: binding.ty.clone(),
            ownership: binding.ownership,
            kind: ResolvedExprKind::Uint8(0),
            span: binding.span,
        };
        let span = binding.span;
        authored(super::tests::inner(super::tests::outer(sum))).push(ResolvedStatement::Assign {
            binding,
            field: None,
            value,
            span,
        });
        let error = super::super::validate_function(&forged.declarations, function(&forged, "sum"))
            .unwrap_err();
        assert!(
            error.message.contains("mutated or rebound"),
            "{}",
            error.message
        );
        assert!(crate::hir::validate(&forged).is_err());
    }
}
#[test]
fn nested_byte_census_uses_exact_owner_and_first_refusal_on_foreign_then_restored_scope() {
    let program = resolved(SOURCE);
    let sum = function(&program, "sum");
    let (((), metrics), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let census = Census::optional(&program, sum).unwrap().unwrap();
        let root = census.rows[0].shape.root;
        measure(|| {
            let (error, child_refused, child_used, child_first) =
                with_limit_usage_refusal(0, || census.region(&program, sum, root).err().unwrap());
            assert!(!child_refused);
            assert_eq!(child_used, 0);
            assert_eq!(child_first, None);
            assert!(error.message.contains("detached"));
            drop(error);
            let restored = census.region(&program, sum, root).err().unwrap();
            assert!(restored.message.contains("detached"));
            drop(restored);
            drop(census);
        })
    });
    assert!(refused);
    let first = first.unwrap();
    assert_eq!((first.requested, first.floor, first.stage), (0, 0, None));
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocated, 0);
}
#[test]
fn nested_byte_census_funds_exact_short_zero_every_frontier_sticky_and_complete_drop() {
    let program = resolved(SOURCE);
    let sum = function(&program, "sum");
    // The recorder is test instrumentation, preowned before the measured route.
    // Its bound follows actual expression/statement shape, not a payment margin.
    let mut expressions = 0usize;
    let mut regions = 0usize;
    let mut bindings = sum.params.len();
    super::super::walk(&sum.body, |expression, authorized| {
        expressions += 1;
        regions += usize::from(shape::shape(expression)?.is_some());
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            bindings += statements.len();
        }
        Ok(authorized)
    })
    .unwrap();
    REQUESTS.with(|slot| {
        *slot.borrow_mut() = Some(Vec::with_capacity(
            64 + expressions * 2 + bindings * 2 + regions * 21,
        ))
    });
    let (((), metrics), refused, exact, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let census = Census::optional(&program, sum).unwrap().unwrap();
            for row in &census.rows {
                assert!(census
                    .region(&program, sum, row.shape.root)
                    .unwrap()
                    .is_some());
            }
            drop(census);
        })
    });
    let requests = REQUESTS.with(|slot| slot.borrow_mut().take().unwrap());
    assert!(!refused);
    assert_eq!(first, None);
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated <= exact);
    assert_eq!(requests.iter().sum::<usize>(), exact);
    let mut cumulative = 0usize;
    let mut caps = vec![0, exact - 1, exact];
    for request in requests {
        cumulative += request;
        if request != 0 {
            caps.push(cumulative - 1);
        }
    }
    caps.sort_unstable();
    caps.dedup();
    for cap in caps {
        FAILED_AT.with(|slot| slot.set(None));
        let ((failed, metrics), refused, used, first) = with_limit_usage_refusal(cap, || {
            measure(|| {
                let result = Census::optional(&program, sum);
                let failed = result.is_err();
                drop(result);
                failed
            })
        });
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        if cap == exact {
            assert!(!failed);
            assert!(!refused);
            assert_eq!(used, exact);
            assert_eq!(first, None);
        } else {
            assert!(failed);
            assert!(refused);
            assert!(first.is_some());
            assert_eq!(
                metrics.allocated,
                FAILED_AT.with(Cell::get).unwrap(),
                "no allocation after first refusal including error/partial owner Drop"
            );
        }
    }
    let ((failed, metrics), refused, used, first) = with_limit_usage_refusal(0, || {
        assert!(!reserve_active_required(1));
        measure(|| {
            let result = Census::optional(&program, sum);
            let failed = result.is_err();
            drop(result);
            failed
        })
    });
    assert!(failed && refused);
    assert_eq!(used, 0);
    assert_eq!(first.unwrap().requested, 1);
    assert_eq!(metrics.allocated, 0);
}
