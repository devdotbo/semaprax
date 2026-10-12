//! Wire evidence only; no decoded carrier selects a runtime allocator.
use super::*;
use crate::cleanup_plan::{StatusSource, StatusSourceId, CLEANUP_PLAN_SCHEMA_V19};

#[test]
fn allocation_tags_append_without_changing_legacy_tags_and_reject_unknown_cases() {
    for (lane, bytes) in [(StatusLane::OperationFailure, vec![0, 0]),
        (StatusLane::ContractFalse, vec![1, 0]), (StatusLane::OwnerAdmission, vec![2, 0])] {
        assert_eq!(super::super::encode(&lane).unwrap(), bytes);
        assert_eq!(super::super::decode::<StatusLane>(&bytes).unwrap(), lane);
    }
    assert_eq!(super::super::encode(&StatusLane::AllocationFailure).unwrap(), [3, 0]);
    let producer = StatusProducer::CheckedAllocation {
        operation: AllocationOperation::BytesCopy,
        normalized_cases: [AllocationCase::PayloadRefused, AllocationCase::SizeOverflow],
    };
    // Independent literal tags: producer3, operation11, cases0 then1; no Vec
    // length/backing is hidden in the closed two-case carrier.
    let bytes = vec![3, 0, 11, 0, 0, 0, 1, 0];
    assert_eq!(super::super::encode(&producer).unwrap(), bytes);
    assert_eq!(super::super::decode::<StatusProducer>(&bytes).unwrap(), producer);
    for bytes in [vec![3, 0, 23, 0, 0, 0, 1, 0], vec![3, 0, 11, 0, 1, 0, 0, 0],
        vec![3, 0, 11, 0, 0, 0, 0, 0], vec![3, 0, 11, 0, 0, 0, 2, 0], vec![4, 0]] {
        assert!(super::super::decode::<StatusProducer>(&bytes).is_err());
    }
}

#[test]
fn allocation_wire_requires_v20_and_never_becomes_ordinary_hir_authority() {
    let source = crate::check("module cache.allocation; @id(\"cache.main\") fn main()->i64 { 0 }", "allocation-cache.spx").unwrap();
    let mut program = crate::hir::resolve(&source).unwrap();
    let mut plan = program.functions[0].cleanup_plan.clone();
    plan.schema = CLEANUP_PLAN_SCHEMA_V20;
    plan.status_sources.push(StatusSource {
        id: StatusSourceId { expression: program.functions[0].body.id.clone(), lane: StatusLane::AllocationFailure },
        producer: StatusProducer::CheckedAllocation { operation: AllocationOperation::StringLiteral,
            normalized_cases: [AllocationCase::PayloadRefused, AllocationCase::SizeOverflow] },
    });
    let bytes = super::super::encode(&plan).unwrap();
    assert_eq!(super::super::decode::<CleanupPlan>(&bytes).unwrap(), plan);
    let mut old = plan.clone(); old.schema = CLEANUP_PLAN_SCHEMA_V19;
    assert!(super::super::encode(&old).is_err());
    let mut forged = bytes.clone();
    let old = b"semaprax.cleanup-plan.v19";
    let new = b"semaprax.cleanup-plan.v20";
    let offset = forged.windows(new.len()).position(|part| part == new).unwrap();
    forged[offset..offset + old.len()].copy_from_slice(old);
    assert!(super::super::decode::<CleanupPlan>(&forged).is_err());
    let mut wrong = plan.clone(); wrong.status_sources[0].id.lane = StatusLane::OperationFailure;
    assert!(super::super::encode(&wrong).is_err());
    program.functions[0].cleanup_plan = super::super::decode(&bytes).unwrap();
    assert!(crate::hir::validate(&program).is_err(), "wire is comparison data, never ordinary allocation admission");
}
