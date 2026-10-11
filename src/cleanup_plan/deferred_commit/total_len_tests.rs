//! Total byte-length producer facts, independent of backend receiving.
use super::*;
use crate::cleanup_plan::{
    CleanupTransition, StatusLane, StatusProducer, StatusSource, StatusSourceId,
};
use crate::hir::{ResolvedExprKind, ResolvedFunction};

fn length_calls(function: &ResolvedFunction) -> Vec<crate::hir::ExpressionId> {
    let mut ids = Vec::new();
    crate::hir::function_value::walk(function, |expression| {
        if matches!(&expression.kind, ResolvedExprKind::Call { callee, instance: None, .. }
            if callee.as_str() == crate::str_ops::LEN_BYTES_ID)
        {
            assert!(expression_is_infallible_compiler_operation(expression));
            ids.push(expression.id.clone());
        }
    });
    assert!(!ids.is_empty());
    ids
}
fn total_plan(function: &ResolvedFunction, ids: &[crate::hir::ExpressionId]) {
    for id in ids {
        assert!(!function
            .cleanup_plan
            .status_sources
            .iter()
            .any(|source| source.id.expression == *id
                && source.id.lane == StatusLane::OperationFailure));
        assert_eq!(function.cleanup_plan.blocks.iter().flat_map(|block| &block.transitions)
            .filter(|transition| matches!(transition, CleanupTransition::CallCommit { call, arguments }
                if call == id && arguments.is_empty())).count(), 1);
        assert!(!function.cleanup_plan.exits.iter().any(|exit|
            matches!(&exit.continuation, crate::cleanup_plan::ExitContinuation::ReturnFailure { source }
                if source.expression == *id && source.lane == StatusLane::OperationFailure)));
    }
}

#[test]
fn exact_total_length_keeps_commit_and_other_failures_and_rejects_forged_status() {
    for source in [
        "module total.length; @id(\"app.main\") fn main()->i64 {let text=\"é\\0\";let view=string_as_str(text);str_len_bytes(view)+1}",
        "module total.length; @id(\"app.main\") fn main()->i64 {let text=\"é\\0\";let view=string_as_str(text);if true {return str_len_bytes(view)+1;}else{0} 0}",
    ] {
        let ast = crate::check(source, "total-length.spx").unwrap();
        let program = crate::hir::resolve(&ast).unwrap();
        crate::hir::validate(&program).unwrap();
        super::super::replay::validate_program(&program).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.main")
            .unwrap();
        let ids = length_calls(function);
        total_plan(function, &ids);
        assert!(function
            .cleanup_plan
            .status_sources
            .iter()
            .any(|source| matches!(source.producer, StatusProducer::CheckedArithmetic { .. })));
        assert!(
            !function.cleanup_plan.slots.is_empty(),
            "the String owner remains a real cleanup root"
        );
        let mut forged = program.clone();
        let function = forged
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "app.main")
            .unwrap();
        function.cleanup_plan.status_sources.push(StatusSource {
            id: StatusSourceId {
                expression: ids[0].clone(),
                lane: StatusLane::OperationFailure,
            },
            producer: StatusProducer::PropagatedCall {
                callee: DeclarationId::new(crate::str_ops::LEN_BYTES_ID),
            },
        });
        assert_eq!(super::super::replay::validate_program(&forged).unwrap_err().code, "SPX-H006");
        assert_eq!(
            crate::cleanup_plan::validate_program(&forged)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
    for callee in [
        crate::str_ops::IS_EMPTY_ID,
        crate::str_ops::BYTE_AT_ID,
        crate::string_ops::LEN_ID,
        "core.str.len_bytes.forged",
    ] {
        assert!(!is_total_str_len_bytes(&DeclarationId::new(callee)));
    }
}

#[test]
fn unchanged_original_selected_33_and_4096_plans_have_no_impossible_length_failure() {
    const ORIGINAL: &str = include_str!("../../../tests/fixtures/record-traversal-maximum.spx");
    for source in [ORIGINAL.replace("4096", "33"), ORIGINAL.to_owned()] {
        let ast = crate::parse(&source, "original-length-plan.spx").unwrap();
        let owner = crate::hir::selected_fused_candidate::validate_selected(&ast).unwrap();
        let function = owner
            .program()
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.main")
            .unwrap();
        total_plan(function, &length_calls(function));
        assert!(function.cleanup_plan.status_sources.iter().any(
            |source| matches!(&source.producer, StatusProducer::PropagatedCall { callee }
                if callee.as_str()==crate::vec_ops::PUSH_ID)
        ));
    }
}
