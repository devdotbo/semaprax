//! Explicit lookups and row-derived reads have distinct, independently replayed forms.
use super::*;
use crate::hir::VecFieldReadOrigin;

const SOURCE: &str = r#"module app.field_origin;
@id("row") record Row { @id("row.text") text:string, @id("row.number") number:i64, }
@id("ascii") fn ascii(text:borrow str)->bool {
 let bytes=str_as_bytes(text);
 for byte in bytes { if byte<65u8 || byte>90u8 { return false; }; 0 }
 true
}
@id("reader") fn reader(rows:borrow Vec<Row>)->i64 {
 let mut seen=0;
 for borrow row at index in rows {
  let current=row.text;
  if index>0usize { let previous=vec_field<Row>(rows,index-1usize,"text"); if ascii(current) && str_len_bytes(previous)>=0 { seen=seen+1; } }
  if index==2usize { break; }
  0
 }
 seen
}
@id("app.main") fn main()->i64 {0}
"#;
fn resolved() -> ResolvedProgram {
    let ast = crate::check(SOURCE, "field-origin.spx").unwrap();
    crate::hir::resolve(&ast).unwrap()
}
fn reader(program: &ResolvedProgram) -> &crate::hir::ResolvedFunction {
    program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "reader")
        .unwrap()
}
fn region(expression: &ResolvedExpr) -> &ResolvedExpr {
    if matches!(&expression.kind, ResolvedExprKind::Block { statements, .. } if matches!(statements.first(), Some(ResolvedStatement::Let { binding, .. }) if binding.name == "#for-borrow-length"))
    {
        return expression;
    }
    let mut pending = vec![expression];
    while let Some(value) = pending.pop() {
        if matches!(&value.kind, ResolvedExprKind::Block { statements, .. } if matches!(statements.first(), Some(ResolvedStatement::Let { binding, .. }) if binding.name == "#for-borrow-length"))
        {
            return value;
        }
        push_children(value, &mut pending);
    }
    panic!("record traversal region")
}
fn read_mut(
    expression: &mut ResolvedExpr,
    selected: VecFieldReadOrigin,
) -> Option<&mut ResolvedExpr> {
    if matches!(&expression.kind, ResolvedExprKind::VecFieldRead { origin, .. } if *origin == selected)
    {
        return Some(expression);
    }
    match &mut expression.kind {
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    ResolvedStatement::While {
                        condition, body, ..
                    }
                    | ResolvedStatement::LexicalWhile {
                        condition, body, ..
                    } => {
                        if let Some(found) = read_mut(condition, selected) {
                            return Some(found);
                        }
                        if let Some(found) = read_mut(body, selected) {
                            return Some(found);
                        }
                    }
                    _ => {
                        if let Some(found) = read_mut(statement.value_mut(), selected) {
                            return Some(found);
                        }
                    }
                }
            }
            read_mut(tail, selected)
        }
        ResolvedExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            if let Some(found) = read_mut(then_branch, selected) {
                return Some(found);
            }
            read_mut(else_branch, selected)
        }
        _ => None,
    }
}
#[test]
fn composed_read_forms_roundtrip_without_rewriting_the_original_receiver() {
    let program = resolved();
    crate::hir::validate(&program).unwrap();
    let expression = region(&reader(&program).body);
    assert!(inspect(&program, expression).unwrap().is_some());
    let ((), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let payer = crate::bounded_output::cfg_reservation::CfgReservation::acquire().unwrap();
        let mut current = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        current.allocation = Some(&payer);
        assert!(paid::inspect_counted(&program, expression, &mut current)
            .unwrap()
            .is_some());
        let mut old = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        old.allocation = Some(&payer);
        // This exact old receiver genuinely rejects the newly admitted ordinary
        // previous-row lookup. It is not rewritten to simulate the new rule.
        let error = match paid_legacy::inspect_counted(&program, expression, &mut old) {
            Err(error) => error,
            Ok(_) => panic!("old receiver admitted an ordinary previous-row lookup"),
        };
        assert_eq!(
            error.message,
            "record traversal length, index, nominal reads or whole-root freeze proof is invalid"
        );
    });
    assert!(!refused);
    assert_eq!(
        include_str!("paid_legacy.rs").replace("                origin: _,\n", ""),
        include_str!(
            "../../../docs/evidence/catalog-record-freeze-composition/original-paid-traversal.txt"
        )
    );
    let original_resolver =
        include_str!("../../hir/borrowed_root_oracle/resolve_expr/vec_field.rs");
    let resolver_inverse = original_resolver
        .replace(
            "                origin: crate::hir::VecFieldReadOrigin::Explicit,\n",
            "",
        )
        .replace("                origin: *origin,\n", "")
        .replace("            origin,\n", "");
    assert_eq!(resolver_inverse, include_str!("../../../docs/evidence/catalog-record-freeze-composition/original-borrowed-root-vec-field-resolver.txt"));
    let ast = crate::check(SOURCE, "field-origin.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let restored_ast = crate::check(&canonical, "field-origin.spx").unwrap();
    assert_eq!(crate::format::canonical(&restored_ast), canonical);
    let graph = crate::graph::to_json(&restored_ast).unwrap();
    assert!(graph.starts_with("{\"schema\":\"semaprax.graph.v85\""));
    assert!(graph.contains("\"source_form\":\"explicit\""));
    assert!(graph.contains("\"source_form\":\"traversal\""));
    crate::graph::verify_json(&restored_ast, &graph).unwrap();
    assert!(crate::graph::to_legacy_json(&restored_ast).is_err());
    assert!(crate::graph::reject_evidence_schema("semaprax.graph.v85").is_err());
    let forged_graph = graph.replacen(
        "\"source_form\":\"explicit\"",
        "\"source_form\":\"traversal\"",
        1,
    );
    assert_ne!(forged_graph, graph);
    assert!(crate::graph::verify_json(&restored_ast, &forged_graph).is_err());
    let wire = crate::cache_codec::encode(&program).unwrap();
    let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
    assert_eq!(restored, program);
    crate::hir::validate(&restored).unwrap();
    assert_eq!(crate::cache_codec::encode(&restored).unwrap(), wire);
}
#[test]
fn row_index_and_explicit_type_root_field_and_freeze_forgeries_remain_refused() {
    let original = resolved();
    for attack in 0..6 {
        let mut forged = original.clone();
        let function = forged
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "reader")
            .unwrap();
        let selected = if attack == 0 {
            VecFieldReadOrigin::Traversal
        } else {
            VecFieldReadOrigin::Explicit
        };
        let expression = read_mut(&mut function.body, selected).unwrap();
        let ResolvedExprKind::VecFieldRead {
            element,
            field,
            args,
            ..
        } = &mut expression.kind
        else {
            unreachable!()
        };
        match attack {
            0 => args[1].kind = ResolvedExprKind::Usize(0),
            1 => args[1].ty = ResolvedType::I64,
            2 => args[1].ownership = OwnershipMode::Own,
            3 => {
                let ResolvedExprKind::Place(place) = &mut args[0].kind else {
                    unreachable!()
                };
                place.root = ValueId::intrinsic_parameter("forged.root", 0);
            }
            4 => *element = ResolvedType::I64,
            _ => *field = crate::hir::DeclarationId::new("foreign.text"),
        }
        let error = crate::hir::validate(&forged).unwrap_err();
        assert!(
            matches!(error.code, "SPX-H006" | "SPX-T284"),
            "attack {attack}: {error:?}"
        );
        let wire = crate::cache_codec::encode(&forged).unwrap();
        let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
        assert!(crate::hir::validate(&restored).is_err(), "attack {attack}");
    }
    let owned_source = SOURCE.replace("fn reader(rows:borrow Vec<Row>)->i64 {", "fn reader()->i64 { let empty=vec_with_capacity<Row>(1usize); let rows=vec_push<Row>(empty,Row{text:\"A\",number:1});");
    let ast = crate::check(&owned_source, "owned-field-origin.spx").unwrap();
    let mut owned = crate::hir::resolve(&ast).unwrap();
    let function = owned
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "reader")
        .unwrap();
    assert!(function
        .loan_plan
        .loans
        .iter()
        .any(|loan| loan.cause == LoanCause::TraversalFreeze));
    function
        .loan_plan
        .loans
        .retain(|loan| loan.cause != LoanCause::TraversalFreeze);
    assert_eq!(crate::hir::validate(&owned).unwrap_err().code, "SPX-H006");
}
#[test]
fn explicit_lookup_keeps_exact_short_zero_backing_and_work_refusal() {
    let program = resolved();
    let expression = region(&reader(&program).body);
    let probe = |limit| {
        crate::bounded_output::with_limit_usage(limit, || {
            let payer = crate::bounded_output::cfg_reservation::CfgReservation::acquire()
                .map_err(|_| ())?;
            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            work.allocation = Some(&payer);
            paid::inspect_counted(&program, expression, &mut work)
                .map(|p| p.is_some())
                .map_err(|_| ())
        })
    };
    let (result, refused, used) = probe(usize::MAX);
    assert_eq!(result, Ok(true));
    assert!(!refused && used > 0);
    let (result, refused, exact) = probe(used);
    assert_eq!(result, Ok(true));
    assert!(!refused);
    assert_eq!(exact, used);
    for limit in [used - 1, 0] {
        let (result, refused, _) = probe(limit);
        assert!(result.is_err() && refused);
    }
    let ((), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let payer = crate::bounded_output::cfg_reservation::CfgReservation::acquire().unwrap();
        let mut work = WorkCounter::new(0);
        work.allocation = Some(&payer);
        let error = match paid::inspect_counted(&program, expression, &mut work) {
            Err(error) => error,
            Ok(_) => panic!("zero work was admitted"),
        };
        assert_eq!(error.code, "SPX-H006");
        assert_eq!(
            error.message,
            "loan analysis exceeds 1,000,000 checked work"
        );
    });
    assert!(!refused);
}

#[test]
fn complete_original_previous_indexed_negative_still_runs_the_original_classifier() {
    const ORIGINAL: &str = include_str!("../../../docs/evidence/catalog-record-freeze-composition/original-previous-indexed-direct.spx.txt");
    let ast = crate::check(ORIGINAL, "original-previous-indexed-direct.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    let main = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let expression = region(&main.body);
    let ((), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let payer = crate::bounded_output::cfg_reservation::CfgReservation::acquire().unwrap();
        let mut old = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        old.allocation = Some(&payer);
        let error = match paid_legacy::inspect_counted(&program, expression, &mut old) {
            Err(error) => error,
            Ok(_) => panic!("complete original negative lost its old classifier"),
        };
        assert_eq!(error.code, "SPX-H006");
        assert_eq!(
            error.message,
            "record traversal length, index, nominal reads or whole-root freeze proof is invalid"
        );
    });
    assert!(!refused);
}
