//! Complete original import walk with real receiving directory ownership.
//! Ordinary and warm views retain additional spare storage, deliberately.
use super::*;

pub(in crate::workspace_graph) fn construct_receiving<'a>(
    program: &'a Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    registry: &LayoutRegistry<'_>,
) -> Result<SyntheticProgramView<'a>, Vec<Diagnostic>> {
    // Unbounded compatibility callers retain the complete old constructor.
    if crate::bounded_output::active_limit().is_none() {
        return super::construct(program, authored, programs, registry);
    }
    reserve_builder_structure(0)?;
    if !eligible(program, authored, programs) {
        return Err(vec![graph_error(
            "SPX-G173",
            "imported borrowed projection requires the owned route",
        )]);
    }
    let layout = registry.get(program, authored, programs)?;
    let explicit = program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Type)
        .count();
    let type_count = explicit
        .checked_add(layout.entries.len())
        .ok_or_else(refusal)?;
    let recursive = iterative(program, authored);
    if recursive {
        pay_teardown_minimum()?;
    }
    let type_values =
        crate::bounded_output::key_directory::KeyDirectory::allocate(&program.types, type_count)
            .map_err(|()| refusal())?;
    let mut roots =
        crate::ast::program_source::receiving_roots::ReceivingRoots::new(type_values, recursive);
    let types = roots.types.as_mut().expect("type directory acquired");
    for import in program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Type)
    {
        let target = &authored[import.persistent_id.as_str()];
        let declaration = target.ty.expect("validated type target");
        pay_type(declaration)?;
        if recursive {
            let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
            ast_type_declaration_cost(declaration, &mut cost)?;
            pay_teardown(&cost)?;
        }
        types.push(declaration.clone()).map_err(|()| refusal())?;
        let copy = types.last_mut().expect("inserted type");
        copy.name = crate::bounded_output::budgeted_clone(&import.alias);
        super::super::super::generic_type_import::rewrite_declaration_with_layout(
            copy,
            target.module,
            program,
            programs,
            Some(&layout),
        )?;
    }
    // Preserve the independent closure's exact order after explicit aliases.
    for entry in &layout.entries {
        pay_type(entry.declaration)?;
        if recursive {
            let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
            ast_type_declaration_cost(entry.declaration, &mut cost)?;
            pay_teardown(&cost)?;
        }
        types
            .push(entry.declaration.clone())
            .map_err(|()| refusal())?;
        let copy = types.last_mut().expect("inserted private type");
        copy.name = crate::bounded_output::budgeted_clone(&entry.alias);
        super::super::super::generic_type_import::rewrite_declaration_with_layout(
            copy,
            entry.module,
            program,
            programs,
            Some(&layout),
        )?;
    }
    let all_count = program
        .types
        .len()
        .checked_add(types.len())
        .ok_or_else(refusal)?;
    let mut type_index = directory::<(&str, &TypeDeclaration)>(all_count)?;
    type_index.extend(
        program
            .types
            .iter()
            .chain(types.iter())
            .map(|declaration| (declaration.name.as_str(), declaration)),
    );
    type_index.sort_unstable_by(|a, b| a.0.cmp(b.0));
    if type_index.windows(2).any(|items| items[0].0 == items[1].0) {
        return Err(vec![graph_error(
            "SPX-G173",
            "synthetic workspace type-name index is not unique",
        )]);
    }
    let needs_main = !program
        .functions
        .iter()
        .any(|function| function.name == "main")
        && !program
            .module_uses
            .iter()
            .any(|import| import.kind == ModuleUseKind::Function && import.alias == "main");
    let imports = program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Function)
        .count();
    // Keep this physical frontier after the original type-name verdict.
    let generated_count = imports
        .checked_add(usize::from(needs_main))
        .ok_or_else(refusal)?;
    let function_values = crate::bounded_output::key_directory::KeyDirectory::allocate(
        &program.functions,
        generated_count,
    )
    .map_err(|()| refusal())?;
    roots.functions = Some(function_values);
    let functions = roots
        .functions
        .as_mut()
        .expect("function directory acquired");
    let mut byte_proofs = import_stub::ByteHelperProofs::new(authored, programs);
    for import in program
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Function)
    {
        let target = &authored[import.persistent_id.as_str()];
        let original = target.function.expect("validated function target");
        let retain_body = programs
            .iter()
            .find(|provider| provider.module == target.module)
            .is_some_and(|provider| {
                crate::vec_ops::source_wrapper(provider, original).is_some()
                    || crate::box_ops::source_wrapper(provider, original).is_some()
            });
        if retain_body {
            let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
            ast_function_cost(original, &mut cost)?;
            reserve_builder_structure(cost.total)?;
            pay_teardown(&cost)?;
        } else {
            pay_signature(original)?;
            if recursive {
                let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
                ast_function_signature_cost(original, &mut cost)?;
                pay_teardown(&cost)?;
            }
        }
        let mut function = if retain_body {
            original.clone()
        } else {
            import_stub::signature(original)
        };
        function.name = crate::bounded_output::budgeted_clone(&import.alias);
        functions.push(function).map_err(|()| refusal())?;
        if retain_body {
            continue;
        }
        let function = functions.last_mut().expect("inserted signature");
        for param in &mut function.params {
            type_rewrite::rewrite_with_layout(
                &mut param.ty,
                target.module,
                program,
                programs,
                Some(&layout),
            )?;
        }
        type_rewrite::rewrite_with_layout(
            &mut function.return_type,
            target.module,
            program,
            programs,
            Some(&layout),
        )?;
        if recursive {
            pay_default_teardown(&function.return_type, &type_index)?;
        }
        function.body = default_expr(
            function,
            &type_index,
            super::super::super::owned_function_import::admitted_with_layout(
                program, target, authored, programs, &layout,
            ),
            import_stub::byte_helper_authorized(original, target.module, &mut byte_proofs)?,
        )?;
    }
    // Same old rule also recognizes an imported alias named main.
    if needs_main && !functions.iter().any(|function| function.name == "main") {
        reserve_builder_structure(std::mem::size_of::<Expr>())?;
        functions
            .push(Function {
                stable_id: budgeted_format(format_args!(
                    "workspace.synthetic.main.{}",
                    program.module
                )),
                explicit_id: true,
                name: crate::bounded_output::budgeted_clone("main"),
                name_span: crate::ast::Span::default(),
                type_parameters: Vec::new(),
                params: Vec::new(),
                return_type: Type::I64,
                effects: Vec::new(),
                yields: None,
                follows: None,
                requires: Vec::new(),
                ensures: Vec::new(),
                body: Expr {
                    kind: ExprKind::Int(0),
                    span: crate::ast::Span::default(),
                },
                span: crate::ast::Span::default(),
            })
            .map_err(|()| refusal())?;
    }
    drop(type_index);
    SyntheticProgramView::with_receiving_owner(program, roots).ok_or_else(refusal)
}
