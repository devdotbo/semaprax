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

// Actual requested Layout observation is deliberately independent of component
// census arithmetic. Input HIR/attached plans were constructed before this
// observation; only newly constructed replacement plans and temporary scratch,
// including their complete receiving Drop, are measured here.
fn observe_current_only(program: &ResolvedProgram) -> Components {
    let (components, physical) = measure(|| {
        let plans = helper_index::build_program_plans(program).unwrap();
        let components = LAST.with(Cell::get);
        drop(plans);
        components
    });
    assert!(!physical.invalid);
    assert!(physical.allocated > 0 && physical.highwater > 0);
    assert_eq!(physical.allocated, physical.freed);
    assert_eq!(physical.live, 0);
    assert!(components.valid);
    components
}

#[test]
fn standalone_builder_and_invocation_overlap_remain_distinct_after_reconstruction() {
    let large = program(LARGE);
    let small = program(SMALL);
    reset_lower_capacity_high_water();
    let first = observe_current_only(&large);
    assert_eq!(standalone_builder_high_water(), first.builder);
    assert_eq!(construction_component_maxima_upper(), Some(first.upper()));
    let repeated = observe_current_only(&large);
    assert_eq!(first, repeated);
    assert_eq!(standalone_builder_high_water(), first.builder);
    assert_eq!(construction_component_maxima_upper(), Some(first.upper()));
    let last = observe_current_only(&small);
    assert_eq!(
        standalone_builder_high_water(),
        first.builder.max(last.builder)
    );
    assert_eq!(
        construction_component_maxima_upper(),
        Some(first.upper().max(last.upper()))
    );
    assert_eq!(
        lower_capacity_high_water(),
        standalone_builder_high_water().max(construction_component_maxima_upper().unwrap())
    );
    reset_lower_capacity_high_water();
    assert_eq!(standalone_builder_high_water(), 0);
    assert_eq!(construction_component_maxima_upper(), Some(0));
}

#[test]
fn staged_component_keeps_earlier_retained_maximum_and_nested_builder_is_standalone() {
    reset_lower_capacity_high_water();
    let mut plans = Vec::with_capacity(2);
    plans.push(CleanupPlan::unresolved());
    {
        let _construction = Construction::begin();
        note_lower_capacity_high_water(11);
        staged(&plans);
        let full = CURRENT.with(Cell::get).unwrap().staged_max;
        let smaller = Vec::new();
        staged(&smaller);
        assert_eq!(CURRENT.with(Cell::get).unwrap().staged, 0);
        assert!(full > 0);
        assert_eq!(CURRENT.with(Cell::get).unwrap().staged_max, full);
        assert_eq!(standalone_builder_high_water(), 11);
        {
            let _nested = Construction::begin();
            note_lower_capacity_high_water(17);
        }
        assert_eq!(CURRENT.with(Cell::get).unwrap().builder, 11);
        assert_eq!(standalone_builder_high_water(), 17);
    }
    assert_eq!(CURRENT.with(Cell::get), None);
    assert!(construction_component_maxima_upper().unwrap() >= 17);
    drop(plans);
}

#[test]
fn phase_overlap_pairs_current_staging_and_full_held_index_without_previous_builder() {
    reset_lower_capacity_high_water();
    {
        let _scope = Construction::begin();
        attached(3);
        held_index(5);
        let mut plans = Vec::with_capacity(2);
        staged(&plans);
        let empty_backing = CURRENT.with(Cell::get).unwrap().staged;
        note_lower_capacity_high_water(1000);
        assert_eq!(
            CURRENT.with(Cell::get).unwrap().phase_overlap_upper,
            1000 + empty_backing + 3
        );
        plans.push(CleanupPlan::unresolved());
        staged(&plans);
        let c = CURRENT.with(Cell::get).unwrap();
        assert_eq!(
            c.phase_overlap_upper,
            (1000 + empty_backing + 3).max(c.staged + 5 + 3)
        );
        assert_eq!(c.upper(), 1000 + c.staged_max + 3);
        plans.clear();
        staged(&plans);
        assert_eq!(CURRENT.with(Cell::get).unwrap().staged, empty_backing);
        assert_eq!(CURRENT.with(Cell::get).unwrap().staged_max, c.staged_max);
    }
    reset_lower_capacity_high_water();
    {
        let _scope = Construction::begin();
        attached(3);
        held_index(5);
    }
    assert_eq!(construction_phase_sampled_census(), Some(8));
    assert_eq!(construction_component_maxima_upper(), Some(3));
    // A held-only phase exceeds the historical builder-based sum. Neither is
    // asserted to equal a complete physical allocator highwater.
    {
        let _scope = Construction::begin();
        attached(usize::MAX);
        held_index(1);
        assert!(!CURRENT.with(Cell::get).unwrap().valid);
        assert_eq!(CURRENT.with(Cell::get).unwrap().phase_overlap_upper, 0);
    }
    assert_eq!(construction_phase_sampled_census(), None);
    assert_eq!(construction_component_maxima_upper(), None);
    // Invalidity survives a subsequent valid invocation until explicit reset.
    {
        let _scope = Construction::begin();
        note_lower_capacity_high_water(1);
    }
    assert_eq!(construction_phase_sampled_census(), None);
}

#[test]
fn full_original_capacity_receiver_and_controls_remain_exact_with_only_index_hook() {
    use sha2::{Digest, Sha256};
    let controls = include_str!("../../../../docs/evidence/capacity-census-811-residual/src__cleanup_plan__build__capacity_observation__tests.rs.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(controls.as_bytes()))
        ),
        "b3ef73fcea5448341a333c2d14569db50ffb09e801dd7b9c61235cd6790bf6bc"
    );
    let helper = include_str!("../../../../docs/evidence/capacity-census-811-residual/src__cleanup_plan__build__helper_index.rs.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(helper.as_bytes()))
        ),
        "c5efac6b49131bbe0e04dfadc9a62e1333ff27df0b6983336168327657e052d1"
    );
    let legacy = include_str!("../../../../docs/evidence/capacity-census-811-residual/src__cleanup_plan__build__capacity_observation__legacy.rs.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(legacy.as_bytes()))
        ),
        "3a8240040c0580868d4a465be868a99192e72c5d2a6cc9cbb614e625f849db13"
    );
    assert!(include_str!("tests.rs").starts_with(controls));
    assert_eq!(include_str!("legacy.rs"), legacy);
    let current = include_str!("../helper_index.rs");
    let hook =
        "    #[cfg(test)]\n    capacity_observation::held_index(index.owned_capacity_bytes());\n";
    assert_eq!(current.matches(hook).count(), 1);
    assert_eq!(current.replacen(hook, "", 1), helper);
}
