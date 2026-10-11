//! Saved schema oracle and actual provider profile controls; no admission hook.
use super::*;
fn line_end() -> crate::hir::ResolvedProgram {
    let source = include_str!("../../../../../std/io-lines/src/lines.spx");
    let start = source.find("@id(\"std.io.lines.line-end\")").unwrap();
    let rest = &source[start..];
    let end = rest[4..]
        .find("@id(")
        .map_or(rest.len(), |offset| offset + 4);
    let selected = format!("module std.io.lines;\n{}", &rest[..end]);
    crate::hir::resolve(&crate::check(&selected, "actual-line-end.spx").unwrap()).unwrap()
}
#[test]
fn actual_provider_profile_is_current_and_keeps_the_original_value_match_schema_oracle() {
    let program = line_end();
    let function = &program.functions[0];
    let original = selected_schema_with_index(&program, function, None).unwrap();
    assert_eq!(original, function.cleanup_plan.schema);
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    let profile = build(&program, function, &mut budget).unwrap();
    assert!(profile.matches(&program, function));
    assert!(!profile.matches(&program, &function.clone()));
    assert!(!profile.matches(&program.clone(), function));
    assert_eq!(
        profile.inventory(),
        &crate::cleanup::build_inventory(&program, function).unwrap()
    );
    // The witness is nonowning but its schema remains the independent original
    // oracle's result. No nominal-to-v6 rule or fixed-v2 receiving hook is used.
    assert_eq!(
        selected_schema_with_index(&program, function, None).unwrap(),
        original
    );
}
#[test]
fn complete_profile_and_inventory_share_exact_work_backing_and_held_pool_through_drop() {
    let program = line_end();
    let function = &program.functions[0];
    fn measured(
        program: &crate::hir::ResolvedProgram,
        work: usize,
        bytes: usize,
    ) -> (
        Result<(usize, usize), Diagnostic>,
        crate::test_allocation_observer::Metrics,
    ) {
        crate::test_allocation_observer::measure(|| {
            let function = &program.functions[0];
            let mut budget = ReplayBudget::with_physical(function, bytes)?;
            budget.remaining = work;
            budget.skeleton_remaining = 101;
            let profile = build(program, function, &mut budget)?;
            assert!(profile.matches(program, function));
            assert_eq!(budget.skeleton_remaining, 101);
            assert!(!budget.lent_skeleton);
            let receipt = (
                work - budget.remaining,
                budget.physical.as_ref().unwrap().charged(),
            );
            drop(profile);
            drop(budget);
            Ok(receipt)
        })
    }
    let (wide, allocation) = measured(&program, MAX_REPLAY_WORK_UNITS - 101, 1 << 24);
    let (work, bytes) = wide.unwrap();
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.live == 0);
    let (exact, allocation) = measured(&program, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.live == 0);
    assert_eq!(
        measured(&program, work - 1, bytes).0.unwrap_err().code,
        "SPX-H006"
    );
    assert_eq!(
        measured(&program, work, bytes - 1).0.unwrap_err().code,
        "SPX-H006"
    );
    // Pointer-only clones cannot inherit the current function/context witness.
    let clone = function.clone();
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    assert_eq!(
        build(&program, &clone, &mut budget).err().unwrap().code,
        "SPX-H006"
    );
}
#[test]
fn independently_mutated_modes_roots_option_metadata_and_loan_generation_refuse() {
    for mutation in 0..4 {
        let mut program = line_end();
        match mutation {
            0 => program.functions[0].params[0].ownership = OwnershipMode::Own,
            1 => {
                program.functions[0].params[0].id =
                    crate::hir::ValueId::intrinsic_parameter("forged.view", 0)
            }
            2 => {
                program
                    .declarations
                    .type_parameters
                    .get_mut(&crate::hir::DeclarationId::new(crate::prelude::OPTION_ID))
                    .unwrap()[0]
                    .index = 1
            }
            _ => program.functions[0].loan_plan.schema = "forged.loan.generation",
        }
        let function = &program.functions[0];
        let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
        assert_eq!(
            build(&program, function, &mut budget).err().unwrap().code,
            "SPX-H006",
            "profile mutant {mutation}"
        );
    }
}
#[test]
fn original_lexical_and_writer_priorities_are_not_conferred_by_the_borrowed_profile() {
    let source = "module lexical; @id(\"lexical.main\") fn main()->i64 {return 42;}";
    let program =
        crate::hir::resolve(&crate::check(source, "lexical-profile.spx").unwrap()).unwrap();
    let function = &program.functions[0];
    assert_eq!(
        selected_schema_with_index(&program, function, None).unwrap(),
        CLEANUP_PLAN_SCHEMA_V18
    );
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    assert!(build(&program, function, &mut budget).is_err());
    let source = include_str!("../../../../../tests/owned_data/byte_helper_renewal.spx");
    let program =
        crate::hir::resolve(&crate::check(source, "writer-profile.spx").unwrap()).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    assert_eq!(
        selected_schema_with_index(&program, function, None).unwrap(),
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
    );
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    assert!(build(&program, function, &mut budget).is_err());
}

#[test]
fn exact_value_option_profile_does_not_hide_the_original_borrow_match_schema() {
    let mut program = line_end();
    let function = &mut program.functions[0];
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("actual line-end block");
    };
    let body = statements
        .iter_mut()
        .find_map(|statement| match statement {
            ResolvedStatement::While { body, .. } => Some(body),
            _ => None,
        })
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("actual while block");
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("actual terminator binding");
    };
    let ResolvedExprKind::Match { mode, .. } = &mut value.kind else {
        panic!("actual optional byte match");
    };
    *mode = crate::hir::ResolvedMatchMode::Borrow;
    let function = &program.functions[0];
    assert_eq!(
        selected_schema_with_index(&program, function, None).unwrap(),
        CLEANUP_PLAN_SCHEMA_V6
    );
    let mut budget = ReplayBudget::with_physical(function, 1 << 24).unwrap();
    assert!(build(&program, function, &mut budget).is_err());
}
