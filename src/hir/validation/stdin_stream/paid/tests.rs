use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
const LIMIT: usize = 64 * 1024 * 1024;
fn streaming() -> ResolvedProgram {
    // Reuse the actual original source corpus without a second altered fixture.
    let source = include_str!("../../../../stdin_stream_ops/tests.rs")
        .split_once("const SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    crate::hir::resolve(&crate::check(source, "stdin-stream.spx").unwrap()).unwrap()
}
#[test]
fn paid_type_and_expression_predicates_keep_complete_original_truth_and_sticky_refusal() {
    let program = streaming();
    for function in &program.functions {
        assert_eq!(
            type_uses(&function.return_type).unwrap(),
            stream::resolved_type_uses(&function.return_type)
        );
        assert_eq!(
            expression_uses(&function.body).unwrap(),
            stream::resolved_expression_uses(&function.body)
        );
        assert_eq!(
            function_uses(function).unwrap(),
            stream::resolved_function_uses(function)
        );
    }
    let ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("wide.fixture"),
        arguments: vec![ResolvedType::I64; 65],
    };
    let expected = stream::resolved_type_uses(&ty);
    let run = || type_uses(&ty);
    let ((result, actual), refused, paid, first) = with_limit_usage_refusal(LIMIT, || measure(run));
    assert_eq!(result.unwrap(), expected);
    assert!(
        !refused
            && first.is_none()
            && !actual.invalid
            && actual.live == 0
            && actual.allocated == actual.freed
    );
    assert!(actual.allocated <= paid && actual.highwater <= paid && paid > 0);
    let ((result, actual), refused, used, first) = with_limit_usage_refusal(paid, || measure(run));
    assert_eq!(result.unwrap(), expected);
    assert!(!refused && used == paid && first.is_none() && actual.live == 0);
    for limit in [0, paid - 1] {
        let ((result, actual), refused, used, first) = with_limit_usage_refusal(limit, || {
            measure(|| {
                let first = run();
                assert!(first.is_err());
                assert!(type_uses(&ResolvedType::I64).is_err());
                first
            })
        });
        assert!(result.is_err() && refused && first.is_some() && used <= limit);
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    }
}
#[test]
fn full_paid_streaming_derivation_matches_original_facts_through_drop_exact_short_and_zero() {
    let program = streaming();
    let old = stream::analysis::derive(&program).unwrap();
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = analysis::derive(&program);
                if let Ok(current) = &result {
                    assert_eq!(current, &old);
                }
                let ok = result.is_ok();
                drop(result);
                ok
            })
        })
    };
    let ((ok, actual), refused, paid, first) = observe(LIMIT);
    assert!(ok && !refused && first.is_none() && !actual.invalid && actual.live == 0);
    assert!(
        actual.allocated <= paid && actual.highwater <= paid && actual.allocated == actual.freed
    );
    let ((ok, actual), refused, used, first) = observe(paid);
    assert!(ok && !refused && used == paid && first.is_none() && actual.live == 0);
    for limit in [0, paid - 1] {
        let ((ok, actual), refused, used, first) = observe(limit);
        assert!(!ok && refused && first.is_some() && used <= limit);
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    }
}
#[test]
fn full_sealed_preflight_keeps_original_selected_diagnostic_and_all_fields() {
    let source = "module test.paid.preflight; @id(\"row\") record Row { @id(\"row.field\") field:i64, } @id(\"main\") fn main()->i64 { 0 }";
    let base = crate::hir::resolve(&crate::check(source, "paid-preflight.spx").unwrap()).unwrap();
    for forged in [false, true] {
        let mut program = base.clone();
        if forged {
            let record = program
                .types
                .iter_mut()
                .find(|t| t.id.as_str() == "row")
                .unwrap();
            let ResolvedTypeDeclarationKind::Record { fields } = &mut record.kind else {
                panic!("record");
            };
            fields[0].ty = ResolvedType::Nominal {
                declaration: DeclarationId::new(stream::READER_ID),
                arguments: Vec::new(),
            };
        }
        let old = super::super::reject_sealed_escape(&program);
        let ((current, actual), refused, _, first) =
            with_limit_usage_refusal(LIMIT, || measure(|| reject_sealed_escape_paid(&program)));
        assert_eq!(current, old);
        assert!(
            !refused
                && first.is_none()
                && !actual.invalid
                && actual.live == 0
                && actual.allocated == actual.freed
        );
    }
}
