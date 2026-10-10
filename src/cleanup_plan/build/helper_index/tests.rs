use super::*;
const SOURCE: &str = include_str!("../../../../tests/owned_data/byte_helper_renewal.spx");
fn program(source: &str) -> ResolvedProgram {
    let ast = crate::check(source, "construction-index.spx").unwrap();
    crate::hir::resolve(&ast).unwrap()
}
#[test]
fn construction_index_reuses_exact_sites_and_preserves_independent_plan_bytes() {
    let program = program(SOURCE);
    let index = ModuleIndex::new(&program).unwrap();
    let actual = build_program_plans(&program).unwrap();
    for (function, plan) in program.functions.iter().zip(&actual) {
        let execution = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
        let direct =
            lexical_control::build_plan_for_execution(&program, function, &execution).unwrap();
        assert_eq!(plan, &direct);
        assert_eq!(
            crate::cache_codec::encode(plan).unwrap(),
            crate::cache_codec::encode(&direct).unwrap()
        );
        let builder = PlanBuilder::with_index(&program, function, &index).unwrap();
        let independently =
            crate::loop_calls::byte_writer::checked_bindings(&program, function).unwrap();
        for (at, binding) in independently {
            let (found, overflowed, used) =
                crate::bounded_output::with_limit_usage(0, || builder.renewal_binding(at));
            assert_eq!(found, Some(binding));
            assert!(!overflowed);
            assert_eq!(used, 0);
        }
        // Construction keeps this program's own backing, not a cloned function.
        assert!(matches!(builder.byte_helpers, HelperSites::Indexed { .. }));
    }
    crate::hir::validate(&program).unwrap();
    crate::cleanup_plan::validate_program(&program).unwrap();
}
#[test]
fn construction_exact_one_short_and_zero_budget_leave_attached_plans_unchanged() {
    let mut program = program(SOURCE);
    let original = program
        .functions
        .iter()
        .map(|f| crate::cache_codec::encode(&f.cleanup_plan).unwrap())
        .collect::<Vec<_>>();
    let (plans, overflowed, required) =
        crate::bounded_output::with_limit_usage(usize::MAX, || build_program_plans(&program));
    assert!(plans.is_ok());
    assert!(!overflowed);
    assert!(required > 0);
    let (plans, overflowed, used) =
        crate::bounded_output::with_limit_usage(required, || build_program_plans(&program));
    assert!(plans.is_ok());
    assert!(!overflowed);
    assert_eq!(used, required);
    let (failure, overflowed, _) = crate::bounded_output::with_limit_usage(required - 1, || {
        attach_program_plans(&mut program)
    });
    assert_eq!(failure.unwrap_err().code, "SPX-H006");
    assert!(overflowed);
    assert_eq!(
        original,
        program
            .functions
            .iter()
            .map(|f| crate::cache_codec::encode(&f.cleanup_plan).unwrap())
            .collect::<Vec<_>>()
    );
    let (failure, overflowed, _) =
        crate::bounded_output::with_limit_usage(0, || attach_program_plans(&mut program));
    assert_eq!(failure.unwrap_err().code, "SPX-H006");
    assert!(overflowed);
}
#[test]
fn construction_rejects_copied_function_foreign_program_and_reproves_changed_provider() {
    let program = program(SOURCE);
    let index = ModuleIndex::new(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let copy = function.clone();
    let error = PlanBuilder::with_index(&program, &copy, &index)
        .err()
        .unwrap();
    assert_eq!(error.code, "SPX-H006");
    let foreign = program.clone();
    let error = PlanBuilder::with_index(&foreign, function, &index)
        .err()
        .unwrap();
    assert_eq!(error.code, "SPX-H006");
    drop(index);
    let mut changed = program.clone();
    // Exact helper call remains in its body but the target inventory is missing.
    changed.functions.retain(|f| f.id.as_str() != "helper.leaf");
    let rebuilt = ModuleIndex::new(&changed).unwrap();
    let main = changed
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    assert!(
        !rebuilt.has_sites(main),
        "missing provider cannot retain helper authority"
    );
    assert!(
        crate::hir::validate(&changed).is_err(),
        "missing call target must fail independent validation"
    );
}
#[test]
fn irrelevant_and_allocating_legacy_helpers_keep_old_schemas_and_canonical_order() {
    for source in ["module plain;@id(\"main\") fn main()->i64{0}",
        "module old;@id(\"old\") fn old(input:own Bytes)->Bytes{let size=byte_len(bytes_as_slice(input));bytes_zeroed(1usize)} @id(\"main\") fn main()->i64{let mut output=bytes_zeroed(1usize);output=old(output);0}"] {
        let program=program(source);
        let index=ModuleIndex::new(&program).unwrap();
        if source.starts_with("module plain") {assert_eq!(index.owned_capacity_bytes(),0);}
        let plans=build_program_plans(&program).unwrap();
        for (function,plan) in program.functions.iter().zip(&plans) {
            assert_ne!(plan.schema,super::super::super::CLEANUP_PLAN_SCHEMA_V19);
            assert_eq!(plan,&function.cleanup_plan);
        }
    }
}
