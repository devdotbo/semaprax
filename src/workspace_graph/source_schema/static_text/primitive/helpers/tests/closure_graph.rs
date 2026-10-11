//! #763 closure graph. The direct `parity` controls here are algorithm
//! evidence only: they run the saved and current graphs alone, and the
//! cycle, duplicate, wide/dense and 8 KiB cases mutate or replace caller HIR,
//! bypassing consumer source authentication. They are not production-cap
//! acceptance. Complete authentic-source entries, saved-entry replay and
//! unchanged default-cap controls live in `entry`; the exact/one-short and
//! sticky later-provider controls below also run the complete entry.
use super::*;
mod entry;
mod oracle;

fn current(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> Result<oracle::Shape, Vec<Diagnostic>> {
    let anchor = module.functions.first().ok_or_else(fail)?;
    let mut resources =
        SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 28).map_err(|_| fail())?;
    let graph = graph::discover(module, modules, facts, &mut resources, anchor)?;
    let mut order = Vec::new();
    let verdict = graph::order(&graph, &mut resources, anchor, |vertex| order.push(vertex));
    let functions: Vec<*const ResolvedFunction> = module
        .functions
        .iter()
        .chain(graph.helpers.iter().map(|m| m.function))
        .map(|f| f as *const ResolvedFunction)
        .collect();
    // Every ordinal keeps its arena slot; the linked tree stays AVL-bounded.
    let (arena, sorted, height) = graph.directory.audit();
    assert_eq!(arena, functions, "arena slot i is ordinal i");
    assert!(height <= avl_bound(sorted.len()), "height {height}");
    let offsets = &graph.offsets;
    assert_eq!(offsets.len(), functions.len() + 1);
    assert_eq!(offsets[0], 0);
    assert_eq!(offsets[functions.len()], graph.targets.len());
    let mut edges = Vec::new();
    for from in 0..functions.len() {
        assert!(
            offsets[from] <= offsets[from + 1],
            "contiguous range {from}"
        );
        for to in &graph.targets[offsets[from]..offsets[from + 1]] {
            edges.push((from, *to));
        }
    }
    let visits = order
        .iter()
        .map(|&v| offsets[v + 1] - offsets[v])
        .sum::<usize>();
    let acyclic = match verdict {
        Ok(counted) => {
            assert_eq!(counted, visits);
            assert_eq!(counted, edges.len(), "one Kahn visit per outgoing edge");
            true
        }
        Err(_) => false,
    };
    Ok(oracle::Shape {
        functions,
        edges,
        order,
        acyclic,
        walks: graph.walks,
        visits,
    })
}
/// The greatest height of any AVL tree of `n` nodes: the largest `h` whose
/// minimal node count `N(h) = N(h-1) + N(h-2) + 1` does not exceed `n`.
fn avl_bound(n: usize) -> usize {
    let (mut height, mut low, mut high) = (0, 0usize, 1usize);
    while high <= n {
        (low, high) = (high, low + high + 1);
        height += 1;
    }
    height
}
fn codes(errors: &[Diagnostic]) -> Vec<&'static str> {
    errors.iter().map(|d| d.code).collect()
}
fn parity(build: &WorkspaceGraphBuild, name: &str) -> Result<oracle::Shape, Vec<&'static str>> {
    let module = build.hir.modules.iter().find(|m| m.module == name).unwrap();
    let saved = oracle::run(module, &build.hir.modules, &build.hir.declarations);
    let actual = current(module, &build.hir.modules, &build.hir.declarations);
    match (saved, actual) {
        (Ok(saved), Ok(actual)) => {
            assert_eq!(actual.functions, saved.functions, "exact defining pointers");
            assert_eq!(actual.edges, saved.edges, "edge order and multiplicity");
            assert_eq!(actual.order, saved.order, "frontier order");
            assert_eq!(actual.acyclic, saved.acyclic, "DAG/cycle verdict");
            assert_eq!(saved.walks, 2 * saved.functions.len());
            assert_eq!(actual.walks, actual.functions.len(), "one walk per body");
            assert_eq!(saved.visits, saved.order.len() * saved.edges.len());
            assert!(actual.visits <= actual.edges.len());
            Ok(actual)
        }
        (Err(saved), Err(actual)) => {
            assert_eq!(codes(&actual), codes(&saved));
            Err(codes(&actual))
        }
        (saved, actual) => panic!(
            "verdict split: saved {} current {}",
            saved.is_ok(),
            actual.is_ok()
        ),
    }
}
fn ptr(build: &WorkspaceGraphBuild, module: &str, id: &str) -> *const ResolvedFunction {
    build
        .hir
        .modules
        .iter()
        .find(|m| m.module == module)
        .unwrap()
        .functions
        .iter()
        .find(|f| f.id.as_str() == id)
        .unwrap() as *const ResolvedFunction
}
fn module_mut<'a>(
    build: &'a mut WorkspaceGraphBuild,
    name: &str,
) -> &'a mut WorkspaceResolvedModule {
    build
        .hir
        .modules
        .iter_mut()
        .find(|m| m.module == name)
        .unwrap()
}
fn each_call(expression: &mut hir::ResolvedExpr, visit: &mut dyn FnMut(&mut hir::DeclarationId)) {
    match &mut expression.kind {
        hir::ResolvedExprKind::Call { callee, args, .. } => {
            visit(callee);
            for argument in args {
                each_call(argument, visit);
            }
        }
        hir::ResolvedExprKind::Binary { left, right, .. } => {
            each_call(left, visit);
            each_call(right, visit);
        }
        hir::ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                if let hir::ResolvedStatement::Let { value, .. } = statement {
                    each_call(value, visit);
                }
            }
            each_call(tail, visit);
        }
        _ => {}
    }
}

const DIAMOND_CALLER: &str = r#"module scalar.app;
use function @id("a.step") from scalar.a as a_step;
use function @id("b.step") from scalar.b as b_step;
@id("app.main") fn main()->i64 {let label=str"é\u{0}";let outcome=a_step(1)+b_step(2);let mut tick=0usize;while tick<1usize {tick=tick+1usize;return outcome;} 0}
"#;
fn side(name: &str, body: &str) -> String {
    format!(
        "module scalar.{name};\nuse function @id(\"leaf.step\") from scalar.leaf as leaf_step;\n@id(\"{name}.step\") fn step(n:i64)->i64 {{{body}}}\n"
    )
}
fn diamond(b: &str) -> WorkspaceGraphBuild {
    build_sources(&[
        ("caller.spx", DIAMOND_CALLER),
        ("a.spx", &side("a", "leaf_step(n)")),
        ("b.spx", &side("b", b)),
        ("leaf.spx", LEAF),
    ])
    .unwrap()
}
fn repeated() -> WorkspaceGraphBuild {
    build(
        &CALLER.replace("step(41)", "step(20)+step(21)"),
        &PROVIDER.replace("{leaf_step(n)}", "{leaf_step(n)+leaf_step(0)}"),
        LEAF,
    )
    .unwrap()
}
fn aliases() -> WorkspaceGraphBuild {
    let caller = CALLER
        .replace(
            "as step;",
            "as first_step;\nuse function @id(\"leaf.step\") from scalar.leaf as second_step;",
        )
        .replace("step(41)", "first_step(30)+second_step(0)");
    build(&caller, PROVIDER, LEAF).unwrap()
}
/// Keep the ordered call census and left-to-right sum without a deep binary tree.
/// Each initializer adds one call to the previous Copy scalar binding.
fn flat_call_sum(prefix: &str, count: usize, argument: &str) -> String {
    if count == 0 {
        return "    0\n".to_owned();
    }
    let mut body = format!("    let sum0 = {prefix}0({argument});\n");
    for j in 1..count {
        body.push_str(&format!(
            "    let sum{j} = sum{} + {prefix}{j}({argument});\n",
            j - 1
        ));
    }
    body.push_str(&format!("    sum{}\n", count - 1));
    body
}
/// Definitions run high to low so sorted ID order differs from ordinal order.
fn dense_source(count: usize) -> String {
    let mut source = String::from("module scalar.dense;\n");
    for i in (0..count).rev() {
        let body = if i == 0 {
            "    n\n".to_owned()
        } else {
            flat_call_sum("f", i, "n")
        };
        source.push_str(&format!(
            "\n@id(\"dense.{i:03}\")\nfn f{i}(n: i64) -> i64\n{{\n{body}}}\n"
        ));
    }
    source
}
fn wide_source(count: usize) -> String {
    let calls = flat_call_sum("g", count, "0");
    let mut source =
        format!("module scalar.wide;\n\n@id(\"wide.main\")\nfn main() -> i64\n{{\n{calls}}}\n");
    for j in (0..count).rev() {
        source.push_str(&format!(
            "\n@id(\"wide.{j:03}\")\nfn g{j}(n: i64) -> i64\n{{\n    n\n}}\n"
        ));
    }
    source
}
/// Graph-mutant only: the caller's functions become an independently resolved
/// local program that never passes consumer source authentication.
fn with_local(source: &str) -> WorkspaceGraphBuild {
    let mut build = build(CALLER, PROVIDER, LEAF).unwrap();
    let parsed = crate::parse(source, "graph-local.spx").unwrap();
    let needs_entry = !parsed
        .functions
        .iter()
        .any(|function| function.name == "main");
    let checked_source = if needs_entry {
        format!("{source}\n@id(\"graph.fixture.entry\") fn main()->i64 {{0}}\n")
    } else {
        source.to_owned()
    };
    let mut program =
        hir::resolve(&crate::check(&checked_source, "graph-local.spx").unwrap()).unwrap();
    // The executable-only check entry is not part of the graph-mutant corpus.
    if needs_entry {
        program
            .functions
            .retain(|function| function.id.as_str() != "graph.fixture.entry");
    }
    module_mut(&mut build, "scalar.app").functions =
        crate::workspace_graph::checked_retention::Retained::Owned(program.functions);
    build
}

#[test]
fn chain_diamond_repeated_and_alias_closures_match_the_saved_oracle_exactly() {
    let chain = build(CALLER, PROVIDER, LEAF).unwrap();
    let shape = parity(&chain, "scalar.app").ok().unwrap();
    assert_eq!(
        shape.functions,
        [
            ptr(&chain, "scalar.app", "app.main"),
            ptr(&chain, "scalar.helper", "helper.step"),
            ptr(&chain, "scalar.leaf", "leaf.step"),
        ]
    );
    assert_eq!(shape.edges, [(0, 1), (1, 2)]);
    assert_eq!(shape.order, [0, 1, 2]);
    assert!(shape.acyclic);

    let diamond = diamond("leaf_step(n)");
    assert_eq!(schema(&diamond).unwrap(), "semaprax.graph.v77");
    let shape = parity(&diamond, "scalar.app").ok().unwrap();
    assert_eq!(
        shape.functions,
        [
            ptr(&diamond, "scalar.app", "app.main"),
            ptr(&diamond, "scalar.a", "a.step"),
            ptr(&diamond, "scalar.b", "b.step"),
            ptr(&diamond, "scalar.leaf", "leaf.step"),
        ]
    );
    assert_eq!(shape.edges, [(0, 1), (0, 2), (1, 3), (2, 3)]);
    assert_eq!(shape.order, [0, 2, 1, 3]);
    assert_eq!(shape.visits, 4);

    let repeated = repeated();
    assert_eq!(schema(&repeated).unwrap(), "semaprax.graph.v77");
    let shape = parity(&repeated, "scalar.app").ok().unwrap();
    assert_eq!(shape.functions.len(), 3);
    assert_eq!(shape.edges, [(0, 1), (0, 1), (1, 2), (1, 2)]);
    assert_eq!(shape.order, [0, 1, 2]);
    assert_eq!(shape.visits, 4);

    let aliases = aliases();
    let shape = parity(&aliases, "scalar.app").ok().unwrap();
    assert_eq!(
        shape.functions,
        [
            ptr(&aliases, "scalar.app", "app.main"),
            ptr(&aliases, "scalar.helper", "helper.step"),
            ptr(&aliases, "scalar.leaf", "leaf.step"),
        ]
    );
    assert_eq!(shape.edges, [(0, 1), (0, 2), (1, 2)]);
    assert_eq!(shape.order, [0, 1, 2]);
}

#[test]
fn self_and_two_module_cycles_and_duplicate_current_ids_keep_saved_verdicts() {
    let mut selfish = build(CALLER, PROVIDER, LEAF).unwrap();
    *call_mut(&mut module_mut(&mut selfish, "scalar.app").functions[0].body).unwrap() =
        hir::DeclarationId::new("app.main");
    let shape = parity(&selfish, "scalar.app").ok().unwrap();
    assert_eq!(shape.edges, [(0, 0)]);
    assert!(!shape.acyclic && shape.order.is_empty());

    // The consumer's current function takes the provider's callee identity;
    // the unchanged provider body then resolves back into the consumer.
    let mut mutual = build(CALLER, PROVIDER, LEAF).unwrap();
    module_mut(&mut mutual, "scalar.app").functions[0].id = hir::DeclarationId::new("leaf.step");
    let shape = parity(&mutual, "scalar.app").ok().unwrap();
    assert_eq!(
        shape.functions,
        [
            ptr(&mutual, "scalar.app", "leaf.step"),
            ptr(&mutual, "scalar.helper", "helper.step"),
        ]
    );
    assert_eq!(shape.edges, [(0, 1), (1, 0)]);
    assert!(!shape.acyclic && shape.order.is_empty());

    let mut dense = with_local(&dense_source(12));
    let functions = &mut module_mut(&mut dense, "scalar.app").functions;
    let first = functions
        .iter()
        .position(|f| f.id.as_str() == "dense.001")
        .unwrap();
    let mut retargeted = false;
    each_call(&mut functions[first].body, &mut |callee| {
        if !retargeted {
            *callee = hir::DeclarationId::new("dense.011");
            retargeted = true;
        }
    });
    assert!(retargeted);
    let shape = parity(&dense, "scalar.app").ok().unwrap();
    assert!(!shape.acyclic && shape.order.len() < shape.functions.len());

    let mut duplicate = with_local(&dense_source(6));
    let functions = &mut module_mut(&mut duplicate, "scalar.app").functions;
    let second = functions
        .iter()
        .position(|f| f.id.as_str() == "dense.002")
        .unwrap();
    functions[second].id = hir::DeclarationId::new("dense.001");
    assert_eq!(
        parity(&duplicate, "scalar.app").err().unwrap(),
        ["SPX-G410"]
    );
}

/// Graph-mutant: an equal later current ID stays in its arena ordinal but
/// unlinked. The retained lookup answers the saved first match, while a
/// discovery call to the shared ID still refuses like the saved lookup.
#[test]
fn shared_current_ids_resolve_to_the_saved_first_match_and_refuse_discovery() {
    let mut duplicate = with_local(&dense_source(6));
    let functions = &mut module_mut(&mut duplicate, "scalar.app").functions;
    let second = functions
        .iter()
        .position(|f| f.id.as_str() == "dense.002")
        .unwrap();
    functions[second].id = hir::DeclarationId::new("dense.001");
    let module = caller(&duplicate);
    let anchor = &module.functions[0];
    let mut resources =
        SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 26).unwrap();
    let mut path = graph::Path::new(&mut resources, anchor).unwrap();
    let directory = graph::Directory::new(module, &mut path, &mut resources, anchor).unwrap();
    let (arena, sorted, height) = directory.audit();
    assert_eq!(
        arena,
        module
            .functions
            .iter()
            .map(|f| f as *const ResolvedFunction)
            .collect::<Vec<_>>()
    );
    assert_eq!(sorted.len(), 5, "the later equal ID is not linked");
    assert!(height <= avl_bound(5));
    for id in [
        "dense.000",
        "dense.001",
        "dense.003",
        "dense.005",
        "dense.009",
    ] {
        assert_eq!(
            directory.resolve(id).map(|f| f as *const ResolvedFunction),
            oracle::linear(&module.functions, &[], id).map(|f| f as *const ResolvedFunction),
            "{id}"
        );
    }
    assert!(std::ptr::eq(
        directory.resolve("dense.001").unwrap(),
        &module.functions[second]
    ));
    assert_eq!(
        parity(&duplicate, "scalar.app").err().unwrap(),
        ["SPX-G410"]
    );
}

/// The insertion path scratch is initialized once per discovery after its
/// whole initialization is debited; one short refuses before it exists and
/// stays sticky. Each later insertion debits only the levels it writes, so
/// reusing the scratch costs every directory the same exact work.
#[test]
fn directory_path_scratch_is_debited_once_before_initialization() {
    let build = with_local(&dense_source(40));
    let module = caller(&build);
    let anchor = &module.functions[0];
    let fresh = |work| SelectedReplayPreparation::with_limits(anchor, work, 1 << 26).unwrap();
    let scratch = 2 * graph::PATH;

    let mut short = fresh(scratch - 1);
    assert!(graph::Path::new(&mut short, anchor).is_err());
    assert!(short.charge(anchor, 0, "probe").is_err(), "sticky");

    let mut wide = fresh(32_000_000);
    let mut path = graph::Path::new(&mut wide, anchor).unwrap();
    assert_eq!(wide.remaining(), 32_000_000 - scratch);
    let first = graph::Directory::new(module, &mut path, &mut wide, anchor).unwrap();
    let once = 32_000_000 - scratch - wide.remaining();
    let before = wide.remaining();
    let second = graph::Directory::new(module, &mut path, &mut wide, anchor).unwrap();
    assert_eq!(before - wide.remaining(), once, "reused scratch, same work");
    assert_eq!(first.audit(), second.audit());
    assert!(
        once < scratch * module.functions.len(),
        "no per-insertion scratch initialization"
    );

    let mut exact = fresh(scratch + once);
    let mut path = graph::Path::new(&mut exact, anchor).unwrap();
    assert!(graph::Directory::new(module, &mut path, &mut exact, anchor).is_ok());
    assert_eq!(exact.remaining(), 0);
    let mut short = fresh(scratch + once - 1);
    let mut path = graph::Path::new(&mut short, anchor).unwrap();
    assert!(graph::Directory::new(module, &mut path, &mut short, anchor).is_err());
    assert!(short.charge(anchor, 0, "probe").is_err(), "sticky");
}

#[test]
fn stale_missing_duplicate_and_retargeted_providers_keep_saved_verdicts() {
    for attack in 0..6 {
        let mut build = build(CALLER, PROVIDER, LEAF).unwrap();
        let refused = match attack {
            0 => {
                *call_mut(&mut module_mut(&mut build, "scalar.app").functions[0].body).unwrap() =
                    hir::DeclarationId::new("leaf.step");
                None
            }
            1 => {
                *call_mut(&mut module_mut(&mut build, "scalar.helper").functions[0].body)
                    .unwrap() = hir::DeclarationId::new("helper.step");
                None
            }
            2 => {
                build.hir.modules.retain(|m| m.module != "scalar.leaf");
                Some(true)
            }
            3 => {
                let mut another = self::build(CALLER, PROVIDER, LEAF).unwrap();
                let position = another
                    .hir
                    .modules
                    .iter()
                    .position(|m| m.module == "scalar.leaf")
                    .unwrap();
                build.hir.modules.push(another.hir.modules.remove(position));
                Some(true)
            }
            4 => {
                let mut another =
                    self::build(CALLER, PROVIDER, &LEAF.replace("n+1", "n+2")).unwrap();
                module_mut(&mut build, "scalar.leaf").scalar_helpers =
                    module_mut(&mut another, "scalar.leaf")
                        .scalar_helpers
                        .take();
                Some(true)
            }
            _ => {
                build.hir.declarations.get_mut("leaf.step").unwrap().module =
                    Some("scalar.helper".into());
                Some(true)
            }
        };
        let verdict = parity(&build, "scalar.app");
        if let Some(refused) = refused {
            assert_eq!(verdict.is_err(), refused, "provider attack {attack}");
        }
    }
}

#[test]
fn long_ids_and_wide_dense_graphs_match_the_saved_oracle_with_single_walks() {
    let prefix = "p".repeat(100);
    let long = |tail: &str| format!("{prefix}.{tail}");
    let caller = CALLER
        .replace("\"helper.step\"", &format!("\"{}\"", long("helper")))
        .replace("\"app.main\"", &format!("\"{}\"", long("main")));
    let provider = PROVIDER
        .replace("\"helper.step\"", &format!("\"{}\"", long("helper")))
        .replace("\"leaf.step\"", &format!("\"{}\"", long("leaf")));
    let leaf = LEAF.replace("\"leaf.step\"", &format!("\"{}\"", long("leaf")));
    let built = build(&caller, &provider, &leaf).unwrap();
    let shape = parity(&built, "scalar.app").ok().unwrap();
    assert_eq!(shape.edges, [(0, 1), (1, 2)]);
    assert_eq!(
        shape.functions[2],
        ptr(&built, "scalar.leaf", &long("leaf"))
    );

    // Graph-mutant algorithm evidence below: replaced caller HIR, graph only.
    let dense = with_local(&dense_source(40));
    let shape = parity(&dense, "scalar.app").ok().unwrap();
    assert_eq!(shape.functions.len(), 40);
    assert_eq!(shape.edges.len(), 40 * 39 / 2);
    assert_eq!(
        shape.edges,
        (0..40)
            .flat_map(|from| (from + 1..40).rev().map(move |to| (from, to)))
            .collect::<Vec<_>>(),
        "every lower-numbered callee in authored call order"
    );
    assert!(shape.acyclic);
    assert_eq!(shape.visits, shape.edges.len());
    assert_eq!(shape.walks, 40);

    let wide = with_local(&wide_source(128));
    let shape = parity(&wide, "scalar.app").ok().unwrap();
    // `g0` is defined last, so call order g0..g127 targets ordinals 128..1.
    assert_eq!(shape.functions.len(), 129);
    assert_eq!(
        shape.edges,
        (1..=128).rev().map(|to| (0, to)).collect::<Vec<_>>()
    );
    assert_eq!(shape.order, (0..=128).collect::<Vec<_>>());

    // Equal long prefixes make every directory comparison scan 8 KiB.
    let mut long_dense = with_local(&dense_source(8));
    let prefix = "z".repeat(8192);
    for function in &mut module_mut(&mut long_dense, "scalar.app").functions {
        function.id = hir::DeclarationId::new(format!("{prefix}{}", function.id.as_str()));
        each_call(&mut function.body, &mut |callee| {
            *callee = hir::DeclarationId::new(format!("{prefix}{}", callee.as_str()));
        });
    }
    let shape = parity(&long_dense, "scalar.app").ok().unwrap();
    assert_eq!(shape.edges.len(), 8 * 7 / 2);
    assert!(shape.acyclic);

    let names = (0..12)
        .map(|k| {
            (
                format!("w{k}.spx"),
                format!("module scalar.w{k};\n@id(\"w{k}.step\") fn step(n:i64)->i64 {{n+{k}}}\n"),
            )
        })
        .collect::<Vec<_>>();
    let imports = (0..12)
        .map(|k| format!("use function @id(\"w{k}.step\") from scalar.w{k} as w{k};\n"))
        .collect::<String>();
    let sum = (0..12)
        .map(|k| format!("w{k}(0)"))
        .collect::<Vec<_>>()
        .join("+");
    let caller = format!(
        "module scalar.app;\n{imports}@id(\"app.main\") fn main()->i64 {{let label=str\"é\\u{{0}}\";let outcome={sum};let mut tick=0usize;while tick<1usize {{tick=tick+1usize;return outcome;}} 0}}\n"
    );
    let mut sources = vec![("caller.spx", caller.as_str())];
    sources.extend(names.iter().map(|(p, s)| (p.as_str(), s.as_str())));
    let external = build_sources(&sources).unwrap();
    let shape = parity(&external, "scalar.app").ok().unwrap();
    assert_eq!(shape.functions.len(), 13);
    for k in 0..12 {
        assert_eq!(
            shape.functions[k + 1],
            ptr(&external, &format!("scalar.w{k}"), &format!("w{k}.step"))
        );
    }
    assert_eq!(shape.order[1..], (1..=12).rev().collect::<Vec<_>>()[..]);
}

#[test]
fn production_entry_counts_one_walk_per_member_and_one_visit_per_edge() {
    for build in [
        build(CALLER, PROVIDER, LEAF).unwrap(),
        diamond("leaf_step(n)"),
        repeated(),
        aliases(),
    ] {
        let saved = parity(&build, "scalar.app").ok().unwrap();
        let module = caller(&build);
        let anchor = &module.functions[0];
        let mut resources =
            SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 26).unwrap();
        let context = authenticate(
            module,
            &build.hir.modules,
            &build.hir.declarations,
            &mut resources,
        )
        .unwrap();
        assert_eq!(context.count, saved.functions.len());
        assert_eq!(context.walks, context.count);
        assert_eq!(context.edges, saved.edges.len());
        for (helper, expected) in context.helpers.iter().zip(&saved.functions[1..]) {
            assert!(std::ptr::eq(helper.function, *expected));
        }
    }
}

#[test]
fn diamond_repeated_and_alias_entries_to_final_drop_are_exact_and_one_short() {
    for build in [diamond("leaf_step(n)"), repeated(), aliases()] {
        let (wide, allocation) = measured(&build, 32_000_000, 1 << 26);
        let (work, bytes) = wide.unwrap();
        assert!(
            !allocation.invalid && allocation.allocated <= bytes && allocation.highwater <= bytes
        );
        assert_eq!(allocation.live, 0);
        let (exact, allocation) = measured(&build, work, bytes);
        assert_eq!(exact.unwrap(), (work, bytes));
        assert!(
            !allocation.invalid && allocation.allocated <= bytes && allocation.highwater <= bytes
        );
        assert_eq!(allocation.live, 0);
        assert!(measured(&build, work - 1, bytes).0.is_err());
        assert!(measured(&build, work, bytes - 1).0.is_err());
    }
}

/// Replace one side's inventory with the prior generation of a longer body.
fn stale(build: &mut WorkspaceGraphBuild, side_name: &str) {
    let body = |name: &str| {
        if name == side_name {
            "leaf_step(n)+1"
        } else {
            "leaf_step(n)"
        }
    };
    let mut altered = build_sources(&[
        ("caller.spx", DIAMOND_CALLER),
        ("a.spx", &side("a", body("a"))),
        ("b.spx", &side("b", body("b"))),
        ("leaf.spx", LEAF),
    ])
    .unwrap();
    let module = format!("scalar.{side_name}");
    module_mut(build, &module).scalar_helpers =
        module_mut(&mut altered, &module).scalar_helpers.take();
}
fn refused(build: &WorkspaceGraphBuild) -> usize {
    let module = caller(build);
    let anchor = &module.functions[0];
    let mut resources =
        SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 26).unwrap();
    assert!(authenticate(
        module,
        &build.hir.modules,
        &build.hir.declarations,
        &mut resources
    )
    .is_err());
    let remaining = resources.remaining();
    assert_eq!(
        resources
            .charge(anchor, 0, "later provider retry")
            .unwrap_err()
            .code,
        "SPX-H006"
    );
    assert_eq!(
        resources
            .reserve_array::<usize>(anchor, 0, "later provider retry backing")
            .unwrap_err()
            .code,
        "SPX-H006"
    );
    assert_eq!(resources.remaining(), remaining, "no refund or mint");
    32_000_000 - remaining
}
#[test]
fn later_provider_failure_is_sticky_and_cold_rebuilt_sources_retry_with_exact_parity() {
    let mut first = diamond("leaf_step(n)");
    stale(&mut first, "a");
    let mut later = diamond("leaf_step(n)");
    stale(&mut later, "b");
    let early = refused(&first);
    let spent = refused(&later);
    // The later provider fails only after the first was fully authenticated
    // and receipted on the same owner, so its refusal retains strictly more work.
    assert!(spent > early && early > 0);
    let module = caller(&later);
    let anchor = &module.functions[0];
    let mut short = SelectedReplayPreparation::with_limits(anchor, spent - 1, 1 << 26).unwrap();
    assert!(authenticate(
        module,
        &later.hir.modules,
        &later.hir.declarations,
        &mut short
    )
    .is_err());
    assert_eq!(
        short.charge(anchor, 0, "one-short retry").unwrap_err().code,
        "SPX-H006"
    );

    let cold = diamond("leaf_step(n)");
    let rebuilt = diamond("leaf_step(n)");
    let (cold_measure, _) = measured(&cold, 32_000_000, 1 << 26);
    let (rebuilt_measure, allocation) = measured(&rebuilt, 32_000_000, 1 << 26);
    let (work, bytes) = cold_measure.unwrap();
    assert_eq!(rebuilt_measure.unwrap(), (work, bytes));
    assert_eq!(allocation.live, 0);
    assert!(
        spent < work,
        "a refused later provider never exceeds the full entry"
    );
    assert_eq!(measured(&rebuilt, work, bytes).0.unwrap(), (work, bytes));
    assert!(measured(&rebuilt, work - 1, bytes).0.is_err());
    let ids = |build: &WorkspaceGraphBuild| {
        let shape = parity(build, "scalar.app").ok().unwrap();
        let functions = shape
            .functions
            .iter()
            .map(|f| {
                build
                    .hir
                    .modules
                    .iter()
                    .flat_map(|m| m.functions.iter().map(move |g| (m.module.as_str(), g)))
                    .find(|(_, g)| std::ptr::eq(*g, *f))
                    .map(|(m, g)| format!("{m}/{}", g.id.as_str()))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        (functions, shape.edges, shape.order)
    };
    assert_eq!(ids(&cold), ids(&rebuilt));
}
