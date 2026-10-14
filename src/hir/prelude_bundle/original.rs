//! Complete independent pre-schema base constructors and exact helper bodies.
use crate::ast::{
    FieldDeclaration, Span, Type, TypeDeclaration, TypeDeclarationKind, TypeParameterDeclaration,
    VariantCaseDeclaration,
};
use crate::prelude::{
    OPTION_ID, OPTION_NONE_ID, OPTION_SOME_ID, OPTION_SOME_VALUE_ID, RESULT_ERR_ERROR_ID,
    RESULT_ERR_ID, RESULT_ID, RESULT_OK_ID, RESULT_OK_VALUE_ID,
};
fn parameter(name: &str) -> TypeParameterDeclaration {
    TypeParameterDeclaration {
        name: name.to_owned(),
        span: Span::default(),
    }
}

fn parameter_type(name: &str) -> Type {
    Type::Named {
        name: name.to_owned(),
        arguments: Vec::new(),
    }
}

fn field(id: &str, name: &str, ty: Type) -> FieldDeclaration {
    FieldDeclaration {
        stable_id: id.to_owned(),
        explicit_id: true,
        name: name.to_owned(),
        name_span: Span::default(),
        ty,
        span: Span::default(),
    }
}

fn case(id: &str, name: &str, fields: Vec<FieldDeclaration>) -> VariantCaseDeclaration {
    VariantCaseDeclaration {
        stable_id: id.to_owned(),
        explicit_id: true,
        name: name.to_owned(),
        name_span: Span::default(),
        fields,
        span: Span::default(),
    }
}

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
