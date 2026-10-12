//! Exact-capacity and first-overflow evidence for Shared Loan Plan v1.
//!
//! These fixtures operate on resolved, typed HIR and always retain a real
//! own-root loan. Point/edge padding uses disconnected Boolean preconditions.
//! The checked-work boundary instead places Boolean statements before real
//! last uses of every loan, so the production planner performs the measured
//! reachability and live-edge work.

use std::path::Path;

use crate::ast::Span;
use crate::hir::{
    FunctionExecutionId, PatternValue, ResolvedBinding, ResolvedExpr, ResolvedExprKind,
    ResolvedFunction, ResolvedMatchArm, ResolvedMatchMode, ResolvedMatchPattern, ResolvedStatement,
    ResolvedType, ValueId,
};

use super::*;

pub(super) fn fixture(loan_count: usize) -> (ResolvedProgram, usize) {
    fixture_with_uses(loan_count, false)
}

pub(super) fn fixture_with_uses(
    loan_count: usize,
    retain_last_uses: bool,
) -> (ResolvedProgram, usize) {
    let source = boundary_source(loan_count, retain_last_uses);
    let ast = crate::parse(&source, Path::new("loan-plan-boundaries.spx")).unwrap();
    assert!(crate::verify::verify(&ast).is_empty());
    let program = crate::hir::resolve(&ast).unwrap();
    let index = program
        .functions
        .iter()
        .position(|function| function.id.as_str() == "loan.boundary")
        .unwrap();
    (program, index)
}

fn boundary_source(loan_count: usize, retain_last_uses: bool) -> String {
    let mut source = String::from(
        "module test.loan_plan_boundaries;\n\
         @id(\"loan.boundary\")\n\
         fn boundary(input: borrow Slice<u8>) -> i64 {\n\
         let owned = bytes_copy(input);\n",
    );
    for index in 0..loan_count {
        source.push_str(&format!(
            "let boundary_view_{index} = bytes_as_slice(owned);\n"
        ));
    }
    if retain_last_uses {
        for index in 0..loan_count {
            source.push_str(&format!(
                "let boundary_use_{index} = byte_len(boundary_view_{index});\n"
            ));
        }
    }
    source.push_str("0\n}\n@id(\"app.main\") fn main() -> i64 { 0 }\n");
    source
}

/// Keep the source overflow refusal and the detached 257-loan planner corpus.
/// The extra view has fresh binding/expression identities and the same own root.
pub(super) fn first_loan_overflow() -> (ResolvedProgram, usize, ResolvedFunction) {
    let text = boundary_source(WORK_ROOT_LOANS, true).replace(
        "0\n}\n@id",
        "let boundary_extra = bytes_as_slice(owned);\n0\n}\n@id",
    );
    let ast = crate::parse(&text, Path::new("loan-plan-overflow.spx")).unwrap();
    assert!(crate::verify::verify(&ast).is_empty());
    let errors = crate::hir::resolve(&ast).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("function exceeds 256 shared loans")),
        "{errors:?}"
    );

    let (program, index) = fixture_with_uses(WORK_ROOT_LOANS, true);
    let mut function = program.functions[index].clone();
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("boundary fixture body remains a block")
    };
    let mut extra = statements[1].clone();
    let ResolvedStatement::Let { binding, value, .. } = &mut extra else {
        panic!("first view remains a let")
    };
    binding.id = ValueId::local(&execution, "overflow.extra.binding");
    binding.name = "boundary_extra".to_string();
    value.id = ExpressionId::new(&execution, "overflow.extra.call");
    let ResolvedExprKind::Call { args, .. } = &mut value.kind else {
        panic!("first view remains a call")
    };
    assert_eq!(args.len(), 1);
    args[0].id = ExpressionId::new(&execution, "overflow.extra.root");
    statements.push(extra);
    (program, index, function)
}

fn padding_statement(
    function: &ResolvedFunction,
    path: &str,
    value: ResolvedExpr,
) -> ResolvedStatement {
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    ResolvedStatement::Let {
        binding: ResolvedBinding {
            id: ValueId::local(&execution, &format!("{path}.binding")),
            name: format!("__loan_work_{}", path.replace('.', "_")),
            ownership: OwnershipMode::Value,
            ty: ResolvedType::Bool,
            span: Span::default(),
        },
        mutable: false,
        value,
        span: Span::default(),
    }
}

pub(super) fn add_live_padding(
    function: &mut ResolvedFunction,
    loan_count: usize,
    prefix: &str,
    leaves: usize,
    branches: usize,
    matches: usize,
) {
    let mut padding = Vec::new();
    for index in 0..leaves {
        let path = format!("{prefix}.leaf.{index}");
        padding.push(padding_statement(
            function,
            &path,
            bool_leaf(function, &path),
        ));
    }
    for index in 0..branches {
        let path = format!("{prefix}.branch.{index}");
        padding.push(padding_statement(
            function,
            &path,
            branch_root(function, &path),
        ));
    }
    for index in 0..matches {
        let path = format!("{prefix}.match.{index}");
        padding.push(padding_statement(
            function,
            &path,
            three_arm_match_root(function, &path),
        ));
    }
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("boundary fixture body remains a block")
    };
    let before_last_uses = 1 + loan_count;
    statements.splice(before_last_uses..before_last_uses, padding);
}

fn expression_id(function: &ResolvedFunction, path: &str) -> ExpressionId {
    ExpressionId::new(&FunctionExecutionId::Monomorphic(function.id.clone()), path)
}

fn bool_leaf(function: &ResolvedFunction, path: &str) -> ResolvedExpr {
    ResolvedExpr {
        id: expression_id(function, path),
        ty: ResolvedType::Bool,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Bool(true),
        span: Span::default(),
    }
}

fn branch_root(function: &ResolvedFunction, path: &str) -> ResolvedExpr {
    ResolvedExpr {
        id: expression_id(function, path),
        ty: ResolvedType::Bool,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::If {
            condition: Box::new(bool_leaf(function, &format!("{path}.condition"))),
            then_branch: Box::new(bool_leaf(function, &format!("{path}.then"))),
            else_branch: Box::new(bool_leaf(function, &format!("{path}.else"))),
        },
        span: Span::default(),
    }
}

fn three_arm_match_root(function: &ResolvedFunction, path: &str) -> ResolvedExpr {
    let arm = |suffix: &str, pattern| ResolvedMatchArm {
        pattern,
        guard: None,
        value: bool_leaf(function, &format!("{path}.arm.{suffix}")),
        span: Span::default(),
    };
    ResolvedExpr {
        id: expression_id(function, path),
        ty: ResolvedType::Bool,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Match {
            mode: ResolvedMatchMode::Value,
            scrutinee: Box::new(bool_leaf(function, &format!("{path}.scrutinee"))),
            arms: vec![
                arm(
                    "true",
                    ResolvedMatchPattern::Literal(PatternValue::Bool(true)),
                ),
                arm(
                    "false",
                    ResolvedMatchPattern::Literal(PatternValue::Bool(false)),
                ),
                arm("fallback", ResolvedMatchPattern::Wildcard),
            ],
        },
        span: Span::default(),
    }
}

pub(super) fn add_padding(
    function: &mut ResolvedFunction,
    prefix: &str,
    leaves: usize,
    branches: usize,
    matches: usize,
) {
    for index in 0..leaves {
        let expression = bool_leaf(function, &format!("{prefix}.leaf.{index}"));
        function.requires.push(expression);
    }
    for index in 0..branches {
        let expression = branch_root(function, &format!("{prefix}.branch.{index}"));
        function.requires.push(expression);
    }
    for index in 0..matches {
        let expression = three_arm_match_root(function, &format!("{prefix}.match.{index}"));
        function.requires.push(expression);
    }
}

pub(super) fn cfg_counts(function: &ResolvedFunction) -> (usize, usize) {
    let mut work = WorkCounter::new(usize::MAX);
    let cfg = build_cfg(function, &mut work).expect("boundary fixture CFG builds");
    (cfg.points.len(), cfg.edges.len())
}

fn install_plan(
    mut program: ResolvedProgram,
    index: usize,
    function: ResolvedFunction,
    plan: LoanPlan,
) -> ResolvedProgram {
    let mut function = function;
    function.loan_plan = plan;
    program.functions[index] = function;
    program
}

fn uncached_live_nodes(cfg: &Cfg<'_>, start: u16, seeds: &BTreeSet<u16>) -> BTreeSet<u16> {
    // The saved nested tables, rebuilt from the sorted edges, keep this
    // reference independent of the flat adjacency under test.
    let legacy = legacy_adjacency::LegacyAdjacency::from_edges(cfg.points.len(), &cfg.edges);
    let mut reachable = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(node) = pending.pop() {
        if reachable.insert(node) {
            pending.extend(legacy.successors[node as usize].iter().rev().copied());
        }
    }
    let mut live = BTreeSet::new();
    let mut pending = seeds
        .iter()
        .filter(|seed| reachable.contains(seed))
        .copied()
        .collect::<Vec<_>>();
    pending.push(start);
    while let Some(node) = pending.pop() {
        if !reachable.contains(&node) || !live.insert(node) || node == start {
            continue;
        }
        pending.extend(legacy.predecessors[node as usize].iter().rev().copied());
    }
    live
}

fn all_edge_liveness(cfg: &Cfg<'_>, live: &[BTreeSet<u16>]) -> work::EdgeLiveness {
    let mut edge_live = vec![Vec::new(); cfg.edges.len()];
    let mut termination_edges = vec![Vec::new(); live.len()];
    for (loan_index, nodes) in live.iter().enumerate() {
        let id = LoanId(loan_index as u16);
        for (edge_index, (from, to)) in cfg.edges.iter().copied().enumerate() {
            if nodes.contains(&from) && nodes.contains(&to) {
                edge_live[edge_index].push(id);
            } else if nodes.contains(&from) {
                termination_edges[loan_index].push(edge_index as u16);
            }
        }
    }
    (edge_live, termination_edges)
}

#[test]
fn batched_reachability_and_live_source_edges_preserve_canonical_proof() {
    let (program, index) = fixture(2);
    let function = &program.functions[index];
    let mut cfg_work = WorkCounter::new(usize::MAX);
    let cfg = build_cfg(function, &mut cfg_work).expect("fixture CFG builds");
    let start = cfg.node(&function.body, LoanPointPhase::Before).unwrap();
    let end = cfg.node(&function.body, LoanPointPhase::After).unwrap();
    let seeds = BTreeSet::from([end]);
    let expected = uncached_live_nodes(&cfg, start, &seeds);
    let mut work = WorkCounter::new(usize::MAX);
    let reachable = work::Reachability::build(&cfg, [start].into_iter(), &mut work).unwrap();
    let first = work::live_nodes(&cfg, LoanId(0), start, &seeds, &reachable, &mut work).unwrap();
    assert_eq!(first, expected);
    let live = vec![first, BTreeSet::from([start])];
    let expected_edges = all_edge_liveness(&cfg, &live);
    let before_edges = work.used;
    let actual_edges = work::edge_liveness(&cfg, &live, &mut work).unwrap();
    assert_eq!(actual_edges, expected_edges);
    assert!(work.used - before_edges < live.len() * cfg.edges.len());
}

#[test]
fn batched_masks_match_independent_traversals_at_every_word_boundary() {
    let (program, index) = fixture(64);
    let mut function = program.functions[index].clone();
    add_padding(&mut function, "batch.disconnected", 1, 1, 1);
    let cfg = build_cfg(&function, &mut WorkCounter::new(usize::MAX)).unwrap();
    let end = cfg.node(&function.body, LoanPointPhase::After).unwrap();
    // Distinct starts exceed the former eight-entry memo; disconnected roots,
    // branches and repeated starts exercise exact masks and unreachable seeds.
    for count in [1, 8, 9, 63, 64, 65, 127, 128, 129, 255, 256] {
        let starts = (0..count)
            .map(|index| (index % cfg.points.len()) as u16)
            .collect::<Vec<_>>();
        let mut work = WorkCounter::new(usize::MAX);
        let reachable = work::Reachability::build(&cfg, starts.iter().copied(), &mut work).unwrap();
        let seeds = BTreeSet::from([end, (cfg.points.len() - 1) as u16]);
        let mut live = Vec::new();
        for (loan, start) in starts.iter().copied().enumerate() {
            let nodes = work::live_nodes(
                &cfg,
                LoanId(loan as u16),
                start,
                &seeds,
                &reachable,
                &mut work,
            )
            .unwrap();
            assert_eq!(
                nodes,
                uncached_live_nodes(&cfg, start, &seeds),
                "loan {loan}/{count}"
            );
            live.push(nodes);
        }
        assert_eq!(
            work::edge_liveness(&cfg, &live, &mut work).unwrap(),
            all_edge_liveness(&cfg, &live)
        );
    }
}

#[test]
fn batched_forward_work_is_shared_and_cycles_preserve_start_barriers() {
    let (program, index) = fixture(128);
    let function = &program.functions[index];
    let mut cfg = build_cfg(function, &mut WorkCounter::new(usize::MAX)).unwrap();
    // A linear analysis-only CFG with 256 distinct starts exposes the shape
    // that an eight-start memo cannot share. This does not alter admission.
    cfg.edges = (0..cfg.points.len() - 1)
        .map(|node| (node as u16, (node + 1) as u16))
        .collect();
    cfg.adjacency = adjacency::Adjacency::build(cfg.points.len(), &cfg.edges).unwrap();
    let end = (cfg.points.len() - 1) as u16;
    let starts = (0..MAX_LOANS_PER_FUNCTION_V1 as u16).collect::<Vec<_>>();
    let mut batch_work = WorkCounter::new(usize::MAX);
    let _ = work::Reachability::build(&cfg, starts.iter().copied(), &mut batch_work).unwrap();
    let exact = batch_work.used;
    let mut reference_work = WorkCounter::new(usize::MAX);
    let _ =
        work::Reachability::uncached(&cfg, starts.iter().copied(), &mut reference_work).unwrap();
    assert!(exact < reference_work.used);
    let mut exact_work = WorkCounter::new(exact);
    work::Reachability::build(&cfg, starts.iter().copied(), &mut exact_work).unwrap();
    assert_eq!(exact_work.used, exact);
    let mut short_work = WorkCounter::new(exact - 1);
    let error = work::Reachability::build(&cfg, starts.iter().copied(), &mut short_work)
        .err()
        .unwrap();
    assert_eq!(short_work.used, exact);
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "loan analysis exceeds 1,000,000 checked work"
    );
    // A back edge challenges convergence. Reverse liveness must still stop at
    // each loan's own start even though all starts can now reach every point.
    // `end` is the largest source, so the back edge sorts last.
    cfg.edges.push((end, 0));
    cfg.adjacency = adjacency::Adjacency::build(cfg.points.len(), &cfg.edges).unwrap();
    let reachable =
        work::Reachability::build(&cfg, starts.iter().copied(), &mut batch_work).unwrap();
    for loan in [0, 63, 64, 127, 128, 255] {
        let seeds = BTreeSet::from([end]);
        assert_eq!(
            work::live_nodes(
                &cfg,
                LoanId(loan),
                starts[loan as usize],
                &seeds,
                &reachable,
                &mut batch_work
            )
            .unwrap(),
            uncached_live_nodes(&cfg, starts[loan as usize], &seeds),
        );
    }
}

#[test]
fn batched_forward_build_is_byte_exact_to_uncached_reachability_and_replays() {
    let (program, index) = fixture_with_uses(32, true);
    // Keep the authenticated source-derived cleanup carrier intact for full
    // Graph serialization; synthetic padding is covered by CFG tests above.
    let function = program.functions[index].clone();
    let plan = build_plan(&program, &function).unwrap();
    let mut reference_work = WorkCounter::new(usize::MAX);
    reference_work.uncached_reachability = true;
    let reference = build_cfg_plan_counted(&program, &function, &mut reference_work).unwrap();
    assert_eq!(plan, reference);
    let authenticated = install_plan(program.clone(), index, function.clone(), plan);
    validate_program(&authenticated).expect("batched plan independently replays");
    let reference = install_plan(program, index, function, reference);
    assert_eq!(
        crate::graph::to_hir_json(&authenticated, "batch.reference").unwrap(),
        crate::graph::to_hir_json(&reference, "batch.reference").unwrap()
    );
}

#[test]
fn exact_4096_program_points_rebuild_and_first_representable_overflow_fail_closed() {
    let (program, index) = fixture(1);
    let mut function = program.functions[index].clone();
    let (base_points, _) = cfg_counts(&function);
    assert_eq!(base_points % 2, 0);
    let leaves = (MAX_LOAN_ENDPOINTS_V1 - base_points) / 2;
    add_padding(&mut function, "point.boundary", leaves, 0, 0);

    let plan = build_plan(&program, &function).expect("4,096 points are admitted");
    assert_eq!(plan.endpoints.len(), MAX_LOAN_ENDPOINTS_V1);
    assert!(plan.edges.iter().any(|edge| {
        edge.from as usize == MAX_LOAN_ENDPOINTS_V1 - 2
            && edge.to as usize == MAX_LOAN_ENDPOINTS_V1 - 1
    }));
    assert_eq!(plan, build_plan(&program, &function).unwrap());
    let authenticated = install_plan(program.clone(), index, function.clone(), plan.clone());
    validate_program(&authenticated).expect("the exact-boundary carrier replays");

    let mut forged = authenticated;
    forged.functions[index]
        .loan_plan
        .endpoints
        .swap(0, MAX_LOAN_ENDPOINTS_V1 - 1);
    assert_eq!(validate_program(&forged).unwrap_err().code, "SPX-H006");

    add_padding(&mut function, "point.overflow", 1, 0, 0);
    let error = build_plan(&program, &function).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(error.message, "function exceeds 4,096 loan program points");
}

#[test]
fn exact_4096_cfg_edges_rebuild_and_edge_4097_fails_before_point_capacity() {
    let (program, index) = fixture(1);
    let mut function = program.functions[index].clone();
    let (base_points, base_edges) = cfg_counts(&function);
    let mut shape = None;
    for matches in 0..=((MAX_LOAN_EDGES_V1 - base_edges) / 11) {
        for branches in 0..=((MAX_LOAN_EDGES_V1 - base_edges - 11 * matches) / 8) {
            let leaves = MAX_LOAN_EDGES_V1 - base_edges - 11 * matches - 8 * branches;
            let points = base_points + 10 * matches + 8 * branches + 2 * leaves;
            if points <= MAX_LOAN_ENDPOINTS_V1 - 2 {
                shape = Some((leaves, branches, matches));
                break;
            }
        }
        if shape.is_some() {
            break;
        }
    }
    let (leaves, branches, matches) = shape.expect("an isolated exact-edge fixture exists");
    add_padding(&mut function, "edge.boundary", leaves, branches, matches);

    let plan = build_plan(&program, &function).expect("4,096 edges are admitted");
    assert_eq!(plan.edges.len(), MAX_LOAN_EDGES_V1);
    let exact_points = plan.endpoints.len();
    assert!(exact_points <= MAX_LOAN_ENDPOINTS_V1 - 2);
    assert_eq!(plan, build_plan(&program, &function).unwrap());
    let authenticated = install_plan(program.clone(), index, function.clone(), plan);
    validate_program(&authenticated).expect("the exact-edge carrier replays");

    let mut forged = authenticated;
    forged.functions[index]
        .loan_plan
        .edges
        .swap(0, MAX_LOAN_EDGES_V1 - 1);
    assert_eq!(validate_program(&forged).unwrap_err().code, "SPX-H006");

    add_padding(&mut function, "edge.overflow", 1, 0, 0);
    assert!(exact_points + 2 <= MAX_LOAN_ENDPOINTS_V1);
    let error = build_plan(&program, &function).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(error.message, "function exceeds 4,096 loan CFG edges");
}

// Each byte_len last use is itself a synchronous borrowed-call loan. Pair
// 128 own-root views with their 128 real last-use calls to exercise the
// exact 256-loan boundary without exceeding it before work measurement.
pub(super) const WORK_ROOT_LOANS: usize = MAX_LOANS_PER_FUNCTION_V1 / 2;

/// The checked-work boundary source: the program, the boundary function's
/// index, and the padded function that consumes exactly 1,000,000 work units.
/// Shared by the whole-planner oracle so both read one measured fixture.
pub(super) fn exact_million_work_boundary() -> (ResolvedProgram, usize, ResolvedFunction) {
    let (program, index) = fixture_with_uses(WORK_ROOT_LOANS, true);
    let base = program.functions[index].clone();
    let (base_result, base_work) = build_cfg_plan_with_work_limit(&program, &base, usize::MAX);
    base_result.expect("the unpadded boundary fixture builds");
    let (base_points, base_edges) = cfg_counts(&base);

    let measure_delta = |leaves, branches, matches| {
        let mut probe = base.clone();
        add_live_padding(
            &mut probe,
            WORK_ROOT_LOANS,
            "work.probe",
            leaves,
            branches,
            matches,
        );
        let (result, used) = build_cfg_plan_with_work_limit(&program, &probe, usize::MAX);
        result.expect("a one-shape work probe builds");
        let (points, edges) = cfg_counts(&probe);
        (used - base_work, points - base_points, edges - base_edges)
    };
    let (leaf_work, leaf_points, leaf_edges) = measure_delta(1, 0, 0);
    let (branch_work, branch_points, branch_edges) = measure_delta(0, 1, 0);
    let (match_work, match_points, match_edges) = measure_delta(0, 0, 1);
    let mut tuning_probe = base.clone();
    add_padding(&mut tuning_probe, "work.tuning.probe", 1, 0, 0);
    let (tuning_result, tuning_used) =
        build_cfg_plan_with_work_limit(&program, &tuning_probe, usize::MAX);
    tuning_result.expect("the residue-tuning work probe builds");
    let (tuning_points, tuning_edges) = cfg_counts(&tuning_probe);
    let tuning_work = tuning_used - base_work;
    let tuning_points = tuning_points - base_points;
    let tuning_edges = tuning_edges - base_edges;

    let mut shape = None;
    for matches in 0..=((MAX_LOAN_PLAN_WORK_V1 - base_work) / match_work) {
        let after_matches = base_work + matches * match_work;
        for branches in 0..=((MAX_LOAN_PLAN_WORK_V1 - after_matches) / branch_work) {
            let after_branches = after_matches + branches * branch_work;
            let max_live_leaves = (MAX_LOAN_PLAN_WORK_V1 - after_branches) / leaf_work;
            for live_leaves in (0..=max_live_leaves).rev() {
                let remaining = MAX_LOAN_PLAN_WORK_V1 - after_branches - live_leaves * leaf_work;
                if !remaining.is_multiple_of(tuning_work) {
                    continue;
                }
                let tuning_leaves = remaining / tuning_work;
                let points = base_points
                    + match_points * matches
                    + branch_points * branches
                    + leaf_points * live_leaves
                    + tuning_points * tuning_leaves;
                let edges = base_edges
                    + match_edges * matches
                    + branch_edges * branches
                    + leaf_edges * live_leaves
                    + tuning_edges * tuning_leaves;
                if points <= MAX_LOAN_ENDPOINTS_V1 - leaf_points
                    && edges <= MAX_LOAN_EDGES_V1 - leaf_edges
                {
                    shape = Some((live_leaves, branches, matches, tuning_leaves));
                    break;
                }
            }
            if shape.is_some() {
                break;
            }
        }
        if shape.is_some() {
            break;
        }
    }
    let (live_leaves, branches, matches, tuning_leaves) = shape.unwrap_or_else(|| {
        panic!(
            "a one-million-work fixture exists: base work/points/edges={base_work}/{base_points}/{base_edges}, live leaf={leaf_work}/{leaf_points}/{leaf_edges}, branch={branch_work}/{branch_points}/{branch_edges}, match={match_work}/{match_points}/{match_edges}, tuning leaf={tuning_work}/{tuning_points}/{tuning_edges}"
        )
    });
    let mut exact = base.clone();
    add_live_padding(
        &mut exact,
        WORK_ROOT_LOANS,
        "work.boundary",
        live_leaves,
        branches,
        matches,
    );
    add_padding(&mut exact, "work.tuning.boundary", tuning_leaves, 0, 0);
    (program, index, exact)
}

#[test]
fn exact_million_work_build_replays_and_the_first_extra_unit_is_fail_closed() {
    let (program, index, mut exact) = exact_million_work_boundary();
    let (plan, used) = build_cfg_plan_with_work_limit(&program, &exact, MAX_LOAN_PLAN_WORK_V1);
    let plan = plan.expect("exactly 1,000,000 work units are admitted");
    assert_eq!(used, MAX_LOAN_PLAN_WORK_V1);
    assert_eq!(plan.loans.len(), MAX_LOANS_PER_FUNCTION_V1);
    let authenticated = install_plan(program.clone(), index, exact.clone(), plan);
    validate_program(&authenticated).expect("the exact-work carrier replays");

    let (one_too_small, used) =
        build_cfg_plan_with_work_limit(&program, &exact, MAX_LOAN_PLAN_WORK_V1 - 1);
    let error = one_too_small.unwrap_err();
    assert_eq!(used, MAX_LOAN_PLAN_WORK_V1);
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "loan analysis exceeds 1,000,000 checked work"
    );

    add_live_padding(&mut exact, WORK_ROOT_LOANS, "work.overflow", 1, 0, 0);
    let (overflow, used) = build_cfg_plan_with_work_limit(&program, &exact, MAX_LOAN_PLAN_WORK_V1);
    let error = overflow.unwrap_err();
    assert_eq!(used, MAX_LOAN_PLAN_WORK_V1 + 1);
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "loan analysis exceeds 1,000,000 checked work"
    );
    let public_error = build_plan(&program, &exact).unwrap_err();
    assert_eq!(public_error.code, error.code);
    assert_eq!(public_error.message, error.message);
    assert!(public_error
        .help
        .unwrap()
        .contains(&format!("function @id({:?})", exact.id.as_str())));
}

fn oracle_functions() -> Vec<ResolvedFunction> {
    let (program, index) = fixture_with_uses(3, true);
    let base = program.functions[index].clone();
    let mut padded = base.clone();
    add_live_padding(&mut padded, 3, "oracle.live", 2, 2, 2);
    add_padding(&mut padded, "oracle.disconnected", 1, 1, 1);
    let mut functions = vec![base, padded];
    let source = include_str!("../../tests/language/while_loops/record_borrow_renewal.spx");
    let ast = crate::check(source, Path::new("oracle-renewal.spx")).unwrap();
    let renewal = crate::hir::resolve(&ast).unwrap();
    functions.extend(renewal.functions.iter().cloned());
    functions
}

fn forged_cfg(points: usize, edges: Vec<(u16, u16)>) -> Cfg<'static> {
    let adjacency = adjacency::Adjacency::build(points, &edges).unwrap();
    Cfg {
        points: Vec::new(),
        nodes: BTreeMap::new(),
        expressions: Vec::new(),
        edges,
        adjacency,
    }
}

fn assert_adjacency_matches_original(cfg: &Cfg<'_>, points: usize) {
    let legacy = legacy_adjacency::LegacyAdjacency::from_edges(points, &cfg.edges);
    for node in 0..points {
        assert_eq!(
            cfg.successors(node).collect::<Vec<_>>(),
            legacy.successors[node]
        );
        assert_eq!(
            cfg.successors(node).rev().collect::<Vec<_>>(),
            legacy.successors[node]
                .iter()
                .rev()
                .copied()
                .collect::<Vec<_>>()
        );
        assert_eq!(
            cfg.successor_edges(node).collect::<Vec<_>>(),
            legacy.successor_edges[node]
        );
        assert_eq!(cfg.predecessors(node), legacy.predecessors[node].as_slice());
    }
}

#[test]
fn flat_adjacency_and_borrowed_children_match_the_saved_original_at_every_point() {
    let mut checked = 0;
    for function in oracle_functions() {
        // Every oracle source is real checked or padded HIR and is an intended
        // positive: a CFG refusal must fail the test, not skip the function.
        let cfg = build_cfg(&function, &mut WorkCounter::new(usize::MAX)).unwrap();
        assert_adjacency_matches_original(&cfg, cfg.points.len());
        for expression in &cfg.expressions {
            let borrowed = children::evaluation_children(expression).collect::<Vec<_>>();
            let original = legacy_adjacency::evaluation_children(expression);
            assert_eq!(borrowed.len(), original.len());
            assert!(borrowed
                .iter()
                .zip(&original)
                .all(|(left, right)| std::ptr::eq(*left, *right)));
        }
        checked += 1;
    }
    assert!(checked >= 3, "oracle must cover real and padded CFGs");
}

#[test]
fn flat_adjacency_preserves_order_and_multiplicity_on_forged_edge_lists() {
    let mut state = 0x2545_f491_u32;
    let mut next = move |bound: usize| {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state as usize % bound
    };
    for points in [1_usize, 2, 3, 8, 65] {
        for density in [0_usize, 1, 4, 40, 200] {
            // Duplicates and self-loops are retained: the oracle pushes one
            // entry per edge, so the flat tables must do the same.
            let mut edges = (0..density)
                .map(|_| (next(points) as u16, next(points) as u16))
                .collect::<Vec<_>>();
            edges.sort();
            let cfg = forged_cfg(points, edges);
            assert_adjacency_matches_original(&cfg, points);
        }
    }
    let hub = 64_u16;
    let mut edges = (1..=hub).map(|node| (0, node)).collect::<Vec<_>>();
    edges.extend((0..hub).map(|node| (node, hub)));
    edges.sort();
    assert_adjacency_matches_original(&forged_cfg(65, edges), 65);
    assert_adjacency_matches_original(&forged_cfg(65, Vec::new()), 65);
}

#[test]
fn flat_adjacency_refuses_an_edge_count_beyond_its_offsets_with_the_edge_cap_diagnostic() {
    let cfg = forged_cfg(1, vec![(0, 0); u16::MAX as usize]);
    assert_eq!(cfg.successor_edges(0), 0..u16::MAX);
    assert_eq!(cfg.predecessors(0).len(), u16::MAX as usize);
    let error = adjacency::Adjacency::build(1, &vec![(0, 0); u16::MAX as usize + 1])
        .err()
        .unwrap();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(error.message, "function exceeds 4,096 loan CFG edges");
}

type OraclePipeline = (
    Result<(Vec<BTreeSet<u16>>, work::EdgeLiveness), String>,
    usize,
);

fn flat_pipeline(
    cfg: &Cfg<'_>,
    starts: &[u16],
    seeds: &BTreeSet<u16>,
    limit: usize,
) -> OraclePipeline {
    let mut work = WorkCounter::new(limit);
    let result = (|| -> Result<_, Diagnostic> {
        let reachable = work::Reachability::build(cfg, starts.iter().copied(), &mut work)?;
        let mut live = Vec::new();
        for (loan, start) in starts.iter().copied().enumerate() {
            let id = LoanId(loan as u16);
            live.push(work::live_nodes(
                cfg, id, start, seeds, &reachable, &mut work,
            )?);
        }
        let edges = work::edge_liveness(cfg, &live, &mut work)?;
        Ok((live, edges))
    })()
    .map_err(|error: Diagnostic| format!("{}:{}", error.code, error.message));
    (result, work.used)
}

fn legacy_pipeline(
    cfg: &Cfg<'_>,
    starts: &[u16],
    seeds: &BTreeSet<u16>,
    limit: usize,
) -> OraclePipeline {
    let legacy = legacy_adjacency::LegacyAdjacency::from_edges(cfg.points.len(), &cfg.edges);
    let mut work = WorkCounter::new(limit);
    let result = (|| -> Result<_, Diagnostic> {
        let reachable =
            legacy_adjacency::reachability(&legacy, cfg.points.len(), starts, &mut work)?;
        let mut live = Vec::new();
        for (loan, start) in starts.iter().copied().enumerate() {
            live.push(legacy_adjacency::live_nodes(
                &legacy, &reachable, loan, start, seeds, &mut work,
            )?);
        }
        let edges = legacy_adjacency::edge_liveness(&legacy, &cfg.edges, &live, &mut work)?;
        Ok((live, edges))
    })()
    .map_err(|error: Diagnostic| format!("{}:{}", error.code, error.message));
    (result, work.used)
}

#[test]
fn flat_traversal_results_work_and_first_work_error_match_the_saved_original() {
    let mut checked = 0;
    for function in oracle_functions() {
        let cfg = build_cfg(&function, &mut WorkCounter::new(usize::MAX)).unwrap();
        let points = cfg.points.len();
        let starts = (0..points)
            .step_by(3)
            .take(130)
            .map(|point| point as u16)
            .collect::<Vec<_>>();
        let seeds = BTreeSet::from([(points - 1) as u16, (points / 2) as u16]);
        let (flat, total) = flat_pipeline(&cfg, &starts, &seeds, usize::MAX);
        let (original, original_total) = legacy_pipeline(&cfg, &starts, &seeds, usize::MAX);
        assert_eq!(flat.as_ref().unwrap(), original.as_ref().unwrap());
        assert_eq!(total, original_total);
        // Exact limit succeeds; one short, and a mid-run limit, fail at the
        // same charged unit with the same diagnostic in both pipelines.
        for limit in [total, total - 1, total / 2, 0] {
            assert_eq!(
                flat_pipeline(&cfg, &starts, &seeds, limit),
                legacy_pipeline(&cfg, &starts, &seeds, limit),
                "limit {limit}"
            );
        }
        let reachable = work::Reachability::build(
            &cfg,
            starts.iter().copied(),
            &mut WorkCounter::new(usize::MAX),
        )
        .unwrap();
        let legacy = legacy_adjacency::LegacyAdjacency::from_edges(points, &cfg.edges);
        let original = legacy_adjacency::reachability(
            &legacy,
            points,
            &starts,
            &mut WorkCounter::new(usize::MAX),
        )
        .unwrap();
        for loan in 0..starts.len() {
            for node in 0..points {
                assert_eq!(
                    reachable.contains(LoanId(loan as u16), node as u16),
                    original.contains(loan, node)
                );
            }
        }
        checked += 1;
    }
    assert!(checked >= 3, "oracle must cover real and padded CFGs");
}
