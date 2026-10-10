use super::*;

fn program() -> ResolvedProgram {
    let source = crate::check(
        r#"module replay.inventory_owner;
@id("inventory.read") fn read(input:borrow Bytes)->usize {
 let view=bytes_as_slice(input); byte_len(view)
}
@id("inventory.main") fn main()->i64 {
 let mut data=bytes_zeroed(1usize); let mut at=0;
 while at<2 {data=bytes_set(data,0usize,65u8);at=at+1;if at==1 {continue;};0}
 if read(data)==1usize {42}else{0}
}"#,
        "replay-inventory-owner.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}

#[test]
fn primitive_inventory_matches_legacy_at_exact_and_one_short_reservation() {
    let program = program();
    for function in program
        .functions
        .iter()
        .filter(|f| f.id.as_str().starts_with("inventory."))
    {
        let legacy = build_inventory(&program, function).unwrap();
        let (selected, refused, bytes) =
            crate::bounded_output::with_limit_usage(usize::MAX, || {
                build_inventory_with_reservation(
                    &program,
                    function,
                    &mut ReplayPhysicalReservation::new(usize::MAX),
                )
                .unwrap()
            });
        assert!(!refused);
        assert!(bytes > 0);
        assert_eq!(legacy, selected);
        let (exact, refused, used) = crate::bounded_output::with_limit_usage(bytes, || {
            build_inventory_with_reservation(
                &program,
                function,
                &mut ReplayPhysicalReservation::new(bytes),
            )
            .unwrap()
        });
        assert!(!refused);
        assert_eq!(used, bytes);
        assert_eq!(legacy, exact);
        let (short, refused, used) = crate::bounded_output::with_limit_usage(bytes - 1, || {
            let mut physical = ReplayPhysicalReservation::new(usize::MAX);
            let result = build_inventory_with_reservation(&program, function, &mut physical);
            assert!(physical.reserve(0).is_err());
            result
        });
        assert!(refused);
        assert!(used < bytes);
        assert_eq!(short.unwrap_err().code, "SPX-H006");
    }
}

#[test]
fn unsupported_owned_nominal_refuses_before_constructor_backing() {
    let program = program();
    let mut function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "inventory.read")
        .unwrap()
        .clone();
    function.params[0].ownership = OwnershipMode::Own;
    function.params[0].ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("untrusted.missing.record"),
        arguments: Vec::new(),
    };
    let (result, refused, bytes) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        build_inventory_with_reservation(
            &program,
            &function,
            &mut ReplayPhysicalReservation::new(usize::MAX),
        )
    });
    assert_eq!(result.unwrap_err().code, "SPX-H006");
    assert!(
        !refused,
        "unsupported authority is distinct from quota refusal"
    );
    assert_eq!(
        bytes, 0,
        "no delegated type classifier or inventory backing"
    );
}

#[test]
fn selected_type_boundary_keeps_generic_and_owned_aggregate_shapes_closed() {
    for ty in [
        ResolvedType::Nominal {
            declaration: DeclarationId::new(crate::prelude::VEC_ID),
            arguments: vec![ResolvedType::Bytes],
        },
        ResolvedType::Nominal {
            declaration: DeclarationId::new("application.variant"),
            arguments: Vec::new(),
        },
        ResolvedType::Never,
    ] {
        assert_eq!(selected::needs_drop(&ty).unwrap_err().code, "SPX-H006");
    }
    assert!(selected::needs_drop(&ResolvedType::Bytes).unwrap());
    assert!(selected::needs_drop(&ResolvedType::String).unwrap());
    assert!(!selected::needs_drop(&ResolvedType::SliceU8).unwrap());
    assert!(!selected::needs_drop(&ResolvedType::Str).unwrap());
}
