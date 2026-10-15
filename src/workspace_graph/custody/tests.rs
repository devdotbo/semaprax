use super::*;
use crate::ast::program_source::{checked_key::CheckedSource, SyntheticProgramView};
use crate::bounded_output::{with_limit, with_limit_usage};

fn fixture() -> (Vec<WorkspaceSource>, Vec<Program>) {
    let input = [
        ("dependencies/provider.spx", "module provider; @id(\"provider.keep\") fn keep(value:i64)->i64 {value+1} @id(\"provider.unused\") fn unused()->i64 {99}"),
        ("src/consumer.spx", "module consumer; use function @id(\"provider.keep\") from provider as keep; @id(\"consumer.main\") fn main()->i64 {keep(1)}"),
    ];
    let mut sources = Vec::new();
    let mut programs = Vec::new();
    for (path, source) in input {
        let canonical = crate::format::canonical(&crate::parse(source, path).unwrap());
        let program = crate::parse(&canonical, path).unwrap();
        sources.push(WorkspaceSource {
            path: path.to_owned(),
            source: canonical,
        });
        programs.push(program);
    }
    (sources, programs)
}
fn receive(limit: usize) -> (bool, bool, usize, crate::test_allocation_observer::Metrics) {
    let (sources, mut programs) = fixture();
    let pruned = super::super::dependency_pruning::prune(&mut programs, &sources);
    assert_eq!(pruned.removed_len(0), 1);
    let ((admitted, metrics), refused, paid) = with_limit_usage(limit, || {
        crate::test_allocation_observer::measure(|| {
            let plan = SourceCustody::plan(&sources, &programs, &pruned);
            match plan {
                Ok(plan) => {
                    pruned.restore(&mut programs);
                    let custody = plan.complete(sources, programs).unwrap();
                    assert_eq!(custody.len(), 2);
                    assert_eq!(custody.programs().count(), 2);
                    assert!(custody
                        .parsed_at(0)
                        .unwrap()
                        .is_associated_with(custody.metadata_at(0).unwrap()));
                    assert_eq!(
                        custody.parsed_at(0).unwrap().original_ordinals(),
                        Some(&[0][..])
                    );
                    drop(custody);
                    true
                }
                Err(_) => {
                    assert!(!crate::bounded_output::reserve_active_required(0));
                    false
                }
            }
        })
    });
    (admitted, refused, paid, metrics)
}
#[test]
fn actual_custody_receiving_backing_complete_drop_exact_short_and_zero_sticky() {
    let (admitted, refused, paid, metrics) = receive(1 << 20);
    assert!(admitted && !refused && !metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated <= paid, "actual={metrics:?} paid={paid}");
    let exact = receive(paid);
    assert!(exact.0 && !exact.1);
    assert_eq!(exact.2, paid);
    let short = receive(paid - 1);
    assert!(!short.0 && short.1);
    assert!(!short.3.invalid && short.3.live == 0);
    let zero = receive(0);
    assert!(!zero.0 && zero.1 && zero.3.allocations == 0);
}
#[test]
fn deferred_key_reads_only_after_exact_restored_custody_and_rejects_equal_clone() {
    let (sources, mut programs) = fixture();
    let pruned = super::super::dependency_pruning::prune(&mut programs, &sources);
    let key = {
        let view = SyntheticProgramView::new(&programs[0], Vec::new()).unwrap();
        let (program, roots) = view.into_overlay_owner();
        CheckedSource::deferred(program, roots, 0)
    };
    assert!(key.ready().is_none());
    let (plan, refused) = with_limit(1 << 20, || {
        SourceCustody::plan(&sources, &programs, &pruned)
    });
    assert!(!refused);
    pruned.restore(&mut programs);
    let equal_program = programs[0].clone();
    let custody = plan.unwrap().complete(sources, programs).unwrap();
    let owner = custody.parsed_at(0).unwrap();
    assert!(key.can_bind(0, owner));
    assert!(!key.can_bind(1, owner));
    let (equal, refused) = with_limit(1 << 20, || {
        let mut reserve = |bytes| {
            if crate::bounded_output::reserve_active_required(bytes) {
                Ok(())
            } else {
                Err(())
            }
        };
        CurrentParsedSource::new(
            Arc::clone(owner.metadata()),
            equal_program,
            Some(vec![0]),
            &mut reserve,
        )
    });
    assert!(!refused);
    assert!(!key.can_bind(0, &equal.unwrap()));
    key.bind(0, owner).unwrap();
    assert!(key.ready().is_some());
    assert!(key.bind(0, owner).is_err());
    let materialized = key.materialize();
    assert_eq!(
        crate::cache_codec::encode(&key).unwrap(),
        crate::cache_codec::encode(&materialized).unwrap()
    );
    assert_eq!(materialized.functions.len(), 1);
    assert_eq!(materialized.functions[0], owner.program().functions[0]);
    assert_eq!(materialized.types, owner.program().types);
    assert!(materialized.module_uses.is_empty());
}
#[test]
fn metadata_does_not_retain_ast_generations_and_deferred_receipt_keeps_original_payer() {
    let (sources, mut programs) = fixture();
    let pruned = super::super::dependency_pruning::prune(&mut programs, &sources);
    let (plan, refused) = with_limit(1 << 20, || {
        SourceCustody::plan(&sources, &programs, &pruned)
    });
    assert!(!refused);
    pruned.restore(&mut programs);
    let (result, outer_refused, outer_paid) =
        with_limit_usage(0, || plan.unwrap().complete(sources, programs));
    assert!(result.is_ok());
    assert!(!outer_refused);
    assert_eq!(outer_paid, 0);
    drop(result);
    let (sources, mut programs) = fixture();
    let pruned = super::super::dependency_pruning::prune(&mut programs, &sources);
    let (custody, refused) = with_limit(1 << 20, || {
        let plan = SourceCustody::plan(&sources, &programs, &pruned).unwrap();
        pruned.restore(&mut programs);
        plan.complete(sources, programs).unwrap()
    });
    assert!(!refused);
    let weak = Arc::downgrade(custody.parsed_at(0).unwrap());
    let metadata = Arc::clone(custody.metadata_at(0).unwrap());
    drop(custody);
    assert!(weak.upgrade().is_none());
    assert!(metadata.input().source.contains("provider.unused"));
}
