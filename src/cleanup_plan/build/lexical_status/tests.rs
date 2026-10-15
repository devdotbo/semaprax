//! Authored order remains authoritative when every normal loop latch is closed.
use super::*;
const SOURCE: &str = r#"module lexical.status.order;
@id("app.main") fn main()->i64 ensures result==41 {
 let values=vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize),1),2);
 let mut seen=0;
 for value in values {
  if value==1 {continue;} else {0}
  let marker=2+3;
  seen=marker;
  break;
 }
 return 41;
}"#;
fn program() -> ResolvedProgram {
    let checked = crate::check(SOURCE, "lexical-status-order.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    crate::hir::validate(&program).unwrap();
    program
}
#[test]
fn authored_status_inventory_precedes_cfg_and_has_exact_physical_boundary() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let (result, overflowed, charged) =
        crate::bounded_output::with_limit_usage(usize::MAX, || declare(&program, function));
    assert!(!overflowed);
    let (sources, seen, remaining) = result.unwrap();
    assert_eq!(sources, function.cleanup_plan.status_sources);
    assert_eq!(seen, vec![0; sources.len()]);
    assert!(remaining < WORK);
    assert!(
        sources
            .iter()
            .filter(|s| matches!(
                s.producer,
                StatusProducer::CheckedArithmetic {
                    operation: CheckedOperation::Add,
                    ..
                }
            ))
            .count()
            >= 2,
        "authored local Add and generated final latch Add both remain declared"
    );
    let (exact, overflowed, used) =
        crate::bounded_output::with_limit_usage(charged, || declare(&program, function));
    assert!(!overflowed);
    assert_eq!(used, charged);
    assert_eq!(exact.unwrap().0, sources);
    let (short, overflowed, _) =
        crate::bounded_output::with_limit_usage(charged - 1, || declare(&program, function));
    assert!(overflowed);
    assert!(short.is_err());
}
#[test]
fn reordered_missing_duplicate_and_changed_status_producers_refuse_independent_replay() {
    let original = program();
    for mutation in 0..5 {
        let mut changed = original.clone();
        let function = changed
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let sources = &mut function.cleanup_plan.status_sources;
        assert!(sources.len() > 2);
        match mutation {
            0 => sources.swap(0, 1),
            1 => {
                sources.remove(0);
            }
            2 => sources.push(sources[0].clone()),
            3 => {
                sources[0].producer = StatusProducer::PropagatedCall {
                    callee: DeclarationId::new("forged.status"),
                }
            }
            _ => {
                let source = sources
                    .iter_mut()
                    .find(|s| matches!(s.producer, StatusProducer::CheckedArithmetic { .. }))
                    .unwrap();
                let StatusProducer::CheckedArithmetic {
                    normalized_cases, ..
                } = &mut source.producer
                else {
                    unreachable!()
                };
                normalized_cases.clear();
            }
        }
        assert!(
            crate::hir::validate(&changed).is_err(),
            "mutation {mutation}"
        );
        assert!(
            crate::cleanup_plan::validate_program(&changed).is_err(),
            "mutation {mutation}"
        );
    }
}
