use super::*;
use crate::cleanup_plan::CleanupTransition;

pub(super) const SOURCE: &str = r#"module owned_renewal;
@id("row") record Row {@id("row.n") n:i64,@id("row.text") text:string,@id("row.bytes") bytes:Bytes,}
@id("main") fn main()->i64 {
 let mut words=vec_with_capacity<Row>(2usize);
 let later=vec_with_capacity<Row>(1usize);
 let first=vec_push<Row>(vec_with_capacity<Row>(2usize),Row{n:7,text:"ab",bytes:bytes_zeroed(1usize)});
 let inputs=vec_push<Row>(first,Row{n:7,text:"ab",bytes:bytes_zeroed(1usize)});
 for own item in vec_into_iter<Row>(inputs) {words=vec_push<Row>(words,item);0}
 if vec_len<Row>(words)==2usize && vec_len<Row>(later)==0usize {29}else{0}
}
"#;
fn checked() -> super::super::ResolvedProgram {
    let source = crate::check(SOURCE, "consuming-record-renewal.spx").unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    crate::hir::validate(&program).unwrap();
    program
}
#[test]
fn consuming_row_reuses_canonical_reservation_and_replays_missing_or_forged_epochs() {
    let program = checked();
    let function = &program.functions[0];
    assert!(!super::super::requires(function));
    assert_eq!(
        function.cleanup_plan.schema,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V15
    );
    assert_eq!(sites(&program, function).len(), 1);
    assert_eq!(super::super::bindings_in(&program, function).len(), 1);
    assert!(!requires_with_admission(function, |_, _| None));
    assert!(!requires_with_admission(function, |_, _| Some(
        OwnershipMode::Value
    )));
    let reservations = function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|b| &b.transitions)
        .filter(|t| matches!(t, CleanupTransition::ReserveRenewal { .. }))
        .count();
    assert_eq!(reservations, 1);
    for mutation in 0..4 {
        let mut forged = program.clone();
        let plan = &mut forged.functions[0].cleanup_plan;
        if mutation == 3 {
            plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V13;
        }
        for block in &mut plan.blocks {
            if mutation == 0 {
                block
                    .transitions
                    .retain(|t| !matches!(t, CleanupTransition::ReserveRenewal { .. }));
            }
            for transition in &mut block.transitions {
                if let CleanupTransition::Renew {
                    at,
                    source,
                    destination,
                } = transition
                {
                    if mutation == 1 {
                        *transition = CleanupTransition::Transfer {
                            at: at.clone(),
                            source: source.clone(),
                            destination: destination.clone(),
                        };
                    } else if mutation == 2 {
                        *destination = source.clone();
                    }
                }
            }
        }
        assert!(
            crate::cleanup_plan::validate_program(&forged).is_err(),
            "forgery {mutation}"
        );
    }
}
#[test]
fn actual_yielded_item_seed_mutability_and_record_authority_are_independent() {
    let program = checked();
    let function = &program.functions[0];
    let selected = sites(&program, function);
    let site = selected.values().next().unwrap();
    let mut callback_count = 0;
    assert!(scan(
        function,
        &|op, element| super::super::element_mode(&program.declarations, op, element),
        |_| callback_count += 1
    ));
    assert_eq!(callback_count, 1);
    for mutation in 0..4 {
        let mut call = site.at.clone();
        let ResolvedExprKind::Call {
            args,
            type_arguments,
            instance,
            ..
        } = &mut call.kind
        else {
            panic!("call")
        };
        match mutation {
            0 => {
                args[0].ownership = OwnershipMode::Borrow;
            }
            1 => {
                type_arguments[0] = ResolvedType::I64;
            }
            2 => {
                args[1].ownership = OwnershipMode::Borrow;
            }
            _ => {
                *instance = Some(crate::hir::FunctionInstanceId::derive(
                    &crate::hir::DeclarationId::new("forged"),
                    &[],
                ));
            }
        }
        assert!(!super::super::same_cell(
            site.binding,
            &call,
            Some(&|op, element| super::super::element_mode(&program.declarations, op, element))
        ));
    }
    let mut drift = program.clone();
    super::super::forge_record_field_for_test(
        &mut drift.declarations,
        &crate::hir::DeclarationId::new("row"),
        ResolvedType::Bytes,
    );
    assert!(!requires(&drift, function));
    assert!(crate::hir::validate(&drift).is_err());
}
#[test]
fn canonical_graph_cache_and_frozen_projection_refuse_claimed_record_renewal() {
    let source = crate::check(SOURCE, "consuming-record-renewal.spx").unwrap();
    let canonical = crate::format::canonical(&source);
    let round = crate::check(&canonical, "consuming-record-renewal.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&round));
    let json = crate::graph::to_json(&source).unwrap();
    crate::graph::verify_json(&source, &json).unwrap();
    let document: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(document["schema"], "semaprax.graph.v79");
    assert_eq!(
        document["consuming_owned_record_renewal"]["updates"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        document["vec_loop_renewal"]["updates"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    for field in ["condition", "body", "item", "at", "binding", "execution"] {
        let mut forged = document.clone();
        forged["consuming_owned_record_renewal"]["updates"][0][field] = "forged".into();
        assert!(
            crate::graph::verify_json(&source, &forged.to_string()).is_err(),
            "{field}"
        );
    }
    assert!(crate::graph::verify_json(
        &source,
        &json.replace("semaprax.graph.v79", "semaprax.graph.v78")
    )
    .is_err());
    assert!(crate::graph::to_legacy_json(&source).is_err());
    let program = checked();
    assert_eq!(
        crate::graph::graph_schema_from_parts_and_instances(
            &program.interfaces,
            &program.types,
            &program.functions,
            &program.function_templates,
            &program.function_instances
        )
        .unwrap_err()
        .code,
        "SPX-G410"
    );
    let program = checked();
    let encoded = crate::cache_codec::encode(&program.functions[0].cleanup_plan).unwrap();
    let decoded: crate::cleanup_plan::CleanupPlan = crate::cache_codec::decode(&encoded).unwrap();
    assert_eq!(decoded, program.functions[0].cleanup_plan);
}

// This fixture-specific mutator must find the actual generated wrapper. It never
// stands in for the production exhaustive child visitor or source authority.
fn mutate_wrapper(
    expression: &mut ResolvedExpr,
    action: &mut impl FnMut(&mut ResolvedBinding, &mut ResolvedExpr),
) -> bool {
    let ResolvedExprKind::Block { statements, tail } = &mut expression.kind else {
        return false;
    };
    if matches!(
        statements.as_slice(),
        [
            ResolvedStatement::Let { mutable: true, .. },
            ResolvedStatement::While { .. }
        ]
    ) {
        let ResolvedStatement::Let { binding, value, .. } = &mut statements[0] else {
            unreachable!()
        };
        action(binding, value);
        return true;
    }
    for statement in statements {
        let value = match statement {
            ResolvedStatement::Let { value, .. } | ResolvedStatement::Assign { value, .. } => value,
            _ => continue,
        };
        if mutate_wrapper(value, action) {
            return true;
        }
    }
    mutate_wrapper(tail, action)
}
#[test]
fn forged_seed_identity_type_and_ownership_cannot_select_consuming_authority() {
    for mutation in 0..3 {
        let mut program = checked();
        assert!(mutate_wrapper(
            &mut program.functions[0].body,
            &mut |binding, seed| {
                let ResolvedExprKind::Call {
                    callee,
                    type_arguments,
                    args,
                    ..
                } = &mut seed.kind
                else {
                    panic!("seed")
                };
                match mutation {
                    0 => *callee = crate::hir::DeclarationId::new("forged.next"),
                    1 => type_arguments[0] = ResolvedType::I64,
                    _ => args[0].ownership = OwnershipMode::Borrow,
                }
                assert_eq!(binding.ty, seed.ty);
            }
        ));
        assert!(!requires(&program, &program.functions[0]));
        assert!(crate::hir::validate(&program).is_err(), "seed {mutation}");
    }
}

#[test]
fn conditional_owned_row_transformation_reserves_the_destination_family_independently() {
    let source = SOURCE
        .replace("@id(\"main\")", "@id(\"output\") record Output {@id(\"output.n\") n:i64,@id(\"output.text\") text:string,@id(\"output.bytes\") bytes:Bytes,}\n@id(\"main\")")
        .replace("let mut words=vec_with_capacity<Row>", "let mut words=vec_with_capacity<Output>")
        .replace("words=vec_push<Row>(words,item);0", "let _=match own item {Row{n,text,bytes}=>{if n==7 {words=vec_push<Output>(words,Output{n:n+1,text:text,bytes:bytes});}0},};0")
        .replace("vec_len<Row>(words)", "vec_len<Output>(words)");
    let checked = crate::check(&source, "cross-row-renewal.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    crate::hir::validate(&program).unwrap();
    let function = &program.functions[0];
    let selected = sites(&program, function);
    assert_eq!(selected.len(), 1);
    let site = selected.values().next().unwrap();
    let ResolvedType::Nominal { arguments, .. } = &site.binding.ty else {
        panic!("destination vector")
    };
    let [destination] = arguments.as_slice() else {
        panic!("destination element")
    };
    assert_ne!(destination, &site.item.ty);
    assert_eq!(
        function
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .filter(|transition| matches!(transition, CleanupTransition::ReserveRenewal { .. }))
            .count(),
        1
    );
    // Either family being absent or borrowed must independently close the
    // additive classifier, even when the other exact declaration is admitted.
    for refused in [&site.item.ty, destination] {
        for mode in [None, Some(OwnershipMode::Borrow)] {
            assert!(!requires_with_admission(function, |op, element| {
                if element == refused {
                    mode
                } else {
                    super::super::element_mode(&program.declarations, op, element)
                }
            }));
        }
    }
    let mut forged = program.clone();
    for block in &mut forged.functions[0].cleanup_plan.blocks {
        block
            .transitions
            .retain(|transition| !matches!(transition, CleanupTransition::ReserveRenewal { .. }));
    }
    assert!(crate::cleanup_plan::validate_program(&forged).is_err());
    let mut forged = program.clone();
    super::super::forge_record_field_for_test(
        &mut forged.declarations,
        &crate::hir::DeclarationId::new("output"),
        ResolvedType::Bytes,
    );
    assert!(crate::hir::validate(&forged).is_err());
}

// Independent pre-cursor authored-order oracle, including closure pruning and
// the original MAX_DEPTH + 1 frame boundary. It does not share ChildCursor.
fn old_visit<'a>(root: &'a ResolvedExpr, mut visitor: impl FnMut(&'a ResolvedExpr)) -> bool {
    let mut stack = [None; crate::cache_codec::MAX_DEPTH + 1];
    stack[0] = Some((root, 0usize));
    let mut len = 1;
    while len != 0 {
        let (expr, next) = stack[len - 1].expect("occupied consuming renewal frame");
        if next == 0 {
            visitor(expr);
        }
        let child = if matches!(expr.kind, ResolvedExprKind::Closure { .. }) {
            None
        } else {
            crate::hir::lexical_exit::child(expr, next)
        };
        if let Some(child) = child {
            if len == stack.len() {
                return false;
            }
            stack[len - 1] = Some((expr, next + 1));
            stack[len] = Some((child, 0));
            len += 1;
        } else {
            stack[len - 1] = None;
            len -= 1;
        }
    }
    true
}
fn compare_walk(root: &ResolvedExpr) -> bool {
    let mut old = Vec::new();
    let mut current = Vec::new();
    let expected = old_visit(root, |node| old.push(node as *const ResolvedExpr));
    let actual = visit(root, |node| current.push(node as *const ResolvedExpr));
    assert_eq!(actual, expected);
    assert_eq!(current, old, "exact borrowed authored expression order");
    actual
}
#[test]
fn consuming_cursor_keeps_old_authored_pointer_order_and_wide_statement_edges() {
    let program = checked();
    assert!(compare_walk(&program.functions[0].body));
    let mut source = String::from("module wide; @id(\"main\") fn main()->i64 {\n");
    for index in 0..1024 {
        source.push_str(&format!("let pad{index}={index};\n"));
    }
    source.push_str("if true {1}else{2}}\n");
    let parsed = crate::check(&source, "wide-consuming-cursor.spx").unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    assert!(compare_walk(&program.functions[0].body));
    let mut marked = program.functions[0].body.clone();
    let ResolvedExprKind::Block { statements, tail } = marked.kind else {
        panic!("authored marker payload");
    };
    marked.kind = ResolvedExprKind::VecConstructor {
        element: ResolvedType::I64,
        count: 1024,
        statements,
        tail,
    };
    assert!(compare_walk(&marked));
    let mut closure = marked.clone();
    closure.kind = ResolvedExprKind::Closure {
        parameters: Vec::new(),
        captures: Vec::new(),
        body: Box::new(marked),
    };
    assert!(compare_walk(&closure));
    let mut closure_visits = 0;
    assert!(visit(&closure, |_| closure_visits += 1));
    assert_eq!(
        closure_visits, 1,
        "closure execution remains a separate function"
    );
    let ResolvedExprKind::Block { statements, .. } = &program.functions[0].body.kind else {
        panic!("authored block");
    };
    assert_eq!(statements.len(), 1024);
    // Independent statement-prefix cost of the old indexed Block enumerator:
    // retrieving each one-child statement and the final tail crosses this many
    // statement entries. The cursor crosses each statement once.
    assert_eq!((1..=1024usize).sum::<usize>() + 1024, 525_824);
    assert_eq!(
        statements
            .iter()
            .map(ResolvedStatement::child_count)
            .sum::<usize>(),
        1024
    );
}
#[test]
fn consuming_cursor_precharges_exact_frames_and_one_short_visits_nothing() {
    let program = checked();
    let root = &program.functions[0].body;
    let exact = std::mem::size_of::<
        [Option<crate::hir::lexical_exit::ChildCursor<'_>>; crate::cache_codec::MAX_DEPTH + 1],
    >();
    let ((complete, count), overflow, charged) =
        crate::bounded_output::with_limit_usage(exact, || {
            let mut count = 0;
            let complete = visit(root, |_| count += 1);
            (complete, count)
        });
    assert!(complete && count != 0 && !overflow);
    assert_eq!(charged, exact);
    let ((complete, count), overflow, charged) =
        crate::bounded_output::with_limit_usage(exact - 1, || {
            let mut count = 0;
            let complete = visit(root, |_| count += 1);
            (complete, count)
        });
    assert!(!complete && overflow);
    assert_eq!(count, 0);
    assert_eq!(charged, 0);
}
#[test]
fn consuming_cursor_retains_exact_depth_refusal_and_detached_node_identity() {
    let parsed = crate::check(
        "module depth; @id(\"main\") fn main()->i64 {0}",
        "cursor-depth.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    let mut root = program.functions[0].body.clone();
    // Use a leaf so the independently authored depth is unambiguous.
    root.kind = ResolvedExprKind::Int(0);
    for _ in 0..crate::cache_codec::MAX_DEPTH {
        let mut parent = root.clone();
        parent.kind = ResolvedExprKind::Block {
            statements: Vec::new(),
            tail: Box::new(root),
        };
        root = parent;
    }
    assert!(compare_walk(&root));
    let mut parent = root.clone();
    parent.kind = ResolvedExprKind::Block {
        statements: Vec::new(),
        tail: Box::new(root),
    };
    assert!(!compare_walk(&parent));
    let mut scratch = [None; crate::cache_codec::MAX_DEPTH + 1];
    assert!(!visit_with(&parent, |_| {}, &mut scratch));
    assert!(scratch.iter().all(Option::is_none));
    assert!(visit_with(&program.functions[0].body, |_| {}, &mut scratch));
    assert!(scratch.iter().all(Option::is_none));
    let detached = parent.clone();
    assert!(!std::ptr::eq(&parent, &detached));
    let mut first = None;
    assert!(!visit(&detached, |node| {
        first.get_or_insert(node as *const ResolvedExpr);
    }));
    assert_eq!(first, Some(&detached as *const ResolvedExpr));
}

#[test]
fn consuming_publication_reuses_one_paid_cursor_and_settles_without_heap_backing() {
    let source = crate::check(
        "module scratch; @id(\"row\") record Row {@id(\"row.text\") text:string,} @id(\"main\") fn main()->i64 {if true {1}else{2}}",
        "consuming-scratch.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let function = &program.functions[0];
    let exact = (crate::cache_codec::MAX_DEPTH + 1)
        * std::mem::size_of::<Option<crate::hir::lexical_exit::ChildCursor<'_>>>();
    let mut scratch = [None; crate::cache_codec::MAX_DEPTH + 1];
    let mut first = 0;
    let mut second = 0;
    assert!(visit_with(&function.body, |_| first += 1, &mut scratch));
    assert!(scratch.iter().all(Option::is_none));
    assert!(visit_with(&function.body, |_| second += 1, &mut scratch));
    let mut old = 0;
    assert!(old_visit(&function.body, |_| old += 1));
    assert_eq!((first, second), (old, old));
    assert!(scratch.iter().all(Option::is_none));
    // The active ledger is fixed harness context, outside the cursor observer.
    let ((complete, allocation), overflow, charged) =
        crate::bounded_output::with_limit_usage(exact, || {
            crate::test_allocation_observer::measure(|| {
                let mut found = BTreeMap::new();
                append_bindings(&program, function, &mut found);
                found.is_empty()
            })
        });
    assert!(complete && !overflow);
    assert_eq!(
        charged, exact,
        "two independent scans have one held storage payer"
    );
    assert!(!allocation.invalid);
    assert_eq!((allocation.allocated, allocation.live), (0, 0));
    let ((empty, retried_empty), overflow, charged) =
        crate::bounded_output::with_limit_usage(exact - 1, || {
            let mut found = BTreeMap::new();
            append_bindings(&program, function, &mut found);
            let empty = found.is_empty();
            append_bindings(&program, function, &mut found);
            (empty, found.is_empty())
        });
    assert!(empty && retried_empty && overflow);
    assert_eq!(
        charged, 0,
        "refusal publishes no partially authenticated binding"
    );
}

#[test]
fn absent_current_explicit_record_keeps_old_negative_without_cursor_reservation() {
    let parsed = crate::check(
        "module absent; @id(\"main\") fn main()->i64 {let mut n=0;while n<3 {n=n+1;0} n}",
        "absent-consuming-record.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    let function = &program.functions[0];
    assert!(!has_record_candidate(&program));
    let mut old_sites = 0;
    assert!(scan(
        function,
        &|op, element| super::super::element_mode(&program.declarations, op, element),
        |_| old_sites += 1
    ));
    assert_eq!(
        old_sites, 0,
        "full independent old scan has no positive proof"
    );
    // The fixed ledger context is established before observing the collectors.
    // Its Rc is test setup, not consuming-renewal traversal storage.
    let ((empty, observed), overflow, charged) = crate::bounded_output::with_limit_usage(0, || {
        crate::test_allocation_observer::measure(|| {
            let mut found = BTreeMap::new();
            append_bindings(&program, function, &mut found);
            found.is_empty() && sites(&program, function).is_empty()
        })
    });
    assert!(empty && !overflow);
    assert_eq!(
        charged, 0,
        "absent record needs no physical expression cursor"
    );
    assert!(!observed.invalid);
    assert_eq!(
        (observed.allocations, observed.allocated, observed.live),
        (0, 0, 0)
    );
    // An actual demanded record retains the complete proof and reservation;
    // declaration presence alone cannot authorize any renewal site.
    let record_program = checked();
    let record_function = &record_program.functions[0];
    assert!(has_record_candidate(&record_program));
    let mut old_sites = 0;
    assert!(scan(
        record_function,
        &|op, element| super::super::element_mode(&record_program.declarations, op, element),
        |_| old_sites += 1
    ));
    assert_eq!(old_sites, 1);
    assert_eq!(sites(&record_program, record_function).len(), old_sites);
    let (sites, overflow, charged) =
        crate::bounded_output::with_limit_usage(0, || sites(&record_program, record_function));
    assert!(sites.is_empty() && overflow);
    assert_eq!(
        charged, 0,
        "record candidate cannot bypass physical proof scratch"
    );
}
