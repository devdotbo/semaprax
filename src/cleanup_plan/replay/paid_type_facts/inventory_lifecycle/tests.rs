use super::*;
fn paid<T>(
    program: &ResolvedProgram,
    run: impl FnOnce(&mut Payer<'_>) -> Result<T, Diagnostic>,
) -> T {
    let (result, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let function = &program.functions[0];
        let mut budget = ReplayBudget::new();
        budget
            .charge(function, 0, "full lifecycle bool control")
            .unwrap();
        run(&mut super::super::tests::payer(function, &mut budget)).unwrap()
    });
    assert!(!refused);
    result
}
fn fixtures() -> Vec<ResolvedProgram> {
    let mut programs = Vec::from(super::super::tests::fixtures());
    programs.push(
        crate::hir::resolve(
            &crate::check(
                include_str!("../../../../../tests/fixtures/owned-nested-outcome.spx"),
                "summary-lifecycle.spx",
            )
            .unwrap(),
        )
        .unwrap(),
    );
    programs
}
#[test]
fn complete_step_shape_zero_one_seventeen_and_forged_metadata_match_original_receiver() {
    for program in fixtures() {
        for width in [0, 1, 17] {
            for element in [ResolvedType::I64, ResolvedType::Bytes, ResolvedType::String] {
                let root = ResolvedType::Nominal {
                    declaration: DeclarationId::new(crate::iterator_ops::STEP_ID),
                    arguments: vec![element; width],
                };
                let old = crate::iterator_ops::step_shape(&program.declarations, &root);
                assert_eq!(
                    paid(&program, |payer| step::step_shape(
                        &program.declarations,
                        &root,
                        payer
                    )),
                    old
                );
                for function in &program.functions {
                    assert_eq!(
                        paid(&program, |payer| step::step_shape(
                            &program.declarations,
                            &function.return_type,
                            payer
                        )),
                        crate::iterator_ops::step_shape(
                            &program.declarations,
                            &function.return_type
                        )
                    );
                }
            }
        }
    }
}
#[test]
fn complete_outcome_flat_owned_nested_and_field_membership_match_all_original_metadata() {
    for program in fixtures() {
        for item in &program.types {
            for width in [0, 1, 17] {
                let ty = ResolvedType::Nominal {
                    declaration: item.id.clone(),
                    arguments: vec![ResolvedType::Bytes; width],
                };
                assert_eq!(
                    paid(&program, |payer| outcome::admitted(
                        &program.declarations,
                        &ty,
                        payer
                    )),
                    crate::hir::collection_outcome::runtime_admitted(&program.declarations, &ty)
                );
                assert_eq!(
                    paid(&program, |payer| outcome::field(
                        &program.declarations,
                        &ty,
                        payer
                    )),
                    crate::hir::collection_outcome::field_admitted(&program.declarations, &ty)
                );
            }
            if let crate::hir::ResolvedTypeDeclarationKind::Variant { cases } = &item.kind {
                let container = ResolvedType::Nominal {
                    declaration: item.id.clone(),
                    arguments: vec![],
                };
                for case in cases {
                    for field in &case.fields {
                        assert_eq!(
                            paid(&program, |payer| variants::record(
                                &program, &container, &case.id, &field.id, &field.ty, payer
                            )),
                            crate::cleanup::variant_record_field(
                                &program, &container, &case.id, &field.id, &field.ty
                            )
                        );
                        assert_eq!(
                            paid(&program, |payer| variants::leaf(
                                &program, &container, &case.id, &field.id, &field.ty, payer
                            )),
                            crate::cleanup::variant_leaf_lifecycle(
                                &program, &container, &case.id, &field.id, &field.ty
                            )
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn complete_native_resource_result_receiver_and_exact_borrowed_type_equality_keep_old_authority() {
    for program in fixtures() {
        let mut roots = vec![ResolvedType::Unit, ResolvedType::String];
        for item in &program.types {
            let resource = ResolvedType::Nominal {
                declaration: item.id.clone(),
                arguments: vec![],
            };
            roots.push(ResolvedType::Nominal {
                declaration: DeclarationId::new(crate::prelude::RESULT_ID),
                arguments: vec![resource, ResolvedType::I64],
            });
        }
        for a in &roots {
            assert_eq!(
                paid(&program, |payer| native::result(&program, a, payer)),
                crate::hir::admitted_ri06_regex_result(&program, a)
            );
            for b in &roots {
                assert_eq!(paid(&program, |payer| equal::same(a, b, payer)), a == b);
            }
        }
    }
}

mod native;
