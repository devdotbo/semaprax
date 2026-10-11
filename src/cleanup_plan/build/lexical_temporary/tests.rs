use super::*;
const SOURCE: &str = r#"module lexical.completed_string;
@id("inspect") fn inspect(fail:bool)->i64 ensures !fail {
 let body="body";let mut index=0;
 while index<1 {index=index+1;if index==1 {return string_len(body)+25;} else {0} 0}
 29
}
@id("app.main") fn main()->i64 {0}
"#;
fn fixture(source: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::check(source, "completed-string.spx").unwrap()).unwrap()
}
fn inspect(program: &ResolvedProgram) -> &ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "inspect")
        .unwrap()
}
#[test]
fn completed_return_operand_clone_settles_before_ensures_and_keeps_root_owner() {
    let program = fixture(SOURCE);
    crate::hir::validate(&program).unwrap();
    let function = inspect(&program);
    let exit = function
        .cleanup_plan
        .exits
        .iter()
        .find(|exit| matches!(exit.continuation, ExitContinuation::FunctionReturn { .. }))
        .unwrap();
    let temporary = exit
        .finalize_in_order
        .iter()
        .find(|action| matches!(action.source.storage, StorageId::Temporary(_)))
        .unwrap();
    let StorageId::Temporary(id) = &temporary.source.storage else {
        unreachable!()
    };
    let expression = lexical_control::expression_at(&function.body, id).unwrap();
    assert_eq!(expression.ty, ResolvedType::String);
    assert!(matches!(expression.kind, ResolvedExprKind::Place(_)));
    assert!(!exit
        .finalize_in_order
        .iter()
        .any(|action| matches!(action.source.storage, StorageId::Value(_))));
    let canonical =
        crate::format::canonical(&crate::check(SOURCE, "completed-string.spx").unwrap());
    let repeated = fixture(&canonical);
    crate::hir::validate(&repeated).unwrap();
    assert_eq!(function.cleanup_plan, inspect(&repeated).cleanup_plan);
    let mut forged = function.clone();
    let selected = forged
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| matches!(exit.continuation, ExitContinuation::FunctionReturn { .. }))
        .unwrap();
    selected.finalize_in_order.clear();
    let mut forged_program = program.clone();
    let destination = forged_program
        .functions
        .iter_mut()
        .find(|function| function.id == forged.id)
        .unwrap();
    *destination = forged;
    assert_eq!(
        crate::cleanup_plan::validate_program(&forged_program)
            .unwrap_err()
            .code,
        "SPX-H006"
    );
}
#[test]
fn temporary_kind_and_live_staged_or_provisional_storage_cannot_mint_completion() {
    let program = fixture(SOURCE);
    let function = inspect(&program);
    let builder = PlanBuilder::new(&program, function).unwrap();
    let temp = function
        .cleanup_plan
        .exits
        .iter()
        .flat_map(|exit| &exit.finalize_in_order)
        .find_map(|action| match &action.source.storage {
            StorageId::Temporary(id) => Some(id),
            _ => None,
        })
        .unwrap();
    assert!(builder
        .completed_lexical_storage(&StorageId::Temporary(temp.clone()))
        .unwrap());
    assert!(!builder
        .completed_lexical_storage(&StorageId::ProvisionalResult)
        .unwrap());
    assert!(!builder
        .completed_lexical_storage(&StorageId::CallArgument {
            call: temp.clone(),
            parameter_index: 0,
            value_expression: temp.clone()
        })
        .unwrap());
    let mut changed = function.clone();
    let ResolvedExprKind::Block { statements, .. } = &mut changed.body.kind else {
        panic!("body")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("body owner")
    };
    let fake_id = value.id.clone();
    value.kind = ResolvedExprKind::Int(9);
    let altered = PlanBuilder::new(&program, &changed).unwrap();
    assert!(!altered
        .completed_lexical_storage(&StorageId::Temporary(fake_id))
        .unwrap());
}
#[test]
fn owning_iterator_continue_settles_completed_string_read_without_consuming_remainder() {
    let source = r#"module lexical.completed_iterator;
@id("iterate") fn iterate(values:own Vec<Bytes>)->i64 {
 let body="body";let mut total=0;
 for own item in vec_into_iter<Bytes>(values) {
  let _ = string_len(body);
  if byte_len(bytes_as_slice(item))==0usize {continue;} else {0}
  total=total+1;0
 } total
}
@id("app.main") fn main()->i64 {0}
"#;
    let program = fixture(source);
    crate::hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "iterate")
        .unwrap();
    let control = function.cleanup_plan.lexical_control.as_ref().unwrap();
    let certificate = control
        .exits
        .iter()
        .find(|exit| exit.kind == crate::ast::LexicalExitKind::Continue)
        .unwrap();
    let selected = &function.cleanup_plan.exits[certificate.exit.0 as usize];
    assert!(selected
        .finalize_in_order
        .iter()
        .any(|action| matches!(action.source.storage, StorageId::Temporary(_))));
    let crate::hir::LoopLatchKind::OwnedIteratorStep { remainder, .. } = &control.loops[0].latch
    else {
        panic!("owning latch")
    };
    assert!(!selected
        .finalize_in_order
        .iter()
        .any(|action| action.source.storage == StorageId::Value(remainder.clone())));
}

#[test]
fn terminal_owned_record_arm_has_no_second_normal_scope_terminator() {
    let source = r#"module lexical.terminal_record;
@id("Token") record Token { @id("Token.text") text:string, }
@id("inspect") fn inspect(token:own Token,flag:bool)->i64 ensures result==7 {
 match own token { Token {text} => {
  let mut index=0;
  while index<1 {
   index=index+1;
   if flag {return string_len(text)+3;} else {0}
   0
  }
  return 7;
 }, }
}
@id("app.main") fn main()->i64 {0}
"#;
    let program = fixture(source);
    crate::hir::validate(&program).unwrap();
    let function = inspect(&program);
    assert!(function.cleanup_plan.lexical_control.is_some());
    let returns: Vec<_> = function
        .cleanup_plan
        .exits
        .iter()
        .filter(|exit| matches!(exit.continuation, ExitContinuation::FunctionReturn { .. }))
        .collect();
    assert_eq!(returns.len(), 2);
    assert!(returns.iter().any(|exit| {
        exit.finalize_in_order
            .iter()
            .any(|action| matches!(action.source.storage, StorageId::Value(_)))
    }));
    let canonical = crate::format::canonical(&crate::check(source, "terminal-record.spx").unwrap());
    let repeated = fixture(&canonical);
    crate::hir::validate(&repeated).unwrap();
    assert_eq!(function.cleanup_plan, inspect(&repeated).cleanup_plan);
    for forged_kind in [false, true] {
        let mut malformed = program.clone();
        let function = malformed
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "inspect")
            .unwrap();
        let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("fixture must retain its function block");
        };
        let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
            panic!("fixture must retain its terminal record match");
        };
        assert_eq!(arms[0].value.ty, ResolvedType::Never);
        if forged_kind {
            arms[0].value.kind = ResolvedExprKind::Int(7);
        } else {
            arms[0].value.ownership = OwnershipMode::Own;
        }
        assert_eq!(
            crate::hir::validate(&malformed).unwrap_err().code,
            "SPX-H006"
        );
    }
    let mut forged = program.clone();
    let function = forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "inspect")
        .unwrap();
    let exit = function
        .cleanup_plan
        .exits
        .iter_mut()
        .find(|exit| {
            matches!(exit.continuation, ExitContinuation::FunctionReturn { .. })
                && !exit.finalize_in_order.is_empty()
        })
        .unwrap();
    exit.finalize_in_order.pop();
    assert_eq!(
        crate::cleanup_plan::validate_program(&forged)
            .unwrap_err()
            .code,
        "SPX-H006"
    );
}
