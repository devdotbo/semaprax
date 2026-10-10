use super::*;
use crate::cleanup_plan::*;
use crate::hir::{ControlTarget, LoopLatchKind};

fn control() -> LexicalControlPlan {
    LexicalControlPlan {
        schema: "semaprax.lexical-control-flow.v1",
        loops: Vec::new(),
        exits: Vec::new(),
        resumes: Vec::new(),
        abrupt_only_regions: Vec::new(),
        function_success_exits: Vec::new(),
        execution_boundaries: Vec::new(),
    }
}
fn census(control: LexicalControlPlan) -> usize {
    let mut plan = CleanupPlan::unresolved();
    plan.lexical_control = Some(Box::new(control));
    crate::private_capacity_contract::cleanup_plan_owned_capacity(&plan).unwrap()
}

#[test]
fn lexical_certificate_spare_backings_remain_fully_charged_without_live_entries() {
    let baseline = census(control());
    assert_eq!(baseline, std::mem::size_of::<LexicalControlPlan>());
    let mut item = control();
    item.loops = Vec::with_capacity(7);
    item.exits = Vec::with_capacity(9);
    item.resumes = Vec::with_capacity(11);
    item.abrupt_only_regions = Vec::with_capacity(13);
    item.function_success_exits = Vec::with_capacity(17);
    let physical = item.loops.capacity() * std::mem::size_of::<LoopCertificate>()
        + item.exits.capacity() * std::mem::size_of::<LexicalExitCertificate>()
        + item.resumes.capacity() * std::mem::size_of::<ResumeCertificate>()
        + item.abrupt_only_regions.capacity() * std::mem::size_of::<CleanupRegionId>()
        + item.function_success_exits.capacity() * std::mem::size_of::<ExitTargetId>();
    assert_eq!(census(item) - baseline, physical);
    assert_eq!(
        crate::private_capacity_contract::cleanup_plan_owned_capacity(&CleanupPlan::unresolved()),
        Some(0)
    );
}

#[test]
fn lexical_identity_occurrences_charge_full_backing_and_invalid_latches_refuse() {
    let id = ExpressionId::from_owned("lexical.capacity.site".to_owned());
    let owner = FunctionExecutionId::Monomorphic(DeclarationId::new("capacity.fn"));
    let target = ControlTarget::Loop {
        owner,
        site: id.clone(),
    };
    let mut item = control();
    item.exits.push(LexicalExitCertificate {
        site: id.clone(),
        target: target.clone(),
        kind: crate::ast::LexicalExitKind::Continue,
        exit: ExitTargetId(0),
        resume: Some(BlockId(1)),
    });
    item.resumes.push(ResumeCertificate {
        entry: BlockId(1),
        blocks: Vec::with_capacity(19),
    });
    let headers = std::mem::size_of::<LexicalControlPlan>()
        + item.exits.capacity() * std::mem::size_of::<LexicalExitCertificate>()
        + item.resumes.capacity() * std::mem::size_of::<ResumeCertificate>()
        + item.resumes[0].blocks.capacity() * std::mem::size_of::<BlockId>();
    assert_eq!(
        census(item),
        headers + 2 * id.owned_allocation_bytes().unwrap() + "capacity.fn".len()
    );
    let (invalid, overflowed, _) = crate::bounded_output::with_limit_usage(0, || {
        ExpressionId::from_owned("invalid".to_owned())
    });
    assert!(overflowed);
    assert_eq!(
        crate::private_capacity_contract::control_target_owned_capacity(&ControlTarget::Loop {
            owner: FunctionExecutionId::Monomorphic(DeclarationId::new("capacity.fn")),
            site: invalid.clone(),
        }),
        None
    );
    assert_eq!(
        crate::private_capacity_contract::latch_owned_capacity(&LoopLatchKind::SourceForStep {
            kind: crate::hir::SourceForKind::Copy,
            step: invalid,
        }),
        None
    );
    assert_eq!(
        crate::private_capacity_contract::latch_owned_capacity(&LoopLatchKind::Condition),
        Some(0)
    );
}

#[test]
fn execution_boundary_spare_actions_and_region_backing_are_retained() {
    let baseline = census(control());
    let id = ExpressionId::from_owned("capacity.boundary".to_owned());
    let mut item = control();
    item.execution_boundaries = Vec::with_capacity(23);
    item.execution_boundaries
        .push(ExecutionBoundaryCertificate {
            kind: ExecutionBoundaryKind::LoopIteration,
            site: id.clone(),
            block: BlockId(0),
            region: CleanupRegionId(0),
            leaves_regions: Vec::with_capacity(29),
            finalize_in_order: Vec::with_capacity(31),
        });
    let retained = item.execution_boundaries.capacity()
        * std::mem::size_of::<ExecutionBoundaryCertificate>()
        + item.execution_boundaries[0].leaves_regions.capacity()
            * std::mem::size_of::<CleanupRegionId>()
        + item.execution_boundaries[0].finalize_in_order.capacity()
            * std::mem::size_of::<FinalizeAction>()
        + id.owned_allocation_bytes().unwrap();
    assert_eq!(census(item) - baseline, retained);
}

#[test]
fn execution_boundary_actions_charge_case_and_projection_payloads_and_refuse_invalid_backing() {
    let id = ExpressionId::from_owned("capacity.action".to_owned());
    let mut item = control();
    item.execution_boundaries
        .push(ExecutionBoundaryCertificate {
            kind: ExecutionBoundaryKind::FunctionEntry,
            site: id.clone(),
            block: BlockId(0),
            region: CleanupRegionId(0),
            leaves_regions: Vec::new(),
            finalize_in_order: Vec::with_capacity(3),
        });
    let mut plan = CleanupPlan::unresolved();
    plan.lexical_control = Some(Box::new(item));
    let before = crate::private_capacity_contract::cleanup_plan_owned_capacity(&plan).unwrap();
    let item = plan.lexical_control.as_mut().unwrap();
    item.execution_boundaries[0]
        .finalize_in_order
        .push(FinalizeAction {
            source: CleanupPlace {
                storage: StorageId::Temporary(id.clone()),
                projections: vec![DeclarationId::new("field")],
            },
            lifecycle_id: DeclarationId::new("drop"),
            guard_flag: crate::cleanup::LivenessFlagId(0),
            active_case: Some(VariantCaseGuard {
                storage: StorageId::Temporary(id.clone()),
                variant: DeclarationId::new("variant"),
                case: DeclarationId::new("case"),
            }),
        });
    let action = &item.execution_boundaries[0].finalize_in_order[0];
    let expected = 2 * id.owned_allocation_bytes().unwrap()
        + action.source.projections.capacity() * std::mem::size_of::<DeclarationId>()
        + "fielddropvariantcase".len();
    assert_eq!(
        crate::private_capacity_contract::cleanup_plan_owned_capacity(&plan).unwrap() - before,
        expected
    );
    let (invalid, _, _) = crate::bounded_output::with_limit_usage(0, || {
        ExpressionId::from_owned("invalid.action".to_owned())
    });
    plan.lexical_control.as_mut().unwrap().execution_boundaries[0].finalize_in_order[0]
        .active_case
        .as_mut()
        .unwrap()
        .storage = StorageId::Temporary(invalid);
    assert_eq!(
        crate::private_capacity_contract::cleanup_plan_owned_capacity(&plan),
        None
    );
}
