//! Complete previous receiving validator body; literals preserved verbatim.
use super::*;
impl HirValidator<'_> {
    pub(super) fn validate_type_original_receiver(
        &self,
        ty: &ResolvedType,
    ) -> Result<(), Diagnostic> {
        crate::hir::owned_text_record::validate_use(ty, &self.program.declarations)?;
        enum Frame<'a> {
            Enter(&'a ResolvedType),
            Finish(&'a ResolvedType),
        }
        // The first leaf and zero-argument Finish stay inline. Only actual
        // generic children require a heap worklist.
        let mut frames = Vec::new();
        let mut inline = Some(Frame::Enter(ty));
        while let Some(frame) = inline.take().or_else(|| frames.pop()) {
            match frame {
                Frame::Enter(
                    ResolvedType::Never
                    | ResolvedType::Unit
                    | ResolvedType::I64
                    | ResolvedType::I32
                    | ResolvedType::Char
                    | ResolvedType::U8
                    | ResolvedType::Usize
                    | ResolvedType::ArrayU8(_)
                    | ResolvedType::F32
                    | ResolvedType::F64
                    | ResolvedType::Bool
                    | ResolvedType::String
                    | ResolvedType::Bytes
                    | ResolvedType::Str
                    | ResolvedType::SliceU8
                    | ResolvedType::StringMap
                    | ResolvedType::OnceFunction
                    | ResolvedType::OnceFunctionI64
                    | ResolvedType::OnceFunctionI64Pair
                    | ResolvedType::MutFunctionI64,
                ) => {}
                Frame::Enter(ty @ ResolvedType::Function { .. }) => {
                    if !super::function_value::is_signature(ty) {
                        return Err(hir_error("invalid function value signature"));
                    }
                }
                Frame::Enter(ResolvedType::TypeParameter { .. }) => {
                    return Err(hir_error(
                        "uninstantiated type parameters are not valid in executable HIR",
                    ));
                }
                Frame::Enter(
                    ty @ ResolvedType::Nominal {
                        declaration,
                        arguments,
                    },
                ) => {
                    let kind = self
                        .program
                        .declarations
                        .declaration(declaration)
                        .map(|item| item.kind)
                        .filter(|kind| {
                            matches!(
                                kind,
                                DeclarationKind::Resource
                                    | DeclarationKind::Record
                                    | DeclarationKind::Class
                                    | DeclarationKind::Variant
                            )
                        })
                        .ok_or_else(|| {
                            hir_error(format!(
                                "nominal type `{declaration}` is not a resolved type declaration"
                            ))
                        })?;
                    let parameters = self
                        .program
                        .declarations
                        .type_parameters(declaration)
                        .ok_or_else(|| {
                            hir_error(format!("nominal type `{declaration}` has no parameters"))
                        })?;
                    if arguments.len() != parameters.len() {
                        return Err(hir_error(format!(
                            "nominal type `{declaration}` has incorrect argument arity"
                        )));
                    }
                    if !arguments.is_empty() {
                        let admitted_owned_record =
                            super::type_reachability::is_flat_owned_byte_record(
                                &self.program.declarations,
                                ty,
                            );
                        let admitted_nested_owned_record =
                            super::type_reachability::is_admitted_nested_owned_byte_record(
                                &self.program.declarations,
                                ty,
                            );
                        let admitted_owned_variant =
                            super::type_reachability::is_admitted_concrete_owned_byte_variant(
                                &self.program.declarations,
                                ty,
                            );
                        let admitted_owned_generic = box_intrinsic::is_type(
                            &self.program.declarations,
                            declaration,
                            arguments,
                        );
                        if !matches!(kind, DeclarationKind::Record | DeclarationKind::Variant)
                            || (!admitted_owned_byte_prelude_instance(declaration, arguments)
                                && !(ResolvedImportResultKind::is_owned_container_type(
                                    ty,
                                    &self.program.declarations,
                                ) && self
                                    .program
                                    .interfaces
                                    .iter()
                                    .flat_map(|i| &i.imports)
                                    .any(|i| {
                                        i.native_rust
                                            && i.result
                                                .kind
                                                .value_type(&self.program.declarations)
                                                .is_ok_and(|actual| &actual == ty)
                                    }))
                                && !crate::hir::workspace_link::native_owner::admitted_ri06_regex_result(self.program, ty)
                                && !admitted_owned_record
                                && !admitted_nested_owned_record
                                && !admitted_owned_variant
                                && !super::super::declared_collection::vector(&self.program.declarations,ty)
                                && !admitted_owned_generic
                                && (arguments.as_slice() != [ResolvedType::U8]
                                    || declaration.as_str() != crate::prelude::OPTION_ID)
                                && arguments.iter().any(|argument| {
                                    !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                                }))
                    {
                        return Err(hir_error(format!(
                            "nominal type `{declaration}` has unsupported generic arguments"
                        )));
                    }
                    }
                    if arguments.is_empty() {
                        inline = Some(Frame::Finish(ty));
                    } else {
                        frames.push(Frame::Finish(ty));
                        for argument in arguments.iter().rev() {
                            frames.push(Frame::Enter(argument));
                        }
                    }
                }
                Frame::Finish(ty) => {
                    self.program.declarations.type_flags(ty).ok_or_else(|| {
                        hir_error(format!(
                            "type `{}` has no semantic facts",
                            ty.identity_key()
                        ))
                    })?;
                }
            }
        }
        Ok(())
    }
}
