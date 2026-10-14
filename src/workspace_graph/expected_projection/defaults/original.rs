//! Complete original recursive default receiver, independently compiled.
use super::{
    collection_constructor, graph_error, reserve_builder_structure, Diagnostic, Expr, ExprKind,
    FieldInitializer, Span, Type, TypeDeclaration, TypeDeclarationKind,
};

pub(in crate::workspace_graph::expected_projection) fn default_expr(
    ty: &Type,
    declarations: &[(&str, &TypeDeclaration)],
    allow_owned: bool,
) -> Result<Expr, Vec<Diagnostic>> {
    reserve_builder_structure(std::mem::size_of::<Expr>())?;
    let span = Span::default();
    let kind = match ty {
        Type::Never => {
            return Err(vec![graph_error(
                "SPX-G172",
                "compiler-only completion marker has no default value",
            )])
        }
        ty if allow_owned
            && super::type_rewrite::default_vector_element(ty, declarations).is_some() =>
        {
            let element = super::type_rewrite::default_vector_element(ty, declarations)
                .expect("closed vector default");
            let mut cost = StructuralCost::new();
            super::declaration_cost::ast_type_cost(element, &mut cost)?;
            reserve_builder_structure(checked_builder_sum(
                cost.total,
                std::mem::size_of::<Expr>() + crate::vec_ops::WITH_CAPACITY_NAME.len(),
            )?)?;
            ExprKind::Call {
                name: crate::bounded_output::budgeted_clone(crate::vec_ops::WITH_CAPACITY_NAME),
                type_arguments: vec![element.clone()],
                args: vec![Expr {
                    kind: ExprKind::Usize(0),
                    span,
                }],
            }
        }
        ty if allow_owned && crate::map_ops::ast_collection(ty) => {
            let (name, arguments) = collection_constructor(ty);
            reserve_builder_structure(
                std::mem::size_of::<Expr>() + std::mem::size_of_val(arguments),
            )?;
            ExprKind::Call {
                name: crate::bounded_output::budgeted_clone(name),
                type_arguments: arguments.to_vec(),
                args: vec![Expr {
                    kind: ExprKind::Usize(0),
                    span,
                }],
            }
        }
        Type::I64 => ExprKind::Int(0),
        Type::I32 => ExprKind::Int32(0),
        Type::Char => ExprKind::Char(0),
        Type::U8 => ExprKind::Uint8(0),
        Type::Usize => ExprKind::Usize(0),
        Type::F32 => ExprKind::Float32(0),
        Type::F64 => ExprKind::Float64(0),
        Type::Bool => ExprKind::Bool(false),
        Type::String => ExprKind::String(String::new()),
        Type::Str => {
            return Err(vec![graph_error(
                "SPX-G173",
                "borrowed `str` has no synthesizable workspace default",
            )]);
        }
        Type::SliceU8 | Type::StringMap => {
            return Err(vec![graph_error(
                "SPX-G173",
                "borrowed `Slice<u8>` has no synthesizable workspace default",
            )]);
        }
        Type::Bytes if allow_owned => {
            reserve_builder_structure(std::mem::size_of::<Expr>())?;
            ExprKind::Call {
                name: crate::bounded_output::budgeted_clone(crate::byte_ops::ZEROED_NAME),
                type_arguments: Vec::new(),
                args: vec![Expr {
                    kind: ExprKind::Usize(0),
                    span,
                }],
            }
        }
        Type::ArrayU8(_)
        | Type::Bytes
        | Type::OnceFunction
        | Type::OnceFunctionI64
        | Type::OnceFunctionI64Pair
        | Type::MutFunctionI64
        | Type::Function { .. } => {
            return Err(vec![graph_error(
                "SPX-G173",
                "internal byte-data types have no synthesizable workspace default",
            )]);
        }
        Type::Named { name, arguments } if arguments.is_empty() => {
            let declaration = declarations
                .binary_search_by_key(&name.as_str(), |(name, _)| *name)
                .map(|index| declarations[index].1)
                .map_err(|_| {
                    vec![graph_error(
                        "SPX-G173",
                        "default imported type lookup disagrees",
                    )]
                })?;
            match &declaration.kind {
                TypeDeclarationKind::Record { fields }
                | TypeDeclarationKind::Class { fields, .. } => ExprKind::ConstructRecord {
                    type_name: crate::bounded_output::budgeted_clone(name),
                    type_span: span,
                    type_arguments: Vec::new(),
                    fields: fields
                        .iter()
                        .map(|field| {
                            reserve_builder_structure(std::mem::size_of::<FieldInitializer>())?;
                            Ok(FieldInitializer {
                                name: crate::bounded_output::budgeted_clone(&field.name),
                                name_span: span,
                                value: default_expr(&field.ty, declarations, allow_owned)?,
                                span,
                            })
                        })
                        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?,
                },
                TypeDeclarationKind::Variant { cases } => {
                    let case = cases.first().ok_or_else(|| {
                        vec![graph_error("SPX-G172", "imported Copy variant has no case")]
                    })?;
                    ExprKind::ConstructVariant {
                        type_name: crate::bounded_output::budgeted_clone(name),
                        type_span: span,
                        type_arguments: Vec::new(),
                        case_name: crate::bounded_output::budgeted_clone(&case.name),
                        case_span: span,
                        fields: case
                            .fields
                            .iter()
                            .map(|field| {
                                reserve_builder_structure(std::mem::size_of::<FieldInitializer>())?;
                                Ok(FieldInitializer {
                                    name: crate::bounded_output::budgeted_clone(&field.name),
                                    name_span: span,
                                    value: default_expr(&field.ty, declarations, allow_owned)?,
                                    span,
                                })
                            })
                            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?,
                    }
                }
                TypeDeclarationKind::Resource { .. } => {
                    return Err(vec![graph_error(
                        "SPX-G172",
                        "resource return is not admitted",
                    )])
                }
            }
        }
        Type::Named { .. } => {
            return Err(vec![graph_error(
                "SPX-G172",
                "generic return is not admitted",
            )])
        }
    };
    Ok(Expr { kind, span })
}
