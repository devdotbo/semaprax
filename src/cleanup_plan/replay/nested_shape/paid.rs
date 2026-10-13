//! Same ordered shape derivation with paid direct reconstruction storage.
//! Declaration fact-cache miss reconstruction remains the original classifier.
use super::*;
use crate::cleanup_plan::replay::ordinary_inventory::{
    storage::{array, push},
    substitute,
};

mod scratch;
pub(super) use scratch::Scratch;
use scratch::{CaseMeta, Frame};

pub(super) fn derive(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    root: &ResolvedType,
    next_flag: &mut u32,
    depth: usize,
    budget: &mut Budget,
    physical: &mut ReplayBudget,
) -> Result<FieldLivenessShape, Diagnostic> {
    derive_reusing(
        program,
        function,
        root,
        next_flag,
        (depth, budget),
        physical,
        &mut Scratch::default(),
    )
}

pub(super) fn derive_reusing(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    root: &ResolvedType,
    next_flag: &mut u32,
    bounds: (usize, &mut Budget),
    physical: &mut ReplayBudget,
    scratch: &mut Scratch,
) -> Result<FieldLivenessShape, Diagnostic> {
    let (depth, budget) = bounds;
    physical.reserve_clone(function, root, "independent shape root type")?;
    let mut lease = scratch.lease();
    let (frames, shapes) = lease.parts();
    physical.reserve_vec(function, frames, 1, "independent shape frames")?;
    frames.push(Frame::Enter(root.clone(), depth));
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(ty, depth) => {
                if !type_needs_drop_with_budget(program, function, &ty, physical)? {
                    push(
                        shapes,
                        FieldLivenessShape::NoDrop,
                        function,
                        physical,
                        "independent shape results",
                    )?;
                    continue;
                }
                if let Some(id) = crate::cleanup::primitive_leaf_lifecycle(&ty) {
                    if depth > 1 && !nested_schema(function.cleanup_plan.schema) {
                        return Err(replay_error(
                            function,
                            "nested compiler-owned Bytes cleanup leaf is outside flat record v1",
                        ));
                    }
                    charge_leaf(function, budget)?;
                    let flag = next_flag_id(function, next_flag)?;
                    push(
                        shapes,
                        FieldLivenessShape::Leaf {
                            flag,
                            lifecycle: DeclarationId::paid_borrowed(id, |bytes| {
                                physical.reserve_bytes(
                                    function,
                                    bytes,
                                    "independent shape lifecycle identity",
                                )
                            })?,
                        },
                        function,
                        physical,
                        "independent shape results",
                    )?;
                    continue;
                }
                if crate::cleanup::is_owned_bounded_vec_type(&ty)
                    || crate::hir::owned_record_collection::is_owned_record_vec_type(
                        program.declarations(),
                        &ty,
                    )
                    || crate::hir::owned_leaf_collection::is_copy_or_leaf_vec(
                        program.declarations(),
                        &ty,
                    )
                {
                    charge_leaf(function, budget)?;
                    push(
                        shapes,
                        FieldLivenessShape::Leaf {
                            flag: next_flag_id(function, next_flag)?,
                            lifecycle: DeclarationId::paid_borrowed(
                                crate::cleanup::VEC_DROP_LIFECYCLE_ID,
                                |bytes| {
                                    physical.reserve_bytes(
                                        function,
                                        bytes,
                                        "independent shape lifecycle identity",
                                    )
                                },
                            )?,
                        },
                        function,
                        physical,
                        "independent shape results",
                    )?;
                    continue;
                }
                if crate::iterator_ops::is_iter(&ty) {
                    if !matches!(
                        function.cleanup_plan.schema,
                        CLEANUP_PLAN_SCHEMA_V10
                            | CLEANUP_PLAN_SCHEMA_V11
                            | CLEANUP_PLAN_SCHEMA_V12
                            | CLEANUP_PLAN_SCHEMA_V13
                            | CLEANUP_PLAN_SCHEMA_V15
                            | CLEANUP_PLAN_SCHEMA_V16
                            | CLEANUP_PLAN_SCHEMA_V17
                            | CLEANUP_PLAN_SCHEMA_V18
                            | CLEANUP_PLAN_SCHEMA_V19
                            | CLEANUP_PLAN_SCHEMA_V14
                    ) {
                        return Err(replay_error(
                            function,
                            "iterator cleanup requires schema v10",
                        ));
                    }
                    charge_leaf(function, budget)?;
                    push(
                        shapes,
                        FieldLivenessShape::Leaf {
                            flag: next_flag_id(function, next_flag)?,
                            lifecycle: DeclarationId::paid_borrowed(
                                crate::cleanup::ITER_DROP_LIFECYCLE_ID,
                                |bytes| {
                                    physical.reserve_bytes(
                                        function,
                                        bytes,
                                        "independent shape lifecycle identity",
                                    )
                                },
                            )?,
                        },
                        function,
                        physical,
                        "independent shape results",
                    )?;
                    continue;
                }
                if crate::cleanup::is_owned_bounded_box_type(&ty) {
                    charge_leaf(function, budget)?;
                    push(
                        shapes,
                        FieldLivenessShape::Leaf {
                            flag: next_flag_id(function, next_flag)?,
                            lifecycle: DeclarationId::paid_borrowed(
                                crate::cleanup::BOX_DROP_LIFECYCLE_ID,
                                |bytes| {
                                    physical.reserve_bytes(
                                        function,
                                        bytes,
                                        "independent shape lifecycle identity",
                                    )
                                },
                            )?,
                        },
                        function,
                        physical,
                        "independent shape results",
                    )?;
                    continue;
                }
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &ty
                else {
                    return Err(replay_error(
                        function,
                        "droppable supplemental slot is not nominal",
                    ));
                };
                let item = program
                    .types()
                    .iter()
                    .find(|item| item.id == *declaration)
                    .ok_or_else(|| {
                        replay_error(
                            function,
                            format_args!("unknown cleanup type `{declaration}`"),
                        )
                    })?;
                match &item.kind {
                    ResolvedTypeDeclarationKind::Resource { drop } => {
                        if !arguments.is_empty() {
                            return Err(replay_error(
                                function,
                                "resource cleanup slot has generic arguments",
                            ));
                        }
                        charge_leaf(function, budget)?;
                        push(
                            shapes,
                            FieldLivenessShape::Leaf {
                                flag: next_flag_id(function, next_flag)?,
                                lifecycle: drop.id.clone(),
                            },
                            function,
                            physical,
                            "independent shape results",
                        )?;
                    }
                    ResolvedTypeDeclarationKind::Record { fields }
                    | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                        if !arguments.is_empty()
                            && !matches!(&item.kind, ResolvedTypeDeclarationKind::Record { .. })
                        {
                            return Err(replay_error(
                                function,
                                "record cleanup slot has generic arguments",
                            ));
                        }
                        let child_depth = depth.checked_add(1).ok_or_else(|| {
                            replay_error(function, "cleanup projection depth overflowed")
                        })?;
                        if nested_schema(function.cleanup_plan.schema)
                            && child_depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH
                        {
                            return Err(replay_error(
                                function,
                                format_args!(
                                    "cleanup shape exceeds the {} record-depth limit",
                                    crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH
                                ),
                            ));
                        }
                        charge_fields(function, budget, fields.len())?;
                        let mut meta = array(
                            function,
                            fields.len(),
                            physical,
                            "independent record metadata",
                        )?;
                        meta.extend(fields.iter().map(|field| (field.id.clone(), field.index)));
                        physical.reserve_vec(
                            function,
                            frames,
                            fields.len().saturating_add(1),
                            "independent record frames",
                        )?;
                        frames.push(Frame::FinishRecord(declaration.clone(), meta));
                        for field in fields.iter().rev() {
                            frames.push(Frame::Enter(
                                substitute::substitute_type(
                                    &field.ty,
                                    declaration,
                                    arguments,
                                    function,
                                    physical,
                                )?,
                                child_depth,
                            ));
                        }
                    }
                    ResolvedTypeDeclarationKind::Variant { cases } => {
                        let count = cases
                            .iter()
                            .try_fold(0usize, |count, case| count.checked_add(case.fields.len()))
                            .ok_or_else(|| {
                                replay_error(function, "cleanup visited-field count overflowed")
                            })?;
                        charge_fields(function, budget, count)?;
                        physical.reserve_clone(
                            function,
                            arguments,
                            "independent variant container arguments",
                        )?;
                        let container = ResolvedType::Nominal {
                            declaration: declaration.clone(),
                            arguments: arguments.clone(),
                        };
                        let mut meta = array(
                            function,
                            cases.len(),
                            physical,
                            "independent variant metadata",
                        )?;
                        let mut entries =
                            array(function, count, physical, "independent variant entries")?;
                        for case in cases {
                            let mut case_fields = array(
                                function,
                                case.fields.len(),
                                physical,
                                "independent variant fields",
                            )?;
                            for field in &case.fields {
                                let ty = substitute::substitute_type(
                                    &field.ty,
                                    declaration,
                                    arguments,
                                    function,
                                    physical,
                                )?;
                                let nested_record = crate::cleanup::variant_record_field(
                                    program, &container, &case.id, &field.id, &ty,
                                );
                                if type_needs_drop_with_budget(program, function, &ty, physical)?
                                    && !nested_record
                                    && crate::cleanup::variant_leaf_lifecycle(
                                        program, &container, &case.id, &field.id, &ty,
                                    )
                                    .is_none()
                                {
                                    return Err(replay_error(
                                        function,
                                        "variant owner is outside its admitted cleanup profile",
                                    ));
                                }
                                case_fields.push((field.id.clone(), field.index));
                                entries.push(ty);
                            }
                            meta.push(CaseMeta {
                                case: case.id.clone(),
                                case_index: case.index,
                                fields: case_fields,
                            });
                        }
                        physical.reserve_vec(
                            function,
                            frames,
                            entries.len().saturating_add(1),
                            "independent variant frames",
                        )?;
                        frames.push(Frame::FinishVariant(declaration.clone(), meta));
                        for ty in entries.into_iter().rev() {
                            frames.push(Frame::Enter(ty, depth));
                        }
                    }
                }
            }
            Frame::FinishRecord(declaration, meta) => {
                let start = shapes.len().checked_sub(meta.len()).ok_or_else(|| {
                    replay_error(function, "cleanup record field shape is absent")
                })?;
                let mut expected = array(
                    function,
                    meta.len(),
                    physical,
                    "independent record field shapes",
                )?;
                expected.extend(meta.into_iter().zip(shapes.drain(start..)).map(
                    |((field, field_index), shape)| FieldLiveness {
                        field,
                        field_index,
                        shape,
                    },
                ));
                push(
                    shapes,
                    FieldLivenessShape::Record {
                        declaration,
                        fields: expected,
                    },
                    function,
                    physical,
                    "independent shape results",
                )?;
            }
            Frame::FinishVariant(declaration, meta) => {
                let total = meta.iter().map(|case| case.fields.len()).sum::<usize>();
                let start = shapes.len().checked_sub(total).ok_or_else(|| {
                    replay_error(function, "cleanup variant field shape is absent")
                })?;
                let mut children = array(
                    function,
                    total,
                    physical,
                    "independent variant child shapes",
                )?;
                children.extend(shapes.drain(start..));
                let mut children = children.into_iter();
                let mut expected_cases = array(
                    function,
                    meta.len(),
                    physical,
                    "independent variant case shapes",
                )?;
                for case in meta {
                    let mut expected_fields = array(
                        function,
                        case.fields.len(),
                        physical,
                        "independent variant field shapes",
                    )?;
                    for (field, field_index) in case.fields {
                        let shape = children.next().ok_or_else(|| {
                            replay_error(function, "cleanup variant field shape is absent")
                        })?;
                        expected_fields.push(FieldLiveness {
                            field,
                            field_index,
                            shape,
                        });
                    }
                    expected_cases.push(crate::cleanup::VariantCaseLiveness {
                        case: case.case,
                        case_index: case.case_index,
                        fields: expected_fields,
                    });
                }
                push(
                    shapes,
                    FieldLivenessShape::Variant {
                        declaration,
                        cases: expected_cases,
                    },
                    function,
                    physical,
                    "independent shape results",
                )?;
            }
        }
    }
    let shape = shapes
        .pop()
        .ok_or_else(|| replay_error(function, "cleanup shape is absent"))?;
    if !shapes.is_empty() {
        return Err(replay_error(function, "cleanup shape did not settle"));
    }
    Ok(shape)
}
