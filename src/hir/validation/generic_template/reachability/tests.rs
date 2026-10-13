//! Borrowed reachable-instance closure against the complete original owned reference.
//! Forged carriers are cloned from a checked minimal fixture and exist only in these tests.
use super::*;
use crate::test_allocation_observer::measure;

mod old;

type Owned = Vec<(FunctionInstanceId, DeclarationId, Vec<ResolvedType>)>;
type Borrowed<'p> = Vec<(
    &'p FunctionInstanceId,
    &'p DeclarationId,
    &'p [ResolvedType],
)>;
type Triple = (usize, usize, usize, usize);

const SOURCE: &str = r#"module validator.reachability;
@id("reach.z") fn zed(value:i64)->i64 {value+1}
@id("reach.a") fn alpha()->i64 {2}
@id("reach.main") fn main()->i64 {zed(alpha())}
"#;
const IDENTITY: &str = "identity is inconsistent";
const ORDERED: [&str; 18] = [
    "reach.outer",
    "reach.inner",
    "reach.leaf",
    "reach.sibling",
    "reach.p2",
    "reach.s1",
    "reach.s2",
    "reach.i1",
    "reach.m0",
    "reach.g1",
    "reach.v1",
    "reach.v2",
    "reach.k1",
    "reach.k2",
    "reach.r1",
    "reach.e1",
    "reach.f1",
    "reach.late",
];

fn clean() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "validator-reachability.spx").unwrap()).unwrap()
}

/// Same checked program with every function reduced to a call-free leaf.
fn fixture() -> ResolvedProgram {
    let mut program = clean();
    let mut blank = program.functions[0].body.clone();
    blank.kind = ResolvedExprKind::Int(0);
    for function in &mut program.functions {
        function.requires.clear();
        function.ensures.clear();
        function.body = blank.clone();
    }
    program
}

struct Maker {
    like: ResolvedExpr,
    function: ResolvedFunction,
}

impl Maker {
    fn new(program: &ResolvedProgram) -> Self {
        Self {
            like: program.functions[0].body.clone(),
            function: program.functions[0].clone(),
        }
    }
    fn node(&self, kind: ResolvedExprKind) -> ResolvedExpr {
        let mut expression = self.like.clone();
        expression.kind = kind;
        expression
    }
    fn raw(
        &self,
        callee: &str,
        instance: Option<FunctionInstanceId>,
        arguments: &[ResolvedType],
        children: Vec<ResolvedExpr>,
    ) -> ResolvedExpr {
        self.node(ResolvedExprKind::Call {
            callee: DeclarationId::new(callee),
            type_arguments: arguments.to_vec(),
            instance,
            args: children,
        })
    }
    fn with(
        &self,
        callee: &str,
        arguments: &[ResolvedType],
        children: Vec<ResolvedExpr>,
    ) -> ResolvedExpr {
        self.raw(callee, Some(id(callee, arguments)), arguments, children)
    }
    fn call(&self, callee: &str, arguments: &[ResolvedType]) -> ResolvedExpr {
        self.with(callee, arguments, Vec::new())
    }
    /// A non-generic collector: it publishes no instance but visits its children.
    fn plain(&self, children: Vec<ResolvedExpr>) -> ResolvedExpr {
        self.raw("reach.plain", None, &[], children)
    }
    fn instance(
        &self,
        callee: &str,
        arguments: &[ResolvedType],
        body: ResolvedExpr,
    ) -> ResolvedFunctionInstance {
        let mut function = self.function.clone();
        function.body = body;
        ResolvedFunctionInstance {
            id: id(callee, arguments),
            template: DeclarationId::new(callee),
            type_arguments: arguments.to_vec(),
            function,
        }
    }
}

fn id(callee: &str, arguments: &[ResolvedType]) -> FunctionInstanceId {
    FunctionInstanceId::derive(&DeclarationId::new(callee), arguments)
}

fn nested_with(depth: usize, leaf: ResolvedType) -> ResolvedType {
    let mut ty = leaf;
    for level in 0..depth {
        ty = match level % 3 {
            0 => ResolvedType::Nominal {
                declaration: DeclarationId::new("reach.wrap"),
                arguments: vec![ty, ResolvedType::Bool],
            },
            1 => ResolvedType::Function {
                parameters: vec![ty, ResolvedType::Char],
                result: Box::new(ResolvedType::Usize),
            },
            _ => ResolvedType::Nominal {
                declaration: DeclarationId::new("reach.list"),
                arguments: vec![ty],
            },
        };
    }
    ty
}

fn nested(depth: usize) -> ResolvedType {
    nested_with(depth, ResolvedType::I64)
}

fn owned(result: Result<Borrowed<'_>, Diagnostic>) -> Result<Owned, Diagnostic> {
    result.map(|reachable| {
        reachable
            .into_iter()
            .map(|(instance, template, arguments)| {
                (instance.clone(), template.clone(), arguments.to_vec())
            })
            .collect()
    })
}

/// Full success and full Diagnostic parity with the complete original reference.
fn assert_parity(program: &ResolvedProgram) -> Result<Owned, Diagnostic> {
    let expected = old::reachable_instances(program);
    let actual = owned(reachable_instances(program));
    assert_eq!(actual, expected);
    actual
}

fn names(closure: &Owned) -> Vec<&str> {
    closure
        .iter()
        .map(|(_, template, _)| template.as_str())
        .collect()
}

fn addr<T>(reference: &T) -> usize {
    reference as *const T as usize
}

fn published(reachable: &Borrowed<'_>) -> Vec<Triple> {
    reachable
        .iter()
        .map(|(instance, template, arguments)| {
            (
                addr(*instance),
                addr(*template),
                arguments.as_ptr() as usize,
                arguments.len(),
            )
        })
        .collect()
}

/// Every instance-carrying call node in program order, by exact carrier address.
fn carriers(program: &ResolvedProgram) -> Vec<Triple> {
    let mut found = Vec::new();
    let functions = program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    );
    for function in functions {
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            visit_resolved_calls(expression, &mut |callee, instance, arguments| {
                if let Some(instance) = instance {
                    found.push((
                        addr(instance),
                        addr(callee),
                        arguments.as_ptr() as usize,
                        arguments.len(),
                    ));
                }
            });
        }
    }
    found
}

fn assert_published(program: &ResolvedProgram) {
    let originals = carriers(program);
    for triple in published(&reachable_instances(program).unwrap()) {
        assert!(
            originals.contains(&triple),
            "scratch reference is not an original carrier"
        );
    }
}

fn unique(count: usize, repeat: usize) -> ResolvedProgram {
    let mut program = fixture();
    let maker = Maker::new(&program);
    let mut calls = Vec::new();
    for _ in 0..repeat {
        for index in 0..count {
            calls.push(maker.call(&format!("reach.u{index}"), &[ResolvedType::I64]));
        }
    }
    program.functions[0].body = maker.plain(calls);
    program
}

fn push_root(program: &mut ResolvedProgram, expression: ResolvedExpr) {
    let ResolvedExprKind::Call { args, .. } = &mut program.functions[0].body.kind else {
        panic!("root collector expected");
    };
    args.push(expression);
}

fn ordered_program() -> ResolvedProgram {
    let mut program = fixture();
    let m = Maker::new(&program);
    let a = [ResolvedType::I64];
    let g = |name: &str| m.call(name, &a);
    let copy = |name: &str| ResolvedStatement::CopyDiscard {
        value: g(name),
        span: m.like.span,
    };
    let arm = |guard: Option<ResolvedExpr>, value: ResolvedExpr| ResolvedMatchArm {
        pattern: ResolvedMatchPattern::Wildcard,
        guard: guard.map(Box::new),
        value,
        span: m.like.span,
    };
    let outer = m.with(
        "reach.outer",
        &a,
        vec![
            m.with("reach.inner", &a, vec![g("reach.leaf")]),
            g("reach.sibling"),
        ],
    );
    program.functions[0].requires = vec![
        outer,
        m.node(ResolvedExprKind::FunctionReference {
            target: DeclarationId::new("reach.fn"),
        }),
        g("reach.p2"),
    ];
    program.functions[0].body = m.node(ResolvedExprKind::Block {
        statements: vec![copy("reach.s1"), copy("reach.s2")],
        tail: Box::new(m.node(ResolvedExprKind::If {
            condition: Box::new(g("reach.i1")),
            then_branch: Box::new(m.node(ResolvedExprKind::Match {
                mode: ResolvedMatchMode::Borrow,
                scrutinee: Box::new(g("reach.m0")),
                arms: vec![
                    arm(Some(g("reach.g1")), g("reach.v1")),
                    arm(None, g("reach.v2")),
                ],
            })),
            else_branch: Box::new(m.node(ResolvedExprKind::Invoke {
                callable: Box::new(g("reach.k1")),
                args: vec![
                    g("reach.k2"),
                    m.node(ResolvedExprKind::Project {
                        base: Box::new(g("reach.r1")),
                        field: DeclarationId::new("reach.field"),
                    }),
                ],
            })),
        })),
    });
    program.functions[0].ensures = vec![g("reach.e1")];
    program.functions[1].body = m.plain(vec![g("reach.f1")]);
    program
        .function_instances
        .push(m.instance("reach.i1", &a, m.plain(vec![g("reach.late")])));
    program
}

fn cyclic_program() -> ResolvedProgram {
    let mut program = fixture();
    let m = Maker::new(&program);
    let a = [ResolvedType::I64];
    let b = [ResolvedType::Bool];
    let c = [nested(5)];
    program.functions[1].body = m.plain(vec![m.call("reach.a", &a), m.call("reach.d", &b)]);
    program.function_instances.push(m.instance(
        "reach.a",
        &a,
        m.plain(vec![m.call("reach.b", &b), m.call("reach.a", &a)]),
    ));
    program.function_instances.push(m.instance(
        "reach.b",
        &b,
        m.plain(vec![
            m.call("reach.a", &a),
            m.call("reach.c", &c),
            m.call("reach.b", &b),
        ]),
    ));
    program
}

fn forged_program() -> ResolvedProgram {
    let mut program = fixture();
    let m = Maker::new(&program);
    let a = [ResolvedType::I64];
    program.functions[0].body = m.plain(vec![
        m.call("reach.a", &a),
        m.raw(
            "reach.a",
            Some(id("reach.a", &a)),
            &[ResolvedType::Bool],
            Vec::new(),
        ),
    ]);
    program
}

#[test]
fn empty_and_every_argument_carrier_shape_match_original_with_exact_originals() {
    let empty = fixture();
    assert_eq!(assert_parity(&empty).unwrap(), Owned::new());
    assert!(published(&reachable_instances(&empty).unwrap()).is_empty());

    let mut program = fixture();
    let maker = Maker::new(&program);
    let argument_sets: Vec<Vec<ResolvedType>> = vec![
        Vec::new(),
        vec![ResolvedType::I64],
        vec![ResolvedType::Nominal {
            declaration: DeclarationId::new("reach.record"),
            arguments: vec![ResolvedType::Bool, ResolvedType::Usize],
        }],
        vec![
            ResolvedType::Function {
                parameters: vec![ResolvedType::I64, ResolvedType::Char],
                result: Box::new(ResolvedType::Bool),
            },
            ResolvedType::Str,
        ],
        vec![nested(48), nested(3), ResolvedType::Bytes],
    ];
    let calls: Vec<ResolvedExpr> = argument_sets
        .iter()
        .enumerate()
        .map(|(index, arguments)| maker.call(&format!("reach.c{index}"), arguments))
        .collect();
    program.functions[0].body = maker.plain(calls);
    let actual = assert_parity(&program).unwrap();
    assert_eq!(actual.len(), argument_sets.len());
    for ((_, _, arguments), expected) in actual.iter().zip(&argument_sets) {
        assert_eq!(arguments, expected);
    }
    let reachable = reachable_instances(&program).unwrap();
    assert_eq!(published(&reachable), carriers(&program));
}

#[test]
fn duplicates_keep_bfs_first_seen_order_and_first_original_carriers() {
    let mut program = fixture();
    let m = Maker::new(&program);
    let a = [ResolvedType::I64];
    let b = [ResolvedType::Bool];
    program.functions[0].requires = vec![m.call("reach.b", &b)];
    program.functions[0].body = m.plain(vec![
        m.call("reach.a", &a),
        m.call("reach.b", &b),
        m.call("reach.a", &a),
    ]);
    program.functions[1].ensures = vec![m.call("reach.a", &a), m.call("reach.c", &[])];
    program.functions[2].body = m.plain(vec![m.call("reach.c", &[]), m.call("reach.d", &a)]);
    let closure = assert_parity(&program).unwrap();
    assert_eq!(
        names(&closure),
        ["reach.b", "reach.a", "reach.c", "reach.d"]
    );
    let originals = carriers(&program);
    assert_eq!(originals.len(), 8);
    let reachable = reachable_instances(&program).unwrap();
    assert_eq!(
        published(&reachable),
        [originals[0], originals[1], originals[5], originals[7]]
    );
}

#[test]
fn duplicate_identity_is_authenticated_before_seen_suppression() {
    let a = [ResolvedType::I64];
    let m = Maker::new(&fixture());
    let deep = nested_with(30, ResolvedType::I64);
    let pairs = [
        (
            m.call("reach.a", &a),
            m.raw(
                "reach.a",
                Some(id("reach.a", &a)),
                &[ResolvedType::Bool],
                Vec::new(),
            ),
        ),
        (
            m.call("reach.a", &a),
            m.raw("reach.other", Some(id("reach.a", &a)), &a, Vec::new()),
        ),
        (
            m.call("reach.a", &a),
            m.raw("reach.a", Some(id("reach.a", &a)), &[], Vec::new()),
        ),
        (
            m.call("reach.a", &a),
            m.raw(
                "reach.a",
                Some(id("reach.a", &a)),
                &[ResolvedType::I64, ResolvedType::I64],
                Vec::new(),
            ),
        ),
        (
            m.call("reach.deep", std::slice::from_ref(&deep)),
            m.raw(
                "reach.deep",
                Some(id("reach.deep", std::slice::from_ref(&deep))),
                &[nested_with(30, ResolvedType::Bool)],
                Vec::new(),
            ),
        ),
    ];
    for (valid, forged) in &pairs {
        let mut control = fixture();
        control.functions[0].body = m.plain(vec![valid.clone(), valid.clone()]);
        assert_eq!(assert_parity(&control).unwrap().len(), 1);
        for forged_first in [false, true] {
            let mut program = fixture();
            let children = if forged_first {
                vec![forged.clone(), valid.clone()]
            } else {
                vec![valid.clone(), forged.clone()]
            };
            program.functions[0].body = m.plain(children);
            let error = assert_parity(&program).unwrap_err();
            assert!(error.message.contains(IDENTITY), "{}", error.message);
        }
    }
}

#[test]
fn distinct_instance_bound_is_256_and_counts_materialized_discoveries() {
    let a = [ResolvedType::I64];
    let exact = assert_parity(&unique(256, 1)).unwrap();
    assert_eq!(exact.len(), 256);
    for (index, (_, template, _)) in exact.iter().enumerate() {
        assert_eq!(template.as_str(), format!("reach.u{index}").as_str());
    }
    assert_published(&unique(256, 1));
    assert_eq!(assert_parity(&unique(256, 2)).unwrap(), exact);
    let over = assert_parity(&unique(257, 1)).unwrap_err();
    assert!(
        over.message.contains("exceeds 256 entries"),
        "{}",
        over.message
    );

    // Authentication precedes both seen suppression and the bound.
    let mut forged = unique(256, 1);
    let maker = Maker::new(&forged);
    push_root(
        &mut forged,
        maker.raw("reach.u256", Some(id("reach.u0", &a)), &a, Vec::new()),
    );
    let error = assert_parity(&forged).unwrap_err();
    assert!(error.message.contains(IDENTITY));
    assert!(!error.message.contains("exceeds"));

    for (late, fits) in [("reach.x", false), ("reach.u0", true)] {
        let mut grown = unique(255, 1);
        let maker = Maker::new(&grown);
        push_root(&mut grown, maker.call("reach.m", &a));
        grown.function_instances.push(maker.instance(
            "reach.m",
            &a,
            maker.plain(vec![maker.call(late, &a)]),
        ));
        let result = assert_parity(&grown);
        if fits {
            assert_eq!(result.unwrap().len(), 256);
        } else {
            assert!(result.unwrap_err().message.contains("exceeds 256 entries"));
        }
    }
}

#[test]
fn cycles_and_missing_materializations_terminate_in_original_order() {
    let program = cyclic_program();
    let closure = assert_parity(&program).unwrap();
    assert_eq!(
        names(&closure),
        ["reach.a", "reach.d", "reach.b", "reach.c"]
    );
    assert_published(&program);
}

#[test]
fn first_matching_materialization_wins_for_duplicate_ids() {
    let a = [ResolvedType::I64];
    for first_wins in [true, false] {
        let mut program = fixture();
        let m = Maker::new(&program);
        program.functions[0].body = m.plain(vec![m.call("reach.a", &a)]);
        let first = m.instance("reach.a", &a, m.plain(vec![m.call("reach.first", &a)]));
        let second = m.instance("reach.a", &a, m.plain(vec![m.call("reach.second", &a)]));
        program.function_instances = if first_wins {
            vec![first, second]
        } else {
            vec![second, first]
        };
        let closure = assert_parity(&program).unwrap();
        let expected = if first_wins {
            "reach.first"
        } else {
            "reach.second"
        };
        assert_eq!(names(&closure), ["reach.a", expected]);
        let originals = carriers(&program);
        let reachable = reachable_instances(&program).unwrap();
        let triples = published(&reachable);
        assert_eq!(triples, [originals[0], originals[1]]);
        assert!(!triples.contains(&originals[2]));
    }
}

#[test]
fn requires_body_ensures_and_nested_call_preorder_is_exact() {
    let program = ordered_program();
    let closure = assert_parity(&program).unwrap();
    assert_eq!(names(&closure), ORDERED);
    let reachable = reachable_instances(&program).unwrap();
    assert_eq!(published(&reachable), carriers(&program));
}

#[test]
fn full_validator_receiver_matches_original_and_outlives_its_borrow() {
    crate::hir::validate(&clean()).unwrap();
    let programs = [ordered_program(), cyclic_program(), forged_program()];
    for program in &programs {
        let expected = old::reachable_instances(program);
        let validator = HirValidator::new(program).unwrap();
        let actual = owned(validator.reachable_function_instances());
        assert_eq!(actual, expected);
    }
    let validator = HirValidator::new(&programs[2]).unwrap();
    let error = validator.reachable_function_instances().err().unwrap();
    assert!(error.message.contains(IDENTITY));

    let program = ordered_program();
    let reachable = {
        let validator = HirValidator::new(&program).unwrap();
        validator.reachable_function_instances().unwrap()
    };
    assert_eq!(published(&reachable), carriers(&program));
}

fn measured_program() -> ResolvedProgram {
    let mut program = fixture();
    let m = Maker::new(&program);
    let mut calls = Vec::new();
    for _round in 0..2 {
        for index in 0..24 {
            let arguments = [nested(4 + index % 5), ResolvedType::Bool];
            calls.push(m.call(&format!("reach.n{index}"), &arguments));
        }
    }
    program.functions[0].body = m.plain(calls);
    for index in 0..6 {
        let arguments = [nested(4 + index % 5), ResolvedType::Bool];
        let child = [nested(3), ResolvedType::Usize];
        program.function_instances.push(m.instance(
            &format!("reach.n{index}"),
            &arguments,
            m.plain(vec![m.call(&format!("reach.k{index}"), &child)]),
        ));
    }
    program
}

fn summarize<T>(result: Result<Vec<T>, Diagnostic>) -> Result<usize, ()> {
    let summary = result.as_ref().map(Vec::len).map_err(|_| ());
    drop(result);
    summary
}

#[test]
fn borrowed_closure_allocates_less_than_original_through_full_drop() {
    let mut forged = measured_program();
    let m = Maker::new(&forged);
    push_root(
        &mut forged,
        m.raw(
            "reach.n0",
            Some(id("reach.n0", &[ResolvedType::Bool])),
            &[nested(6)],
            Vec::new(),
        ),
    );
    let cases = [
        (measured_program(), Result::<usize, ()>::Ok(30)),
        (forged, Err(())),
    ];
    for (program, expected) in &cases {
        let (old_result, old_metrics) = measure(|| summarize(old::reachable_instances(program)));
        let (new_result, new_metrics) = measure(|| summarize(reachable_instances(program)));
        assert_eq!(old_result, *expected);
        assert_eq!(new_result, *expected);
        for metrics in [old_metrics, new_metrics] {
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
        assert!(old_metrics.allocations > new_metrics.allocations);
        assert!(old_metrics.allocated > new_metrics.allocated);
    }
}
