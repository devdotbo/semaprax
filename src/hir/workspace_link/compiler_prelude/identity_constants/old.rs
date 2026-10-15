//! Saved owning-identity constructors; complete pre-change index oracle.
use super::*;

pub(super) fn saved_compiler_prelude_declarations_for_vec(
    include_vec: bool,
) -> Result<DeclarationIndex, Diagnostic> {
    let mut declarations = DeclarationIndex::from_verified(&workspace_linker_prelude_program())?;
    if include_vec {
        let id = DeclarationId::new(crate::prelude::VEC_ID);
        declarations.insert_top_level(
            "Vec".to_owned(),
            id.clone(),
            DeclarationKind::Record,
            IdentityOrigin::CompilerOwned,
        );
        declarations.type_parameters.insert(
            id.clone(),
            vec![ResolvedTypeParameterDeclaration {
                name: "T".to_owned(),
                index: 0,
                span: Span::default(),
            }],
        );
        declarations.record_fields.insert(id, Vec::new());
    }
    Ok(declarations)
}

pub(super) fn saved_compiler_prelude_declarations_for(
    include_vec: bool,
    include_box: bool,
) -> Result<DeclarationIndex, Diagnostic> {
    // The Box-bearing v4 prelude is additive over the Vec-bearing predecessor.
    let mut declarations = saved_compiler_prelude_declarations_for_vec(include_vec || include_box)?;
    if include_box {
        let id = DeclarationId::new(crate::prelude::BOX_ID);
        declarations.insert_top_level(
            "Box".to_owned(),
            id.clone(),
            DeclarationKind::Record,
            IdentityOrigin::CompilerOwned,
        );
        declarations.type_parameters.insert(
            id.clone(),
            vec![ResolvedTypeParameterDeclaration {
                name: "T".to_owned(),
                index: 0,
                span: Span::default(),
            }],
        );
        declarations.record_fields.insert(id, Vec::new());
    }
    Ok(declarations)
}
