//! Complete original Option/Result constructors, retained independently.
use super::*;
pub(super) fn option() -> TypeDeclaration {
    TypeDeclaration {
        stable_id: OPTION_ID.to_owned(),
        explicit_id: true,
        name: "Option".to_owned(),
        name_span: Span::default(),
        type_parameters: vec![parameter("T")],
        kind: TypeDeclarationKind::Variant {
            cases: vec![
                case(OPTION_NONE_ID, "None", Vec::new()),
                case(
                    OPTION_SOME_ID,
                    "Some",
                    vec![field(OPTION_SOME_VALUE_ID, "value", parameter_type("T"))],
                ),
            ],
        },
        extends: None,
        invariants: None,
        span: Span::default(),
    }
}

pub(super) fn result() -> TypeDeclaration {
    TypeDeclaration {
        stable_id: RESULT_ID.to_owned(),
        explicit_id: true,
        name: "Result".to_owned(),
        name_span: Span::default(),
        type_parameters: vec![parameter("T"), parameter("E")],
        kind: TypeDeclarationKind::Variant {
            cases: vec![
                case(
                    RESULT_OK_ID,
                    "Ok",
                    vec![field(RESULT_OK_VALUE_ID, "value", parameter_type("T"))],
                ),
                case(
                    RESULT_ERR_ID,
                    "Err",
                    vec![field(RESULT_ERR_ERROR_ID, "error", parameter_type("E"))],
                ),
            ],
        },
        extends: None,
        invariants: None,
        span: Span::default(),
    }
}
