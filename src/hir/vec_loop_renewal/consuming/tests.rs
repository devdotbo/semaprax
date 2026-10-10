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
            plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V13.to_owned();
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
