use super::*;
use crate::test_allocation_observer::measure;
const SMALL: &str = "module capacity.small; @id(\"main\") fn main()->i64 {0}";
const LARGE: &str = include_str!("../../../../tests/fixtures/native_rust_hir_capacity.spx");
fn program(source: &str) -> ResolvedProgram {
    let parsed = crate::check(source, "capacity-invocation.spx").unwrap();
    crate::hir::resolve(&parsed).unwrap()
}
fn observe(program: &ResolvedProgram) -> Components {
    let ((), physical) = measure(|| {
        let plans = helper_index::build_program_plans(program).unwrap();
        legacy::record_construction(&plans, LAST.with(Cell::get).attached);
        for (function, actual) in program.functions.iter().zip(&plans) {
            let execution = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
            let original =
                lexical_control::build_plan_for_execution(program, function, &execution).unwrap();
            assert_eq!(actual, &original);
        }
        drop(plans);
    });
    assert!(!physical.invalid);
    assert_eq!(physical.live, 0);
    assert_eq!(physical.allocated, physical.freed);
    let components = LAST.with(Cell::get);
    assert!(components.valid);
    assert_eq!(
        components.upper(),
        components.builder + components.staged + components.attached
    );
    // Physical requested Layout highwater is intentionally separate from this
    // component census, which has shared aliases and legacy map allowances.
    assert!(physical.highwater > 0);
    components
}
#[test]
fn repeated_construction_does_not_feed_previous_aggregate_back_into_builder() {
    for source in [SMALL, LARGE] {
        let mut program = program(source);
        for attached in [false, true] {
            if !attached {
                for function in &mut program.functions {
                    function.cleanup_plan = CleanupPlan::unresolved();
                }
                for instance in &mut program.function_instances {
                    instance.function.cleanup_plan = CleanupPlan::unresolved();
                }
            } else {
                helper_index::attach_program_plans(&mut program).unwrap();
            }
            reset_lower_capacity_high_water();
            let first = observe(&program);
            let upper = lower_capacity_high_water();
            let second = observe(&program);
            assert_eq!(first, second);
            assert_eq!(lower_capacity_high_water(), upper);
            assert_eq!(CURRENT.with(Cell::get), None);
            // Preserve the full old feedback control independently; this
            // demonstrates reachability, not a historical 441713 attribution.
            legacy::reset_lower_capacity_high_water();
            legacy::note_lower_capacity_high_water(first.builder);
            legacy::construction_feedback(first.staged, first.attached);
            let old_first = legacy::lower_capacity_high_water();
            legacy::construction_feedback(second.staged, second.attached);
            assert_eq!(
                legacy::lower_capacity_high_water(),
                old_first + second.staged + second.attached
            );
        }
    }
}
#[test]
fn large_then_small_retains_persistent_max_but_reports_local_components() {
    let large = program(LARGE);
    let small = program(SMALL);
    reset_lower_capacity_high_water();
    let large_components = observe(&large);
    let upper = lower_capacity_high_water();
    let small_after_large = observe(&small);
    assert_eq!(
        lower_capacity_high_water(),
        upper.max(small_after_large.upper())
    );
    reset_lower_capacity_high_water();
    let small_alone = observe(&small);
    assert_eq!(small_after_large, small_alone);
    assert!(large_components.builder > small_alone.builder);
}
#[test]
fn scope_restores_on_success_refusal_and_nested_construction_without_prior_input() {
    reset_lower_capacity_high_water();
    note_lower_capacity_high_water(99);
    {
        let _outer = Construction::begin();
        note_lower_capacity_high_water(7);
        attached(3);
        {
            let _inner = Construction::begin();
            note_lower_capacity_high_water(2);
            assert_eq!(CURRENT.with(Cell::get).unwrap().builder, 2);
        }
        assert_eq!(CURRENT.with(Cell::get).unwrap().builder, 7);
    }
    assert_eq!(CURRENT.with(Cell::get), None);
    assert_eq!(lower_capacity_high_water(), 99);
    let program = program(SMALL);
    let before: Vec<_> = program
        .functions
        .iter()
        .map(|f| crate::cache_codec::encode(&f.cleanup_plan).unwrap())
        .collect();
    let (((), physical), refused, _) = crate::bounded_output::with_limit_usage(0, || {
        measure(|| {
            let failure = helper_index::build_program_plans(&program).err().unwrap();
            assert_eq!(failure.code, "SPX-H006");
            drop(failure);
        })
    });
    assert!(refused);
    assert_eq!(CURRENT.with(Cell::get), None);
    assert!(!physical.invalid);
    assert_eq!(physical.live, 0);
    assert_eq!(physical.allocated, physical.freed);
    assert_eq!(
        before,
        program
            .functions
            .iter()
            .map(|f| crate::cache_codec::encode(&f.cleanup_plan).unwrap())
            .collect::<Vec<_>>()
    );
}
#[test]
fn ownership_census_adds_missing_ordinary_conditional_and_string_move_terms() {
    let program = program(SMALL);
    let function = &program.functions[0];
    let mut builder = PlanBuilder::new(&program, function).unwrap();
    // The entire old census remains callable, not an unused archive.
    assert_eq!(
        builder_nested_capacity(&builder),
        legacy::builder_nested_capacity(&builder)
    );
    let initial = builder_nested_capacity(&builder);
    let expression = function.body.id.clone();
    builder.string_owner_moves.insert(expression.clone());
    assert_eq!(
        builder_nested_capacity(&builder) - initial,
        string_owner_moves(&builder)
    );
    let mut state = builder.initial_state.clone();
    let baseline = flow_state_owned_capacity(&state);
    state.conditional_variants.push(ConditionalFlowVariant {
        root: CleanupPlace {
            storage: StorageId::Temporary(expression),
            projections: Vec::new(),
        },
        variant: DeclarationId::new("conditional"),
        cases: vec![(DeclarationId::new("case"), vec![LivenessFlagId(0)])],
    });
    assert_eq!(
        flow_state_owned_capacity(&state) - baseline,
        conditional_backing(&state)
    );
}

#[test]
fn helper_empty_builder_counts_held_ordinary_directory_and_receiving_drop() {
    let program = program("module capacity.ordinary; @id(\"main\") fn main()->i64 { let mut values=vec_with_capacity<i64>(2usize); let mut i=0; while i<2 { values=vec_push<i64>(values,i); i=i+1; 0 } 0 }");
    let ((), physical) = measure(|| {
        let index = crate::loop_calls::byte_writer::ModuleIndex::new(&program).unwrap();
        let builder = PlanBuilder::with_index(&program, &program.functions[0], &index).unwrap();
        assert!(
            matches!(&builder.byte_helpers, helper_index::HelperSites::Owned(sites) if sites.is_empty())
        );
        assert!(index.owned_capacity_bytes() > 0);
        assert_eq!(
            builder_nested_capacity(&builder) - legacy::builder_nested_capacity(&builder),
            index.owned_capacity_bytes()
        );
        drop(builder);
        drop(index);
    });
    assert!(!physical.invalid && physical.allocated > 0);
    assert_eq!(physical.allocated, physical.freed);
    assert_eq!(physical.live, 0);
}

#[test]
fn checked_owned_variant_builder_attributes_real_conditional_state_through_drop() {
    let program = program(include_str!(
        "../../../../tests/owned_byte_variant_v1_fixture.spx"
    ));
    let mut saw_conditional = false;
    let ((), physical) = measure(|| {
        for function in &program.functions {
            let builder = PlanBuilder::new(&program, function).unwrap();
            let conditional = conditional_backing(&builder.initial_state);
            saw_conditional |= conditional > 0;
            assert_eq!(
                builder_nested_capacity(&builder) - legacy::builder_nested_capacity(&builder),
                conditional + string_owner_moves(&builder)
            );
            drop(builder);
        }
    });
    assert!(saw_conditional);
    assert!(!physical.invalid && physical.allocated > 0);
    assert_eq!(physical.live, 0);
    assert_eq!(physical.allocated, physical.freed);
}
