//! Borrowed construction metadata for the two unconditional base declarations.
//! Forecast consumers borrow this schema without initializing the process table.
use super::*;

pub(crate) struct Variant {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) parameters: &'static [&'static str],
    pub(crate) cases: &'static [Case],
}
pub(crate) struct Case {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) fields: &'static [Field],
}
pub(crate) struct Field {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) parameter: &'static str,
}
pub(crate) const OPTION: Variant = Variant {
    id: OPTION_ID,
    name: "Option",
    parameters: &["T"],
    cases: &[
        Case {
            id: OPTION_NONE_ID,
            name: "None",
            fields: &[],
        },
        Case {
            id: OPTION_SOME_ID,
            name: "Some",
            fields: &[Field {
                id: OPTION_SOME_VALUE_ID,
                name: "value",
                parameter: "T",
            }],
        },
    ],
};
pub(crate) const RESULT: Variant = Variant {
    id: RESULT_ID,
    name: "Result",
    parameters: &["T", "E"],
    cases: &[
        Case {
            id: RESULT_OK_ID,
            name: "Ok",
            fields: &[Field {
                id: RESULT_OK_VALUE_ID,
                name: "value",
                parameter: "T",
            }],
        },
        Case {
            id: RESULT_ERR_ID,
            name: "Err",
            fields: &[Field {
                id: RESULT_ERR_ERROR_ID,
                name: "error",
                parameter: "E",
            }],
        },
    ],
};
/// `declarations_for_program` always selects this ordered prefix, including
/// when authored names overlap. Neither index nor resolver filters it by use.
pub(crate) const BASE: [&Variant; 2] = [&OPTION, &RESULT];

pub(super) fn declaration(schema: &Variant) -> TypeDeclaration {
    TypeDeclaration {
        stable_id: schema.id.to_owned(),
        explicit_id: true,
        name: schema.name.to_owned(),
        name_span: Span::default(),
        type_parameters: schema
            .parameters
            .iter()
            .map(|name| parameter(name))
            .collect(),
        kind: TypeDeclarationKind::Variant {
            cases: schema
                .cases
                .iter()
                .map(|schema| {
                    case(
                        schema.id,
                        schema.name,
                        schema
                            .fields
                            .iter()
                            .map(|schema| {
                                field(schema.id, schema.name, parameter_type(schema.parameter))
                            })
                            .collect(),
                    )
                })
                .collect(),
        },
        extends: None,
        invariants: None,
        span: Span::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn base_metadata_preserves_complete_original_constructors_and_physical_drop() {
        for (schema, original) in [
            (
                &OPTION,
                super::super::base_original::option as fn() -> TypeDeclaration,
            ),
            (
                &RESULT,
                super::super::base_original::result as fn() -> TypeDeclaration,
            ),
        ] {
            assert_eq!(declaration(schema), original());
            let (_, saved) = crate::test_allocation_observer::measure(|| drop(original()));
            let (_, current) =
                crate::test_allocation_observer::measure(|| drop(declaration(schema)));
            assert_eq!(current, saved);
            assert!(!current.invalid);
            assert_eq!(current.live, 0);
            assert_eq!(current.allocated, current.freed);
        }
    }
}
