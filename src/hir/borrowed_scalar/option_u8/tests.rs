//! Independent exact metadata, byte-read and case shape controls.
use super::*;
mod metered;
fn ty() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(crate::prelude::OPTION_ID),
        arguments: vec![ResolvedType::U8],
    }
}
fn index() -> DeclarationIndex {
    let mut index = DeclarationIndex::default();
    let mut physical = crate::cleanup_plan::ReplayPhysicalReservation::new(1 << 20);
    index
        .append_option_u8_replay_metered(&mut physical, &mut |_| Ok(()))
        .unwrap();
    index
}
fn read() -> ResolvedFunction {
    let source = r#"module option.helper; @id("option.read") fn read(view:borrow Slice<u8>,at:usize)->i64 {match byte_get(view,at) {Option::Some {value}=>i64_from_u8(value),Option::None {}=>0,}}"#;
    crate::hir::resolve(&crate::check(source, "option-helper.spx").unwrap())
        .unwrap()
        .functions
        .remove(0)
}
fn matched(function: &ResolvedFunction) -> &ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("checked block")
    };
    tail
}
#[test]
fn exact_compiler_option_metadata_requires_parameter_case_field_and_origin_identity() {
    assert!(authenticated(&index(), &ty()));
    assert!(metered::original(&index(), &ty()));
    for mutation in 0..11 {
        let mut index = index();
        let owner = DeclarationId::new(crate::prelude::OPTION_ID);
        match mutation {
            0 => {
                index.type_parameters.remove(&owner);
            }
            1 => {
                index.type_parameters.get_mut(&owner).unwrap()[0].name = "U".into();
            }
            2 => {
                index.type_parameters.get_mut(&owner).unwrap()[0].index = 1;
            }
            3 => {
                let parameter = index.type_parameters[&owner][0].clone();
                index
                    .type_parameters
                    .get_mut(&owner)
                    .unwrap()
                    .push(parameter);
            }
            4 => {
                index.variant_cases.get_mut(&owner).unwrap().swap(0, 1);
            }
            5 => {
                index.variant_cases.get_mut(&owner).unwrap()[1].index = 0;
            }
            6 => {
                index.variant_cases.get_mut(&owner).unwrap()[1].fields[0].ty = ResolvedType::Bytes;
            }
            7 => {
                index
                    .case_fields
                    .get_mut(&DeclarationId::new(crate::prelude::OPTION_SOME_ID))
                    .unwrap()
                    .clear();
            }
            8 => {
                index.declarations.get_mut(&owner).unwrap().identity_origin =
                    IdentityOrigin::Explicit;
            }
            9 => {
                index
                    .declarations
                    .get_mut(&DeclarationId::new(crate::prelude::OPTION_SOME_VALUE_ID))
                    .unwrap()
                    .owner = Some(owner.clone());
            }
            _ => {
                index.variant_cases.get_mut(&owner).unwrap()[0].fields.push(
                    ResolvedFieldDeclaration {
                        id: DeclarationId::new("foreign"),
                        name: "foreign".into(),
                        index: 0,
                        ty: ResolvedType::U8,
                        span: crate::ast::Span::default(),
                    },
                );
            }
        }
        assert!(!authenticated(&index, &ty()), "metadata mutant {mutation}");
        assert_eq!(
            authenticated(&index, &ty()),
            metered::original(&index, &ty())
        );
    }
    for payload in [ResolvedType::I64, ResolvedType::Bytes, ResolvedType::Str] {
        let mut wrong = ty();
        let ResolvedType::Nominal { arguments, .. } = &mut wrong else {
            unreachable!()
        };
        arguments[0] = payload;
        assert!(!is_type(&wrong));
        assert!(!authenticated(&index(), &wrong));
    }
}
#[test]
fn byte_get_elimination_requires_distinct_cases_even_with_distinct_some_bindings() {
    let function = read();
    let expression = matched(&function);
    assert!(super::super::expression(&function, expression, &mut |_| Ok::<_, ()>(())).unwrap());
    let mut changed = expression.clone();
    let ResolvedExprKind::Match { arms, .. } = &mut changed.kind else {
        panic!("checked match")
    };
    let mut second = arms[0].pattern.clone();
    let ResolvedMatchPattern::Variant { fields, .. } = &mut second else {
        unreachable!()
    };
    fields[0].binding.id = ValueId::intrinsic_parameter("different.some", 0);
    fields[0].binding.name = "different".into();
    arms[1].pattern = second;
    assert!(!super::super::expression(&function, &changed, &mut |_| Ok::<_, ()>(())).unwrap());
}
#[test]
fn byte_get_parameter_mode_result_and_option_payload_are_exact() {
    let function = read();
    let expression = matched(&function);
    let ResolvedExprKind::Match { scrutinee, .. } = &expression.kind else {
        panic!("checked match")
    };
    assert!(super::super::expression(&function, scrutinee, &mut |_| Ok::<_, ()>(())).unwrap());
    for mutation in 0..4 {
        let mut changed = (**scrutinee).clone();
        let ResolvedExprKind::Call { args, .. } = &mut changed.kind else {
            panic!("checked byte read")
        };
        match mutation {
            0 => args[0].ownership = OwnershipMode::Own,
            1 => args[1].ty = ResolvedType::I64,
            2 => changed.ownership = OwnershipMode::Own,
            _ => {
                let ResolvedType::Nominal { arguments, .. } = &mut changed.ty else {
                    unreachable!()
                };
                arguments[0] = ResolvedType::Bytes;
            }
        }
        assert!(!super::super::expression(&function, &changed, &mut |_| Ok::<_, ()>(())).unwrap());
    }
}
#[test]
fn canonical_index_factory_creation_through_drop_keeps_exact_and_one_short_ledgers() {
    fn measured(
        work: usize,
        bytes: usize,
    ) -> (
        Result<(usize, usize), crate::diagnostic::Diagnostic>,
        crate::test_allocation_observer::Metrics,
    ) {
        crate::test_allocation_observer::measure(|| {
            let mut remaining = work;
            let mut physical = crate::cleanup_plan::ReplayPhysicalReservation::new(bytes);
            let mut index = DeclarationIndex::default();
            index.append_option_u8_replay_metered(&mut physical, &mut |units| {
                remaining = remaining.checked_sub(units).ok_or_else(|| {
                    crate::diagnostic::Diagnostic::io("SPX-H006", "exact factory work exhausted")
                })?;
                Ok(())
            })?;
            let result = (work - remaining, physical.charged());
            drop(index);
            Ok(result)
        })
    }
    let (wide, allocation) = measured(1 << 20, 1 << 20);
    let (work, bytes) = wide.unwrap();
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.live == 0);
    let (exact, allocation) = measured(work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.live == 0);
    assert!(measured(work - 1, bytes).0.is_err());
    assert!(measured(work, bytes - 1).0.is_err());
}
