//! Exact-source imported overlays. Invariant/class and retained-body transforms
//! keep the complete owning producer; these views confer no import authority.
use super::*;
use crate::ast::program_source::SyntheticProgramView;

pub(in crate::workspace_graph) fn eligible(
    program: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> bool {
    let closed = |declaration: &TypeDeclaration| {
        declaration.invariants().is_empty()
            && !matches!(declaration.kind, TypeDeclarationKind::Class { .. })
    };
    if program.module_uses.is_empty() || !program.types.iter().all(closed) {
        return false;
    }
    // This is fallback selection only. Avoid allocating wrapper classifiers
    // merely to decide whether borrowing is possible. All body-retaining
    // compiler wrappers have type parameters and retain their old receiver.
    program.module_uses.iter().all(|import| {
        let Some(target) = authored.get(import.persistent_id.as_str()) else { return false; };
        target.function.is_none_or(|function| function.type_parameters.is_empty())
            && programs.iter().filter(|provider| provider.module == target.module)
                .all(|provider| provider.types.iter().all(closed))
    })
}
fn refusal() -> Vec<Diagnostic> {
    vec![limit_error("builder_bytes", active_builder_limit())]
}
fn directory<T>(count: usize) -> Result<Vec<T>, Vec<Diagnostic>> {
    let bytes = count.checked_mul(std::mem::size_of::<T>()).ok_or_else(refusal)?;
    reserve_builder_structure(bytes)?;
    let values = Vec::with_capacity(count);
    if values.capacity() != count { return Err(refusal()); }
    Ok(values)
}
fn pay_type(declaration: &TypeDeclaration) -> Result<(), Vec<Diagnostic>> {
    let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
    ast_type_declaration_cost(declaration, &mut cost)?;
    reserve_builder_structure(cost.total)
}
fn pay_signature(function: &Function) -> Result<(), Vec<Diagnostic>> {
    let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
    ast_function_signature_cost(function, &mut cost)?;
    reserve_builder_structure(cost.total)
}

pub(in crate::workspace_graph) fn construct<'a>(
    program: &'a Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    registry: &LayoutRegistry<'_>,
) -> Result<SyntheticProgramView<'a>, Vec<Diagnostic>> {
    reserve_builder_structure(0)?;
    if !eligible(program, authored, programs) {
        return Err(vec![graph_error("SPX-G173", "imported borrowed projection requires the owned route")]);
    }
    let layout = registry.get(program, authored, programs)?;
    let explicit = program.module_uses.iter().filter(|import| import.kind == ModuleUseKind::Type).count();
    let type_count = explicit.checked_add(layout.entries.len()).ok_or_else(refusal)?;
    let mut types = directory::<TypeDeclaration>(type_count)?;
    for import in program.module_uses.iter().filter(|import| import.kind == ModuleUseKind::Type) {
        let target = &authored[import.persistent_id.as_str()];
        let declaration = target.ty.expect("validated type target");
        pay_type(declaration)?;
        let mut copy = declaration.clone();
        copy.name = crate::bounded_output::budgeted_clone(&import.alias);
        super::super::generic_type_import::rewrite_declaration_with_layout(
            &mut copy, target.module, program, programs, Some(&layout),
        )?;
        types.push(copy);
    }
    // Preserve the independent closure's exact order after explicit aliases.
    for entry in &layout.entries {
        pay_type(entry.declaration)?;
        let mut copy = entry.declaration.clone();
        copy.name = crate::bounded_output::budgeted_clone(&entry.alias);
        super::super::generic_type_import::rewrite_declaration_with_layout(
            &mut copy, entry.module, program, programs, Some(&layout),
        )?;
        types.push(copy);
    }
    let all_count = program.types.len().checked_add(types.len()).ok_or_else(refusal)?;
    let mut type_index = directory::<(&str, &TypeDeclaration)>(all_count)?;
    type_index.extend(program.types.iter().chain(&types).map(|declaration| (declaration.name.as_str(), declaration)));
    type_index.sort_unstable_by(|a, b| a.0.cmp(b.0));
    if type_index.windows(2).any(|items| items[0].0 == items[1].0) {
        return Err(vec![graph_error("SPX-G173", "synthetic workspace type-name index is not unique")]);
    }
    let needs_main = !program.functions.iter().any(|function| function.name == "main")
        && !program.module_uses.iter().any(|import| import.kind == ModuleUseKind::Function && import.alias == "main");
    let imports = program.module_uses.iter().filter(|import| import.kind == ModuleUseKind::Function).count();
    let mut functions = directory::<Function>(imports.checked_add(usize::from(needs_main)).ok_or_else(refusal)?)?;
    let mut byte_proofs = import_stub::ByteHelperProofs::new(authored, programs);
    for import in program.module_uses.iter().filter(|import| import.kind == ModuleUseKind::Function) {
        let target = &authored[import.persistent_id.as_str()];
        let original = target.function.expect("validated function target");
        pay_signature(original)?;
        let mut function = import_stub::signature(original);
        function.name = crate::bounded_output::budgeted_clone(&import.alias);
        for param in &mut function.params {
            type_rewrite::rewrite_with_layout(&mut param.ty, target.module, program, programs, Some(&layout))?;
        }
        type_rewrite::rewrite_with_layout(&mut function.return_type, target.module, program, programs, Some(&layout))?;
        function.body = default_expr(&function, &type_index,
            super::super::owned_function_import::admitted_with_layout(program, target, authored, programs, &layout),
            import_stub::byte_helper_authorized(original, target.module, &mut byte_proofs)?,
        )?;
        functions.push(function);
    }
    // Same old rule also recognizes an imported alias named main.
    if needs_main && !functions.iter().any(|function| function.name == "main") {
        reserve_builder_structure(std::mem::size_of::<Expr>())?;
        functions.push(Function {
            stable_id: budgeted_format(format_args!("workspace.synthetic.main.{}", program.module)),
            explicit_id: true, name: crate::bounded_output::budgeted_clone("main"),
            name_span: crate::ast::Span::default(), type_parameters: Vec::new(), params: Vec::new(),
            return_type: Type::I64, effects: Vec::new(), yields: None, follows: None,
            requires: Vec::new(), ensures: Vec::new(),
            body: Expr { kind: ExprKind::Int(0), span: crate::ast::Span::default() },
            span: crate::ast::Span::default(),
        });
    }
    drop(type_index);
    SyntheticProgramView::with_imports(program, types, functions).ok_or_else(refusal)
}

#[cfg(test)]
#[path = "borrowed_imports/tests.rs"]
mod tests;
