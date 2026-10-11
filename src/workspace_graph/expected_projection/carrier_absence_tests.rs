//! Carrier-free root census: independent AST and resolved-HIR position oracles,
//! a complete-forecast differential against the position-blind formula, exact
//! and one-short limits, malformed-census refusal, and a resolver/replay
//! allocation highwater gate. Nothing here mirrors `declaration_cost`.
use super::super::{cached_exact, dependency_identity_max, synthetic_builder_bytes_scoped};
use super::StructuralCost;
use crate::ast::{Expr, ExprKind, Program};
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedStatement};

/// Independent actual-layout oracle for the six omitted carriers.
fn carriers() -> usize {
    use std::mem::size_of;
    size_of::<crate::hir::ResolvedBinding>()
        + size_of::<crate::hir::ResolvedStatement>()
        + size_of::<crate::hir::ResolvedFieldInitializer>()
        + size_of::<crate::hir::ResolvedMatchArm>()
        + size_of::<crate::hir::ResolvedMatchPatternField>()
        + size_of::<crate::hir::ResolvedRecordMatchPatternField>()
}

#[derive(Debug, Default, Eq, PartialEq)]
struct Census {
    kinds: [usize; 3],
    free: usize,
}

fn ast_kind(kind: &ExprKind) -> Option<usize> {
    match kind {
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_) => Some(0),
        ExprKind::Unary { .. } => Some(1),
        ExprKind::Binary { .. } => Some(2),
        _ => None,
    }
}

/// Raw AST oracle: a root is free only as a unary/binary operand, an `if`
/// condition/branch, a block tail, or a free-function call argument.
fn ast_census(expression: &Expr, free: bool, census: &mut Census) {
    if let Some(kind) = ast_kind(&expression.kind) {
        census.kinds[kind] += 1;
        census.free += usize::from(free);
    }
    let free_children: Vec<&Expr> = match &expression.kind {
        ExprKind::Unary { value, .. } => vec![value],
        ExprKind::Binary { left, right, .. } => vec![left, right],
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => vec![condition, then_branch, else_branch],
        ExprKind::Call { args, .. } => args.iter().collect(),
        ExprKind::Block { tail, .. } => vec![tail],
        _ => Vec::new(),
    };
    let mut index = 0;
    while let Some(child) = expression.child(index) {
        let is_free = free_children.iter().any(|candidate| std::ptr::eq(*candidate, child));
        ast_census(child, is_free, census);
        index += 1;
    }
}

fn hir_kind(kind: &ResolvedExprKind) -> Option<usize> {
    match kind {
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_) => Some(0),
        ResolvedExprKind::Unary { .. } => Some(1),
        ResolvedExprKind::Binary { .. } => Some(2),
        _ => None,
    }
}

/// Resolved-HIR oracle over the actual containers: a root is free only when
/// it is held by a `Box` of a unary/binary/`if`/block-tail node or by a call's
/// argument vector. Statement children are carried by `ResolvedStatement`.
/// Returns the number of auxiliary carriers actually constructed.
fn hir_census(expression: &ResolvedExpr, free: bool, census: &mut Census) -> usize {
    if let Some(kind) = hir_kind(&expression.kind) {
        census.kinds[kind] += 1;
        census.free += usize::from(free);
    }
    match &expression.kind {
        ResolvedExprKind::Unary { value, .. } => hir_census(value, true, census),
        ResolvedExprKind::Binary { left, right, .. } => {
            hir_census(left, true, census) + hir_census(right, true, census)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            hir_census(condition, true, census)
                + hir_census(then_branch, true, census)
                + hir_census(else_branch, true, census)
        }
        ResolvedExprKind::Call { args, .. } => args
            .iter()
            .map(|argument| hir_census(argument, true, census))
            .sum(),
        ResolvedExprKind::Block { statements, tail } => {
            let mut built = 0;
            for statement in statements {
                built += 1 + usize::from(matches!(
                    statement,
                    ResolvedStatement::Let { .. } | ResolvedStatement::Assign { .. }
                ));
                for index in 0..statement.child_count() {
                    built += hir_census(statement.child(index).unwrap(), false, census);
                }
            }
            built + hir_census(tail, true, census)
        }
        ResolvedExprKind::Place(_) => 0,
        _ if hir_kind(&expression.kind) == Some(0) => 0,
        other => panic!("fixture admits no other resolved kind: {other:?}"),
    }
}

fn cost_census(program: &Program) -> StructuralCost {
    let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
    for function in &program.functions {
        super::super::declaration_cost::ast_function_cost(function, &mut cost).unwrap();
    }
    cost
}

// Literal, checked unary/binary, lazy `&&`/`||`, comparisons and call arguments
// in carrier-free positions beside `let`/assignment values that stay delegated.
const GATE: &str = r#"module carrier.gate;
@id("carrier.gate.add") fn add(a: i64, b: i64) -> i64 { a + b }
@id("carrier.gate.main") fn main() -> i64 {
 let mut total = 1 + 2;
 total = total * 3;
 let flag = !(total < 4) && (total >= 5 || total != 6);
 if flag && add(7, 8 - 9) == 10 { -(11 / 12) } else { add(total, 13) % 14 }
}
"#;

#[test]
fn raw_ast_resolved_and_cost_censuses_agree_on_every_carrier_free_root() {
    let parsed = crate::parse(GATE, std::path::Path::new("gate.spx")).unwrap();
    let mut ast = Census::default();
    for function in &parsed.functions {
        ast_census(&function.body, false, &mut ast);
    }
    // Hand count: 14 literals, 2 unary, 13 binary; `1 + 2`, `total * 3` and
    // the `let flag` conjunction are the only delegated censused roots.
    assert_eq!(
        ast,
        Census {
            kinds: [14, 2, 13],
            free: 26
        }
    );
    let cost = cost_census(&parsed);
    assert_eq!(cost.exact_absent_roots, ast.kinds);
    assert_eq!(cost.exact_carrier_free_roots, ast.free);
    // Legacy and runtime accumulators never count.
    let mut legacy = StructuralCost::new();
    for function in &parsed.functions {
        super::super::declaration_cost::ast_function_cost(function, &mut legacy).unwrap();
    }
    assert_eq!(legacy.exact_carrier_free_roots, 0);

    let checked = crate::check(GATE, "gate.spx").unwrap();
    let resolved = crate::hir::resolve(&checked).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let mut hir = Census::default();
    let mut built = 0;
    for name in ["add", "main"] {
        let function = resolved
            .functions
            .iter()
            .find(|function| function.name == name)
            .unwrap();
        assert!(function.requires.is_empty() && function.ensures.is_empty());
        built += hir_census(&function.body, false, &mut hir);
    }
    // No literal/unary/binary root is folded, elided, or moved into a carrier.
    assert_eq!(hir, ast);
    // Exactly the three authored statements (two with bindings) exist, all
    // paid by delegated bundles that this change leaves complete.
    assert_eq!(built, 3 + 3);
    assert!(built <= cost.exact_expressions - cost.exact_carrier_free_roots);
}

#[test]
fn delegated_parents_keep_complete_bundles_and_guards_are_charged() {
    let census = |body: &str| {
        let source = format!("module m; @id(\"m.main\") fn main(left: i64) -> i64 {{ {body} }}");
        let program = crate::parse(&source, std::path::Path::new("m.spx")).unwrap();
        let mut oracle = Census::default();
        ast_census(&program.functions[0].body, false, &mut oracle);
        let cost = cost_census(&program);
        assert_eq!(cost.exact_absent_roots, oracle.kinds, "{body}");
        assert_eq!(cost.exact_carrier_free_roots, oracle.free, "{body}");
        (oracle, cost)
    };
    // Arm values and guards embed or box under a `ResolvedMatchArm`; the
    // `let` value is embedded in its statement. Only the tail-free match
    // root's literal operands inside are free (here: `left + 1` has `1`).
    let (guarded, with_guard) =
        census("let x = 3; match left { n if n >= 0 => n + 1, _ => 2, }");
    assert_eq!(guarded.kinds, [4, 0, 2]);
    assert_eq!(guarded.free, 2, "`0` and `1` are operands; `3`, `2`, arms are not");
    let (_, without_guard) = census("let x = 3; match left { n => n + 1, _ => 2, }");
    // The guard subtree (`n >= 0`, `n`, `0`) is now a complete charged tree.
    assert!(
        with_guard.total - without_guard.total >= 3 * std::mem::size_of::<Expr>(),
        "guard bytes must be charged"
    );
    assert_eq!(with_guard.exact_expressions - without_guard.exact_expressions, 3);
    let slots = |body: &str| {
        let source = format!("module m; @id(\"m.main\") fn main(left: i64) -> i64 {{ {body} }}");
        let program = crate::parse(&source, std::path::Path::new("m.spx")).unwrap();
        super::super::identity_slots::ast_function_identity_slots(&program.functions[0]).unwrap()
    };
    assert!(
        slots("match left { n if n >= 0 => n + 1, _ => 2, }")
            > slots("match left { n => n + 1, _ => 2, }")
    );
}

/// Same node multiset, strings, identities and calls; only the position of
/// the `n` literals differs (let values versus call arguments).
fn pair(n: usize, free: bool) -> Vec<Program> {
    vec![crate::parse(&pair_source(n, free), std::path::Path::new("pair.spx")).unwrap()]
}

fn pair_source(n: usize, free: bool) -> String {
    let params = (0..n)
        .map(|index| format!("a{index}: i64"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut body = String::new();
    for index in 0..n {
        let value = if free { "f()".to_owned() } else { index.to_string() };
        body.push_str(&format!("let v{index} = {value}; "));
    }
    let args = (0..n)
        .map(|index| if free { index.to_string() } else { "f()".to_owned() })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "module pair; @id(\"pair.f\") fn f() -> i64 {{ 0 }} @id(\"pair.g\") fn g({params}) -> i64 {{ 0 }} @id(\"pair.main\") fn main() -> i64 {{ {body}g({args}) }}"
    )
}

fn retained(programs: &[Program], mode: u8) -> (usize, usize) {
    let authored = crate::workspace_graph::index_authored(programs).unwrap();
    let maximum = Some(dependency_identity_max(&programs[0], &authored, programs).unwrap());
    let costs =
        synthetic_builder_bytes_scoped(&programs[0], &authored, programs, maximum, mode, true)
            .unwrap();
    (costs.retained_hir, costs.retained_clone_and_hir)
}

struct LimitRestore(usize);
impl Drop for LimitRestore {
    fn drop(&mut self) {
        crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(self.0));
    }
}

#[test]
fn complete_forecast_differs_from_position_blind_formula_by_exact_carriers() {
    const N: usize = 12;
    let (free, delegated) = (pair(N, true), pair(N, false));
    let mut census = Census::default();
    ast_census(&free[0].functions[2].body, false, &mut census);
    assert_eq!(census.free, N);
    // The old formula is position-blind: every earlier mode is identical.
    for mode in 0..5 {
        assert_eq!(retained(&free, mode), retained(&delegated, mode), "mode {mode}");
    }
    let (free_hir, free_total) = retained(&free, 5);
    let (delegated_hir, delegated_total) = retained(&delegated, 5);
    assert_eq!(delegated_hir - free_hir, N * carriers());
    assert_eq!(delegated_total - free_total, N * carriers());
    for (retention, copies) in [
        (cached_exact::Retention::Parsed, 1),
        (cached_exact::Retention::Semantic, 2),
    ] {
        let sum = |programs: &[Program]| {
            let authored = crate::workspace_graph::index_authored(programs).unwrap();
            cached_exact::summed(programs, &authored, retention).unwrap()
        };
        let (free_sum, delegated_sum) = (sum(&free), sum(&delegated));
        assert_eq!(delegated_sum.0 - free_sum.0, copies * N * carriers());
        assert_eq!(delegated_sum.1 - free_sum.1, copies * N * carriers());
        // Exact limit admits, one short refuses, and the delegated twin
        // (same bytes under the old formula) refuses at the free bound.
        let previous =
            crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(free_sum.1));
        let _restore = LimitRestore(previous);
        let authored = crate::workspace_graph::index_authored(&free).unwrap();
        assert_eq!(
            cached_exact::summed(&free, &authored, retention).unwrap(),
            free_sum
        );
        let twin = crate::workspace_graph::index_authored(&delegated).unwrap();
        assert!(cached_exact::summed(&delegated, &twin, retention)
            .unwrap_err()
            .iter()
            .any(|error| error.code == "SPX-G171"));
        crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(free_sum.1 - 1));
        assert!(cached_exact::summed(&free, &authored, retention)
            .unwrap_err()
            .iter()
            .any(|error| error.code == "SPX-G171"));
    }
}

/// Final gate (actual allocation, not a mirror): resolving and replaying each
/// fixture through the real resolver and HIR validator, with the resolved
/// program live, never requests more backing than its reduced mode-5 forecast.
#[test]
fn resolver_and_replay_highwater_stays_within_the_reduced_forecast() {
    for (source, path) in [
        (GATE.to_owned(), "gate.spx"),
        (pair_source(12, true), "pair.spx"),
    ] {
        let parsed = vec![crate::parse(&source, std::path::Path::new(path)).unwrap()];
        let census = cost_census(&parsed[0]);
        assert!(census.exact_carrier_free_roots >= 12, "gate exercises the discount");
        let (_, forecast) = retained(&parsed, 5);
        let checked = crate::check(&source, path).unwrap();
        let (resolved, allocation) = crate::test_allocation_observer::measure(|| {
            let resolved = crate::hir::resolve(&checked).unwrap();
            crate::hir::validate(&resolved).unwrap();
            resolved
        });
        assert!(!allocation.invalid, "{path}: observer capacity");
        assert!(allocation.live > 0, "{path}: resolved program stays live");
        assert!(
            allocation.highwater <= forecast,
            "{path}: highwater {} exceeds reduced forecast {forecast}",
            allocation.highwater
        );
        drop(resolved);
    }
}
