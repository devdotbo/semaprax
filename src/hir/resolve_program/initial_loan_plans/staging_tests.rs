//! Paid Vec oracle versus caller-held staging, including actual instances.
use super::*;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::{measure, Metrics};

fn resolve(source: &str) -> ResolvedProgram {
    let ast = crate::check(source, "initial-staging.spx").unwrap();
    let mut program = crate::hir::resolve(&ast).unwrap();
    clear(&mut program);
    program
}
fn clear(program: &mut ResolvedProgram) {
    for function in &mut program.functions {
        drop(std::mem::replace(&mut function.loan_plan, LoanPlan::unresolved()));
    }
    for instance in &mut program.function_instances {
        drop(std::mem::replace(&mut instance.function.loan_plan, LoanPlan::unresolved()));
    }
}
fn fixture(group: Group, count: usize) -> ResolvedProgram {
    let mut source = String::from("module initial.staging;\n");
    match group {
        Group::Authored => {
            source.push_str("@id(\"packet\") record Packet {@id(\"payload\") payload:Bytes,}\n");
            for index in 0..count {
                source.push_str(&format!(
                    "@id(\"f{index}\") fn f{index}(packet:own Packet)->usize {{let view=bytes_as_slice(packet.payload);byte_len(view)+byte_len(view)}}\n"
                ));
            }
        }
        Group::Instances => {
            // Distinct real template calls create distinct concrete instances;
            // no cloning or manually attaching plans to pretend instantiation.
            for index in 0..count {
                source.push_str(&format!(
                    "@id(\"g{index}\") fn g{index}<T>(value:T)->T {{value}}\n"
                ));
            }
            source.push_str("@id(\"main\") fn main()->i64 {\n");
            for index in 0..count {
                source.push_str(&format!("let value{index}=g{index}<i64>({index});\n"));
            }
            source.push_str("0}\n");
        }
    }
    if matches!(group, Group::Authored) && count == 0 {
        // Private empty-group control, derived from admitted source. This does
        // not admit a source module without a function or change syntax rules.
        source.push_str("@id(\"empty.seed\") fn empty_seed()->i64 {0}\n");
    }
    let mut program = resolve(&source);
    if matches!(group, Group::Authored) && count == 0 {
        program.functions.clear();
    }
    assert_eq!(group.len(&program), count);
    program
}
pub(super) fn wide_original_fixture() -> ResolvedProgram {
    let mut source = super::tests::SOURCE.to_owned();
    // Preserve the original first body and loan semantics. Only the outer
    // authored directory grows; the shared small fixture remains unchanged.
    for index in 4..33 {
        source.push_str(&format!("@id(\"wide{index}\") fn wide{index}()->i64 {{0}}\n"));
    }
    let program = resolve(&source);
    assert_eq!(program.functions.len(), 33);
    program
}

// Saved CURRENT paid Vec constructor is independent of Staging. It remains
// collect(Some(owner)), with its original first-plan-before-array order.
fn paid_vec_attach(program: &mut ResolvedProgram) -> Result<(), Diagnostic> {
    let owner = CfgReservation::acquire().map_err(|_| refused())?;
    let result = (|| {
        let plans = collect(program, Group::Authored, Some(&owner))?;
        for (function, plan) in program.functions.iter_mut().zip(plans) {
            function.loan_plan = plan;
        }
        let plans = collect(program, Group::Instances, Some(&owner))?;
        for (instance, plan) in program.function_instances.iter_mut().zip(plans) {
            instance.function.loan_plan = plan;
        }
        Ok(())
    })();
    if owner.first().is_some() { return Err(owner.diagnostic()); }
    result
}
fn first_plan(program: &ResolvedProgram, group: Group, owner: &CfgReservation) -> LoanPlan {
    match crate::loan_plan::build_plan_with_reservation(program, group.function(program, 0), owner) {
        Ok(plan) => plan,
        Err(_) => panic!("first ordinary member must plan"),
    }
}
fn removed_directory_bytes(program: &ResolvedProgram) -> usize {
    [Group::Authored, Group::Instances].into_iter()
        .map(|group| group.len(program))
        .filter(|&count| count <= staging::INLINE_PLANS)
        .map(|count| count * std::mem::size_of::<LoanPlan>()).sum()
}
fn observed(program: &ResolvedProgram, old: bool, limit: usize) -> (ResolvedProgram, bool, usize, Metrics) {
    let mut program = program.clone();
    let ((result, metrics), overflow, used) = with_limit_usage(limit, || measure(|| {
        let result = if old { paid_vec_attach(&mut program) } else { attach(&mut program) };
        // The full result is checked separately; all new nested backing and
        // both directories, including the refusal diagnostic, reach Drop here.
        clear(&mut program);
        result
    }));
    result.unwrap();
    (program, overflow, used, metrics)
}
fn balanced(metrics: Metrics) {
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.freed, metrics.allocated);
}
#[test]
fn inline_staging_matches_paid_vec_full_plans_at_both_group_boundaries() {
    for group in [Group::Authored, Group::Instances] {
        for count in [0, 1, 31, 32, 33] {
            let original = fixture(group, count);
            let mut expected = original.clone();
            let (result, overflow, _) = with_limit_usage(usize::MAX, || paid_vec_attach(&mut expected));
            result.unwrap();
            assert!(!overflow);
            let mut actual = original;
            let (result, overflow, _) = with_limit_usage(usize::MAX, || attach(&mut actual));
            result.unwrap();
            assert!(!overflow);
            assert_eq!(actual, expected, "full authored and instantiated plans at count {count}");
        }
    }
}
#[test]
fn inline_staging_removes_exact_outer_directory_bytes_through_all_drop() {
    for group in [Group::Authored, Group::Instances] {
        for count in [0, 1, 31, 32, 33] {
            let original = fixture(group, count);
            let removed = removed_directory_bytes(&original);
            let (_, old_overflow, old_used, old) = observed(&original, true, usize::MAX);
            let (_, new_overflow, new_used, new) = observed(&original, false, usize::MAX);
            assert!(!old_overflow && !new_overflow);
            balanced(old);
            balanced(new);
            assert_eq!(old_used - new_used, removed);
            assert_eq!(old.allocated - new.allocated, removed);
            assert_eq!(old.freed - new.freed, removed);
            let removed_requests = [Group::Authored, Group::Instances].into_iter()
                .map(|group| group.len(&original))
                .filter(|&count| count > 0 && count <= staging::INLINE_PLANS).count();
            assert_eq!(old.allocations - new.allocations, removed_requests);
            assert_eq!(old.reallocations, new.reallocations);
            assert!(new.allocated <= new_used);
            let (_, overflow, exact_used, exact) = observed(&original, false, new_used);
            assert!(!overflow);
            assert_eq!(exact_used, new_used);
            balanced(exact);
        }
    }
}
#[test]
fn wide_first_array_one_short_is_sticky_and_never_publishes_group() {
    for group in [Group::Authored, Group::Instances] {
        let mut program = fixture(group, 33);
        let before = program.clone();
        let ((), overflow, boundary) = with_limit_usage(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let plan = first_plan(&program, group, &owner);
            owner.array::<LoanPlan>(33).unwrap();
            drop(plan);
        });
        assert!(!overflow);
        let ((error, metrics), overflow, used, _) = with_limit_usage_refusal(boundary - 1, || measure(|| {
            let owner = CfgReservation::acquire().unwrap();
            let mut plans = Staging::new();
            assert_eq!(collect_paid(&program, group, &owner, &mut plans).unwrap_err().code, "SPX-H006");
            let first = owner.first().expect("array reservation selected a refusal");
            let remaining = crate::bounded_output::active_remaining();
            assert_eq!(collect_paid(&program, group, &owner, &mut plans).unwrap_err().code, "SPX-H006");
            assert_eq!(owner.first(), Some(first));
            assert_eq!(remaining, crate::bounded_output::active_remaining());
            assert!(plans.empty_unallocated());
            drop(plans);
            let error = owner.diagnostic();
            assert_eq!(error.message, "canonical loan CFG storage reservation refused");
            let code = error.code;
            drop(error);
            code
        }));
        assert_eq!(error, "SPX-H006");
        assert!(overflow);
        assert_eq!(used, boundary - 33 * std::mem::size_of::<LoanPlan>());
        balanced(metrics);
        assert_eq!(program, before);
        let mut expected = before.clone();
        let mut prefix = before.clone();
        let ((), overflow, full_boundary) = with_limit_usage(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            if matches!(group, Group::Instances) {
                let mut plans = Staging::new();
                collect_paid(&prefix, Group::Authored, &owner, &mut plans).unwrap();
                plans.publish(&mut prefix, Group::Authored);
            }
            let plan = first_plan(&prefix, group, &owner);
            owner.array::<LoanPlan>(33).unwrap();
            drop(plan);
        });
        assert!(!overflow);
        if matches!(group, Group::Instances) {
            expected.functions = prefix.functions.clone();
        }
        let (((), metrics), overflow, used) = with_limit_usage(full_boundary - 1, || measure(|| {
            let error = attach(&mut program).unwrap_err();
            assert_eq!(error.code, "SPX-H006");
            assert_eq!(error.message, "canonical loan CFG storage reservation refused");
            drop(error);
            assert_eq!(program, expected);
            let remaining = crate::bounded_output::active_remaining();
            drop(attach(&mut program).unwrap_err());
            assert_eq!(remaining, crate::bounded_output::active_remaining());
            assert_eq!(program, expected);
            clear(&mut program);
        }));
        assert!(overflow);
        assert_eq!(used, full_boundary - 33 * std::mem::size_of::<LoanPlan>());
        balanced(metrics);
    }
}
