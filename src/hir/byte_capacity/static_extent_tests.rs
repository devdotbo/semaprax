//! Exact current-function derivation and hostile proof controls. These do not
//! authorize a capacity result without the complete independent HIR validator.
use super::*;

const SOURCE: &str = r#"module extent.copy;
@id("extent.copy") fn copy()->usize {
    let input=[1u8,0u8,255u8];
    let view=array_as_slice(input);
    let copy=bytes_copy(view);
    byte_len(bytes_as_slice(copy))
}
@id("extent.main") fn main()->i64 {0}
"#;
fn resolve(source: &str) -> ResolvedProgram {
    let ast = crate::check(source, "static-copy-extent.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    program
}
fn function(program: &ResolvedProgram) -> &ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "extent.copy")
        .unwrap()
}
fn argument(function: &ResolvedFunction) -> &ResolvedExpr {
    let mut result = None;
    assert!(visit_extent_body(function, |expression| {
        if let ResolvedExprKind::Call { callee, args, .. } = &expression.kind {
            if callee.as_str() == crate::byte_ops::COPY_ID {
                result = args.first();
            }
        }
    }));
    result.unwrap()
}
fn summary(program: &ResolvedProgram) -> (u32, u64) {
    let capacity = analyze_byte_data_capacity(program).unwrap();
    let function = capacity.function("extent.copy").unwrap();
    (function.bytes_copy_sites, function.owned_byte_payload_bytes)
}
#[test]
fn current_named_array_slice_copy_uses_actual_literal_repeat_and_alias_extents() {
    for (source, expected) in [
        (SOURCE.to_owned(), 3),
        (SOURCE.replace("[1u8,0u8,255u8]", "[255u8; 5]"), 5),
        (SOURCE.replace("[1u8,0u8,255u8]", "[0u8; 0]"), 0),
        (
            SOURCE.replace(
                "let copy=bytes_copy(view);",
                "let alias=view;let copy=bytes_copy(alias);",
            ),
            3,
        ),
    ] {
        let program = resolve(&source);
        let function = function(&program);
        assert_eq!(
            static_array_slice_length(&program, function, argument(function)),
            Some(expected)
        );
        assert_eq!(summary(&program), (1, expected));
    }
}
#[test]
fn unknown_external_slice_retains_original_maximum_payload() {
    let source = r#"module extent.copy;
@id("extent.copy") fn copy(view:borrow Slice<u8>)->usize {
 let copy=bytes_copy(view);byte_len(bytes_as_slice(copy))
}
@id("extent.main") fn main()->i64 {0}
"#;
    let program = resolve(source);
    assert_eq!(
        summary(&program),
        (1, crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES)
    );
    let function = function(&program);
    assert_eq!(
        static_array_slice_length(&program, function, argument(function)),
        None
    );
}
#[test]
fn detached_function_and_equal_argument_do_not_lend_static_extent() {
    let program = resolve(SOURCE);
    let function = function(&program);
    let detached_function = function.clone();
    assert_eq!(
        static_array_slice_length(&program, &detached_function, argument(&detached_function)),
        None
    );
    let detached_argument = argument(function).clone();
    assert_eq!(
        static_array_slice_length(&program, function, &detached_argument),
        None
    );
}
#[test]
fn forged_retained_extent_cannot_reduce_current_demand_and_fails_independent_hir_replay() {
    let mut program = resolve(SOURCE);
    let view = match &argument(function(&program)).kind {
        ResolvedExprKind::Place(place) => place.root.clone(),
        _ => panic!("copy retains its named view argument"),
    };
    let provenance = program
        .declarations
        .byte_slice_roots
        .get_mut(&view)
        .unwrap();
    provenance.length = ByteSliceExtent::Constant(0);
    provenance.root_length = ByteSliceExtent::Constant(0);
    assert_eq!(
        summary(&program),
        (1, 3),
        "proof rows have no sizing authority"
    );
    assert_eq!(crate::hir::validate(&program).unwrap_err().code, "SPX-H006");
}
#[test]
fn mutable_duplicate_assigned_and_forged_alias_shapes_keep_maximum_fallback() {
    for mutation in 0..5 {
        let mut program = resolve(SOURCE);
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "extent.copy")
            .unwrap();
        let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            panic!("block");
        };
        let view_index = statements
            .iter()
            .position(|statement| {
                matches!(statement,
            ResolvedStatement::Let { binding, .. } if binding.ty == ResolvedType::SliceU8)
            })
            .unwrap();
        match mutation {
            0 => {
                let ResolvedStatement::Let { mutable, .. } = &mut statements[view_index] else {
                    unreachable!()
                };
                *mutable = true;
            }
            1 => {
                let duplicate = statements[view_index].clone();
                statements.push(duplicate);
            }
            2 => {
                let ResolvedStatement::Let {
                    binding,
                    value,
                    span,
                    ..
                } = &statements[view_index]
                else {
                    unreachable!()
                };
                let assignment = ResolvedStatement::Assign {
                    binding: binding.clone(),
                    field: None,
                    value: value.clone(),
                    span: *span,
                };
                statements.push(assignment);
            }
            3 => {
                // Retain the old proof but change the actual view operation.
                let ResolvedStatement::Let { value, .. } = &mut statements[view_index] else {
                    unreachable!()
                };
                let ResolvedExprKind::BorrowPlace { operation, .. } = &mut value.kind else {
                    panic!("array view lowers to exact place");
                };
                *operation = DeclarationId::new(crate::byte_ops::BYTES_AS_SLICE_ID);
            }
            _ => {
                // A smaller retained type/proof cannot hide actual array bytes.
                let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
                    unreachable!()
                };
                let ResolvedExprKind::ArrayU8(values) = &mut value.kind else {
                    panic!("literal array");
                };
                values.push(42);
            }
        }
        assert_eq!(
            summary(&program),
            (1, crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES)
        );
        if mutation != 0 {
            assert_eq!(crate::hir::validate(&program).unwrap_err().code, "SPX-H006");
        }
    }
}
