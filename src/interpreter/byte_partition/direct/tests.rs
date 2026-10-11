//! Source-entry controls. Authored but not executed in this implementation lane.
use super::{execute, Account, Outcome};
use crate::hir::byte_partition::ValidatedPartitionProgram;
fn source(extent: u64, count: u64, width: u64) -> String {
    format!(
        r#"module partition_direct;
@id("partition_direct.main") fn main()->i64 {{
 let whole=bytes_zeroed({extent}usize);
 let children=bytes_partition(whole,{count}usize,{width}usize);
 i64_from_usize(vec_len<Bytes>(children))
}}
"#
    )
}
#[test]
fn original_4096_source_uses_one_backing_and_canonical_scalar_publication() {
    for (extent, count, width) in [(12288, 4096, 3), (3, 1, 3), (0, 0, u64::MAX)] {
        let text = source(extent, count, width);
        assert!(crate::check(&text, "partition-direct.spx").is_err());
        let ast = crate::parse(&text, "partition-direct.spx").unwrap();
        let owner = ValidatedPartitionProgram::compile_source(&ast, &mut usize::MAX).unwrap();
        let (mut allocations, mut payload) = (0, 0);
        {
            // Explicit test funding; no forecast-derived production grant.
            let account = Account::funded(&mut allocations, &mut payload, 8_388_608).unwrap();
            for attempt in 1..=3 {
                let report = execute(&owner, &account, &mut usize::MAX);
                assert_eq!(report.outcome, Outcome::Returned(count as i64));
                assert!(report.cleanup_complete);
                assert_eq!(
                    (report.allocations, report.payload),
                    (attempt, extent * u64::from(attempt))
                );
            }
        }
        assert_eq!((allocations, payload), (3, extent * 3));
    }
}
#[test]
fn semantic_fault_is_not_published_or_misreported_as_canonical_cleanup() {
    let text = source(3, 1, 0);
    let ast = crate::parse(&text, "partition-fault.spx").unwrap();
    let owner = ValidatedPartitionProgram::compile_source(&ast, &mut usize::MAX).unwrap();
    let (mut allocations, mut payload) = (0, 0);
    let account = Account::funded(&mut allocations, &mut payload, 8_388_608).unwrap();
    let report = execute(&owner, &account, &mut usize::MAX);
    assert_eq!(
        report.outcome,
        Outcome::PartitionFault(crate::construction_fusion::partition::Fault::WidthZero)
    );
    assert!(!report.cleanup_complete);
    assert_eq!((report.allocations, report.payload), (1, 3));
}
#[test]
fn unsupported_helper_and_receiving_shortage_refuse_before_payload_effects() {
    let text = source(3, 1, 3);
    let helper = text.replace("bytes_zeroed(3usize)", "allocate()")
        + r#"@id("partition_direct.allocate") fn allocate()->Bytes {bytes_zeroed(3usize)}"#;
    let helper_ast = crate::parse(&helper, "partition-refusal.spx").unwrap();
    assert!(ValidatedPartitionProgram::compile_source(&helper_ast, &mut usize::MAX).is_err());
    let extra = text.replace(
        " let children=",
        " let extra=0;
 let children=",
    );
    for (text, remaining) in [(text, 0), (extra, usize::MAX)] {
        let ast = crate::parse(&text, "partition-refusal.spx").unwrap();
        let owner = ValidatedPartitionProgram::compile_source(&ast, &mut usize::MAX).unwrap();
        let (mut allocations, mut payload) = (0, 0);
        let account = Account::funded(&mut allocations, &mut payload, 8_388_608).unwrap();
        let report = execute(&owner, &account, &mut { remaining });
        assert_eq!(report.outcome, Outcome::Refused);
        assert!(!report.cleanup_complete);
        assert_eq!((report.allocations, report.payload), (0, 0));
    }
    let ast = crate::parse(&source(3, 1, 3), "partition-unfunded.spx").unwrap();
    let owner = ValidatedPartitionProgram::compile_source(&ast, &mut usize::MAX).unwrap();
    let (mut allocations, mut payload) = (0, 0);
    let headers = std::alloc::Layout::new::<Account<'_>>().size()
        + std::alloc::Layout::new::<super::super::funding::Handle<'_, '_>>().size()
        + std::alloc::Layout::new::<super::super::funding::BorrowedAccount<'_>>().size();
    let account = Account::funded(&mut allocations, &mut payload, headers).unwrap();
    let report = execute(&owner, &account, &mut usize::MAX);
    assert_eq!(report.outcome, Outcome::Refused);
    assert_eq!((report.allocations, report.payload), (0, 0));
}
