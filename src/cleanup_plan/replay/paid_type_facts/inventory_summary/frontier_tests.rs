use super::*;
use crate::test_allocation_observer::{measure, Metrics};

type Outcome = (Summary, Option<(crate::cleanup::CleanupShapeProfile, bool)>);
fn classify(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    roots: &[&ResolvedType],
    saved: bool,
    budget: &mut ReplayBudget,
) -> Result<Outcome, Diagnostic> {
    budget.charge(function, 0, "shape frontier receiver setup")?;
    let mut payer = super::super::tests::payer(function, budget);
    inventory_text::current(&mut payer)?;
    payer.bytes(std::mem::size_of::<Builder<'_, '_>>())?;
    metadata::step(&mut payer, 2)?;
    let mut builder = Builder {
        program,
        payer: &mut payer,
        summary: Summary::default(),
        shape_frames: Vec::new(),
    };
    let mut previous_capacity = 0;
    let mut last = None;
    for root in roots {
        let outcome = if saved {
            builder.shape_original(root)
        } else {
            builder.shape(root)
        };
        if !saved {
            assert!(builder.shape_frames.is_empty());
            assert!(builder.shape_frames.capacity() >= previous_capacity);
            previous_capacity = builder.shape_frames.capacity();
        }
        last = Some(outcome?);
    }
    Ok((builder.summary, last))
}
fn receive(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    roots: &[&ResolvedType],
    saved: bool,
    bytes: usize,
) -> ((Result<Outcome, ()>, Metrics), bool, usize) {
    crate::bounded_output::with_limit_usage(bytes, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let result = classify(program, function, roots, saved, &mut budget);
            if budget.refusal_spent || crate::bounded_output::active_refused() {
                let before = crate::test_allocation_observer::current_metrics();
                let retry = collect_with_payer(
                    program,
                    function,
                    &mut super::super::tests::payer(function, &mut budget),
                );
                assert!(retry.is_err());
                assert_eq!(
                    before.allocated,
                    crate::test_allocation_observer::current_metrics().allocated
                );
                drop(retry);
            }
            let observed = result.as_ref().copied().map_err(|_| ());
            drop(result);
            drop(budget);
            observed
        })
    })
}

#[test]
fn repeated_shape_frontier_keeps_full_old_results_paid_backing_drop_and_short_sticky() {
    for program in super::super::tests::fixtures() {
        for function in &program.functions {
            let scalar = ResolvedType::Bytes;
            let roots = [&scalar; 17];
            let ((old, old_metrics), old_refused, old_paid) =
                receive(&program, function, &roots, true, usize::MAX);
            let ((current, metrics), refused, paid) =
                receive(&program, function, &roots, false, usize::MAX);
            assert_eq!(current, old);
            assert!(!old_refused && !refused && !metrics.invalid && !old_metrics.invalid);
            assert!(metrics.allocated <= paid && old_metrics.allocated <= old_paid);
            assert!(metrics.allocated < old_metrics.allocated);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
            let ((exact, exact_metrics), refused, exact_paid) =
                receive(&program, function, &roots, false, paid);
            assert_eq!(exact, current);
            assert!(!refused && !exact_metrics.invalid);
            assert_eq!(exact_paid, paid);
            for limit in [0, paid - 1] {
                let ((short, metrics), refused, used) =
                    receive(&program, function, &roots, false, limit);
                assert!(short.is_err() && refused && !metrics.invalid);
                assert!(metrics.allocated <= used);
                assert_eq!(metrics.live, 0);
                assert_eq!(metrics.allocated, metrics.freed);
            }
        }
    }
}

#[test]
fn complete_original_shape_receiver_keeps_every_fixture_root_and_full_flag_result() {
    for program in super::super::tests::fixtures() {
        for function in &program.functions {
            for ty in function
                .params
                .iter()
                .map(|parameter| &parameter.ty)
                .chain(std::iter::once(&function.return_type))
            {
                let roots = [ty];
                let ((old, current), _, _) =
                    crate::bounded_output::with_limit_usage(usize::MAX, || {
                        let old =
                            classify(&program, function, &roots, true, &mut ReplayBudget::new());
                        let current =
                            classify(&program, function, &roots, false, &mut ReplayBudget::new());
                        (old, current)
                    });
                assert_eq!(current, old);
            }
        }
    }
}

#[test]
fn whole_original_shape_source_remains_byte_exact_with_one_named_receiver_hook() {
    let original = include_str!("../../../../../docs/evidence/schema-summary-frontier-reuse/src__cleanup_plan__replay__paid_type_facts__inventory_summary__shape.rs.before.txt");
    assert_eq!(
        include_str!("shape_original.rs").replace(
            "    pub(super) fn shape_original(",
            "    pub(super) fn shape("
        ),
        original
    );
}

#[test]
fn complete_current_shape_and_collector_are_exact_original_with_named_frontier_hooks() {
    let original = include_str!("../../../../../docs/evidence/schema-summary-frontier-reuse/src__cleanup_plan__replay__paid_type_facts__inventory_summary__shape.rs.before.txt");
    let current = include_str!("shape.rs");
    let start = current.find("    pub(super) fn shape(").unwrap();
    let body = current
        .find("        let mut profile = profile::Tracker::default();")
        .unwrap();
    assert_eq!(
        &current[start..body],
        include_str!(
            "../../../../../docs/evidence/schema-summary-frontier-reuse/shape-wrapper-hook.txt"
        )
    );
    let old_start = original.find("    pub(super) fn shape(").unwrap();
    let old_body = original
        .find("        let mut profile = profile::Tracker::default();")
        .unwrap();
    let projected = format!("{}{}{}", &current[..start], &original[old_start..old_body], &current[body..])
        .replace("pub(super) enum Frame<'a>", "enum Frame<'a>")
        .replace("impl<'s> Builder<'s, '_> {", "impl Builder<'_, '_> {")
        .replace("        let mut profile = profile::Tracker::default();", "        let mut profile = profile::Tracker::default();\n        let mut frames = inventory_flags::vector(self.payer)?;")
        .replace("inventory_flags::push(frames,", "inventory_flags::push(&mut frames,")
        .replace("                        frames,", "                        &mut frames,")
        .replace("                                    frames,", "                                    &mut frames,");
    assert_eq!(projected, original);
    assert_eq!(include_str!("collector.rs")
        .replace("impl<'s> Builder<'s, '_> {", "impl Builder<'_, '_> {")
        .replace("&'s ResolvedExpr", "&ResolvedExpr")
        .replace("&'s crate::hir::ResolvedMatchPattern", "&crate::hir::ResolvedMatchPattern"),
        include_str!("../../../../../docs/evidence/schema-summary-frontier-reuse/src__cleanup_plan__replay__paid_type_facts__inventory_summary__collector.rs.before.txt"));
}
