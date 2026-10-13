//! Every saved owned operation payload, including nested generic arguments.
use super::*;
#[test]
fn arena_all_operation_kinds_zero_one_seventeen_keep_complete_owned_oracle() {
    for count in [0, 1, 17] {
        let args = || {
            (0..count)
                .map(|index| scalar(&format!("payload.argument.{index}")))
                .collect()
        };
        let nominal = ResolvedType::Nominal {
            declaration: DeclarationId::new("payload.nominal"),
            arguments: vec![ResolvedType::Nominal {
                declaration: DeclarationId::new("payload.inner"),
                arguments: vec![ResolvedType::U8],
            }],
        };
        let instance = FunctionInstanceId::derive(
            &DeclarationId::new("payload.generic"),
            &vec![nominal.clone(); count],
        );
        let cases = [
            ResolvedExprKind::Call {
                callee: DeclarationId::new("payload.generic"),
                instance: Some(instance),
                args: args(),
                type_arguments: vec![nominal; count],
            },
            ResolvedExprKind::Invoke {
                callable: Box::new(scalar("payload.callable")),
                args: args(),
            },
            ResolvedExprKind::ByteRange {
                operation: DeclarationId::new("core.byte.range"),
                source: Box::new(scalar("payload.source")),
                start: Box::new(scalar("payload.start")),
                end: Box::new(scalar("payload.end")),
            },
            ResolvedExprKind::VecFieldRead {
                element: ResolvedType::I64,
                field: DeclarationId::new("payload.field"),
                bytes: false,
                args: args(),
            },
            ResolvedExprKind::LiteralFormat {
                template: "{} {}".into(),
                args: args(),
            },
            ResolvedExprKind::NativeRustImportCall(crate::hir::ResolvedNativeRustImportCall {
                expression: ExpressionId::from_owned("payload.body".into()),
                import: DeclarationId::new("payload.import"),
                args: args(),
                result: crate::hir::ResolvedImportResultKind::OwnedString,
            }),
            ResolvedExprKind::HostCommandCall(crate::hir::ResolvedHostCommandCall {
                expression: ExpressionId::from_owned("payload.body".into()),
                operation: crate::hir::ResolvedHostCommandOperation::ArgsLen,
                args: args(),
            }),
        ];
        for kind in cases {
            let mut function = function(kind);
            if matches!(
                function.body.kind,
                ResolvedExprKind::NativeRustImportCall(_)
            ) {
                function.body.ownership = OwnershipMode::Own;
            }
            let saved = old::expression_facts(&function).unwrap();
            let current = crate::cleanup_plan::replay::expression_facts::arena::build::<
                crate::cleanup_plan::replay::expression_facts::errors::Legacy,
            >(
                &function,
                capacity::Payer::Selected(&mut PhysicalReservation::new(usize::MAX)),
                &mut ReplayBudget::unobserved(),
            )
            .unwrap();
            assert_eq!(saved.len(), current.len());
            for ((old_key, old), (key, current)) in saved.iter().zip(current.entries()) {
                assert_eq!(old_key, key);
                match (old, current) {
                    (None, None) => {}
                    (Some(old), Some(current)) => {
                        assert_eq!(&old.callee, current.callee.as_ref());
                        assert_eq!(old.instance.as_ref(), current.instance);
                        assert!(old.arguments.iter().eq(current.arguments.iter()));
                        assert_eq!(
                            old.type_arguments.as_slice(),
                            current.type_arguments.as_slice()
                        );
                    }
                    _ => panic!("complete saved fact differs"),
                }
            }
            assert!(current.balanced());
        }
    }
}
