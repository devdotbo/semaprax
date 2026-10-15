//! Read the exact closed native container result kinds without temporary Types.
use crate::ast::{program_source::ProgramSource, ImportResult};

pub(super) fn flags(program: &dyn ProgramSource) -> [bool; 3] {
    [
        ImportResult::OwnedOptionString,
        ImportResult::OwnedResultStringI64,
        ImportResult::OwnedResultStringOptionI64,
    ]
    .map(|kind| {
        program
            .interfaces()
            .iter()
            .flat_map(|interface| &interface.imports)
            .any(|import| import.native_rust && import.result == kind)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ImportDeclaration, ImportFailure, InterfaceDeclaration, Program};
    use crate::bounded_output::{
        active_refused, reserve_active_required, with_limit_usage_refusal,
    };
    use crate::test_allocation_observer::measure;

    // Complete original flag constructor, independent of the new query.
    fn owned_flags(program: &dyn ProgramSource) -> [bool; 3] {
        [
            crate::ast::ImportResult::OwnedOptionString,
            crate::ast::ImportResult::OwnedResultStringI64,
            crate::ast::ImportResult::OwnedResultStringOptionI64,
        ]
        .map(|kind| crate::ast::ImportResult::container_enabled(program, &kind.value_type()))
    }

    fn source(results: &[(bool, ImportResult)]) -> Program {
        let mut program = crate::parse(
            "module native.flags; @id(\"main\") fn main()->i64 {0}",
            "native-flags.spx",
        )
        .unwrap();
        let span = program.functions[0].span;
        for (ordinal, (native_rust, result)) in results.iter().enumerate() {
            program.interfaces.push(InterfaceDeclaration {
                stable_id: format!("host.{ordinal}"),
                explicit_id: true,
                name: format!("Host{ordinal}"),
                name_span: span,
                permits: Vec::new(),
                imports: vec![ImportDeclaration {
                    stable_id: format!("host.{ordinal}.constructor"),
                    explicit_id: true,
                    name: "constructor".to_owned(),
                    name_span: span,
                    native_rust: *native_rust,
                    index_selected: false,
                    selected_signature: None,
                    selected_index_digest: None,
                    selected_receiver: None,
                    rust_path: None,
                    params: Vec::new(),
                    result: result.clone(),
                    effects: Vec::new(),
                    failure: ImportFailure::Infallible,
                    consumes: String::new(),
                    consumes_span: span,
                    span,
                }],
                span,
            });
        }
        program
    }

    #[test]
    fn borrowed_flags_preserve_complete_owned_constructor_and_native_only_admission() {
        let kinds = [
            ImportResult::Unit,
            ImportResult::I64,
            ImportResult::Bool,
            ImportResult::ResultI64I64,
            ImportResult::OwnedString,
            ImportResult::OwnedOptionString,
            ImportResult::OwnedResultStringI64,
            ImportResult::OwnedResultStringOptionI64,
            ImportResult::OwnedResource {
                name: "Resource".to_owned(),
            },
            ImportResult::OwnedResultResourceI64 {
                name: "Resource".to_owned(),
            },
            ImportResult::BorrowedStr {
                owner: "Resource".to_owned(),
            },
        ];
        let empty = source(&[]);
        assert_eq!(flags(&empty), [false; 3]);
        assert_eq!(flags(&empty), owned_flags(&empty));
        for kind in kinds {
            for native in [false, true] {
                let program = source(&[(native, kind.clone())]);
                assert_eq!(flags(&program), owned_flags(&program));
                assert_eq!(
                    flags(&program),
                    [
                        native && kind == ImportResult::OwnedOptionString,
                        native && kind == ImportResult::OwnedResultStringI64,
                        native && kind == ImportResult::OwnedResultStringOptionI64,
                    ]
                );
            }
        }
        let all = source(&[
            (false, ImportResult::OwnedOptionString),
            (true, ImportResult::OwnedResultStringOptionI64),
            (true, ImportResult::OwnedResultStringI64),
            (true, ImportResult::OwnedOptionString),
            (true, ImportResult::OwnedOptionString),
        ]);
        assert_eq!(flags(&all), [true; 3]);
        assert_eq!(flags(&all), owned_flags(&all));
    }

    #[test]
    fn borrowed_flags_remove_only_real_discarded_type_backing_through_drop() {
        let program = source(&[(true, ImportResult::OwnedResultStringOptionI64)]);
        let (old, original) = measure(|| owned_flags(&program));
        assert!(!original.invalid && original.live == 0 && original.allocated > 0);
        assert_eq!(original.allocated, original.freed);
        let (new, current) = measure(|| flags(&program));
        assert_eq!(new, old);
        assert!(!current.invalid && current.live == 0 && current.allocated == 0);
        assert_eq!(current.allocations, 0);
        let ((value, metrics), refused, used, first) =
            with_limit_usage_refusal(0, || measure(|| flags(&program)));
        assert_eq!(value, old);
        assert!(!refused && used == 0 && first.is_none() && metrics.allocated == 0);
        let ((value, metrics), refused, used, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                assert!(!reserve_active_required(1));
                let value = flags(&program);
                assert!(active_refused());
                value
            })
        });
        assert_eq!(value, old);
        assert!(refused && used == 0 && first.is_some() && metrics.allocated == 0);
    }
}
