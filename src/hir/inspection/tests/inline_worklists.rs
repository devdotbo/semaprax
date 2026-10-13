//! Complete old walkers remain independent. Allocation scopes include Drop;
//! inline success is ordinary scratch elimination, never paid-spill evidence.
use super::*;
use crate::cleanup::{FieldLiveness, FieldLivenessShape, LivenessFlagId, VariantCaseLiveness};
use crate::test_allocation_observer::{measure, Metrics};
mod old;

fn settled(metrics: Metrics) {
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
}

fn nominal(id: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    }
}

fn record(shapes: Vec<FieldLivenessShape>) -> FieldLivenessShape {
    FieldLivenessShape::Record {
        declaration: DeclarationId::new("record"),
        fields: shapes
            .into_iter()
            .enumerate()
            .map(|(index, shape)| FieldLiveness {
                field: DeclarationId::new(format!("field.{index}")),
                field_index: index as u32,
                shape,
            })
            .collect(),
    }
}

#[test]
fn compound_types_keep_full_old_diagnostics_and_unbounded_spill() {
    for width in [0, 1, 31, 32, 33, 65, 1024] {
        let ty = nominal("root", vec![ResolvedType::I64; width]);
        let (now, current) = measure(|| audit_resolved_type(&ty));
        let (before, saved) = measure(|| old::run_type(&ty));
        assert_eq!(now, before);
        settled(current);
        settled(saved);
        if width <= 32 {
            assert_eq!(current.allocations, 0);
        } else {
            assert!(current.allocations > 0);
        }
        assert!(saved.allocations > 0);
    }
    // Result precedes reverse parameter order in the full old stack.
    let ty = ResolvedType::Function {
        parameters: vec![
            nominal("parameter\0first", Vec::new()),
            nominal("parameter\0last", Vec::new()),
        ],
        result: Box::new(nominal("result\0first", Vec::new())),
    };
    assert_eq!(audit_resolved_type(&ty), old::run_type(&ty));
    for width in [31, 32, 33, 65] {
        let mut args = vec![ResolvedType::I64; width];
        args[0] = ResolvedType::TypeParameter {
            owner: DeclarationId::new("owner\0first"),
            index: u32::MAX,
        };
        args[width - 1] = nominal("last\0first", Vec::new());
        let malformed = nominal("root", args);
        assert_eq!(audit_resolved_type(&malformed), old::run_type(&malformed));
    }
    let mut deep = ResolvedType::I64;
    for _ in 0..512 {
        deep = nominal("deep", vec![deep]);
    }
    assert_eq!(audit_resolved_type(&deep), old::run_type(&deep));
}

#[test]
fn shape_terminals_compounds_and_malformed_indices_keep_the_full_old_verdict() {
    let leaf = FieldLivenessShape::Leaf {
        flag: LivenessFlagId(u32::MAX),
        lifecycle: DeclarationId::new("drop"),
    };
    for shape in [
        FieldLivenessShape::NoDrop,
        leaf.clone(),
        record(vec![leaf.clone(); 32]),
    ] {
        let (now, current) = measure(|| audit_field_liveness_shape(&shape));
        let (before, saved) = measure(|| old::run_shape(&shape));
        assert_eq!(now, before);
        settled(current);
        settled(saved);
        assert_eq!(current.allocations, 0);
        assert!(saved.allocations > 0);
    }
    for width in [33, 65, 1024] {
        let shape = record(vec![leaf.clone(); width]);
        let (now, current) = measure(|| audit_field_liveness_shape(&shape));
        let (before, saved) = measure(|| old::run_shape(&shape));
        assert_eq!(now, before);
        assert!(current.allocations > 0);
        settled(current);
        settled(saved);
    }
    let malformed_leaf = FieldLivenessShape::Leaf {
        flag: LivenessFlagId(0),
        lifecycle: DeclarationId::new("drop\0bad"),
    };
    let mut deep = malformed_leaf.clone();
    for _ in 0..512 {
        deep = record(vec![deep]);
    }
    for shape in [
        malformed_leaf.clone(),
        deep,
        record(vec![malformed_leaf; 65]),
    ] {
        assert_eq!(audit_field_liveness_shape(&shape), old::run_shape(&shape));
    }
    let mut shape = FieldLivenessShape::Variant {
        declaration: DeclarationId::new("variant"),
        cases: vec![VariantCaseLiveness {
            case: DeclarationId::new("case"),
            case_index: 7,
            fields: Vec::new(),
        }],
    };
    assert_eq!(audit_field_liveness_shape(&shape), old::run_shape(&shape));
    let error = audit_field_liveness_shape(&shape).unwrap_err();
    assert_eq!(
        error.message,
        "cleanup variant case inventory has a non-canonical index"
    );
    if let FieldLivenessShape::Variant { cases, .. } = &mut shape {
        cases[0].case = DeclarationId::new("case\0bad");
    }
    assert_eq!(audit_field_liveness_shape(&shape), old::run_shape(&shape));
    assert_eq!(
        audit_field_liveness_shape(&shape).unwrap_err().message,
        "cleanup variant case identity contains NUL"
    );
}

#[test]
fn expression_roots_and_complete_public_audit_receivers_keep_old_results() {
    let mut program = resolved(CALLS, "inline-audit.spx");
    assert_eq!(
        validate_nul_free_identities(&program),
        old::run_program(&program)
    );
    assert_eq!(
        validate_attached_identity_references(&program),
        old::run_attached(&program)
    );
    let (now, current) = measure(|| validate_nul_free_identities(&program));
    let (before, saved) = measure(|| old::run_program(&program));
    assert_eq!(now, before);
    assert_eq!(current.allocations, 0);
    assert!(saved.allocations > 0);
    settled(current);
    settled(saved);
    for function in &program.functions {
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            let (now, current) = measure(|| audit_resolved_expression(root));
            let (before, saved) = measure(|| old::run_expression(root));
            assert_eq!(now, before);
            assert_eq!(current.allocations, 0);
            assert!(saved.allocations > 0);
            settled(current);
            settled(saved);
        }
    }
    let mut leaf = program.functions[0].body.clone();
    leaf.kind = ResolvedExprKind::Int(1);
    leaf.ty = ResolvedType::I64;
    for width in [0, 1, 31, 32, 33, 65, 1024] {
        let mut root = leaf.clone();
        root.kind = ResolvedExprKind::Invoke {
            callable: Box::new(leaf.clone()),
            args: vec![leaf.clone(); width],
        };
        let (now, current) = measure(|| audit_resolved_expression(&root));
        let (before, saved) = measure(|| old::run_expression(&root));
        assert_eq!(now, before);
        if width < 32 {
            assert_eq!(current.allocations, 0);
        } else {
            assert!(current.allocations > 0);
        }
        settled(current);
        settled(saved);
        if let ResolvedExprKind::Invoke { callable, args } = &mut root.kind {
            callable.id = ExpressionId::from_owned("callable\0bad".to_owned());
            if let Some(last) = args.last_mut() {
                last.ty = nominal("argument\0bad", Vec::new());
            }
        }
        assert_eq!(audit_resolved_expression(&root), old::run_expression(&root));
    }
    let mut deep = leaf.clone();
    deep.id = ExpressionId::from_owned("deep\0bad".to_owned());
    for _ in 0..512 {
        let mut next = leaf.clone();
        next.kind = ResolvedExprKind::Upcast {
            source: Box::new(deep),
        };
        deep = next;
    }
    assert_eq!(audit_resolved_expression(&deep), old::run_expression(&deep));
    program.functions[0].body = leaf;
    program.functions[0].body.id = ExpressionId::from_owned("body\0bad".to_owned());
    assert_eq!(
        validate_nul_free_identities(&program),
        old::run_program(&program)
    );
    // Attached audit checks slot type before its shape, independent of all
    // semantic inventory/plan admission and without repairing malformed data.
    let result_id = program.functions[0].result_id.clone();
    program.functions[0]
        .cleanup
        .slots
        .push(crate::cleanup::CleanupStorageSlot {
            id: crate::cleanup::CleanupStorageId(0),
            discovery_index: 0,
            origin: crate::cleanup::CleanupStorageOrigin::ProvisionalResult { value: result_id },
            ty: nominal("slot", vec![ResolvedType::I64]),
            shape: FieldLivenessShape::Leaf {
                flag: LivenessFlagId(0),
                lifecycle: DeclarationId::new("shape"),
            },
        });
    let (now, current) = measure(|| validate_attached_identity_references(&program));
    let (before, saved) = measure(|| old::run_attached(&program));
    assert_eq!(now, before);
    assert!(saved.allocations > current.allocations);
    assert!(saved.allocated > current.allocated);
    settled(current);
    settled(saved);
    let slot = program.functions[0].cleanup.slots.last_mut().unwrap();
    slot.ty = nominal("slot\0bad", Vec::new());
    slot.shape = FieldLivenessShape::Leaf {
        flag: LivenessFlagId(0),
        lifecycle: DeclarationId::new("shape\0bad"),
    };
    assert_eq!(
        validate_attached_identity_references(&program),
        old::run_attached(&program)
    );
}

#[test]
fn inline_stack_keeps_new_children_ahead_of_older_spill_and_refills() {
    let values: Vec<_> = (0..100).collect();
    let mut pending = super::super::audit_pending::Pending::new(&values[0]);
    let mut saved = vec![&values[0]];
    for value in &values[1..65] {
        pending.push(value);
        saved.push(value);
    }
    for _ in 0..8 {
        assert_eq!(pending.pop(), saved.pop());
    }
    for value in &values[65..] {
        pending.push(value);
        saved.push(value);
    }
    while let Some(value) = saved.pop() {
        assert_eq!(pending.pop(), Some(value));
    }
    assert_eq!(pending.pop(), None);
    pending.extend(&values[..32]);
    for value in values[..32].iter().rev() {
        assert_eq!(pending.pop(), Some(value));
    }
    assert_eq!(pending.pop(), None);
}

#[test]
fn complete_saved_receivers_are_bound_to_the_frozen_archive() {
    let frozen = include_str!("inline_worklists/frozen_walkers.txt");
    let type_body = include_str!("type_identity/old.rs");
    let type_body = type_body[type_body.find("pub(super) fn audit_resolved_type").unwrap()..]
        .replacen("pub(super) fn", "fn", 1);
    let start = frozen.find("fn audit_resolved_type(").unwrap();
    let end = frozen
        .find("\nfn audit_resolved_record_match_pattern(")
        .unwrap();
    let expected = format!("{}{}\n{}", &frozen[..start], type_body, &frozen[end..])
        .replace("super::ControlTarget", "crate::hir::ControlTarget");
    let saved = include_str!("inline_worklists/old.rs");
    let start = saved
        .find("pub(super) fn validate_nul_free_identities(")
        .unwrap();
    let end = saved.find("\npub(super) fn run_type(").unwrap();
    assert_eq!(&saved[start..end], expected);
}

#[test]
fn complete_walk_bodies_keep_frozen_child_checks_and_push_order() {
    let current = include_str!("../../inspection.rs");
    let frozen = include_str!("inline_worklists/frozen_walkers.txt");
    for (start, end) in [
        (
            "fn audit_resolved_type(",
            "\nfn audit_resolved_record_match_pattern(",
        ),
        ("fn audit_resolved_expression(", "\nfn audit_hir_place("),
    ] {
        let current = &current[current.find(start).unwrap()..current.find(end).unwrap()];
        let frozen = &frozen[frozen.find(start).unwrap()..frozen.find(end).unwrap()];
        assert_eq!(current.replace("Pending::new(root)", "vec![root]"), frozen);
    }
    let start = "    while let Some(shape) = pending.pop() {";
    let end = "\nfn audit_inventory_place(";
    let current = &current[current.find(start).unwrap()..current.find(end).unwrap()];
    let frozen = &frozen[frozen.find(start).unwrap()..frozen.find(end).unwrap()];
    assert_eq!(current, frozen);
}
