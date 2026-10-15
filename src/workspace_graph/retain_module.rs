//! Move authenticated module outputs into their retained carriers.
use super::*;

#[allow(
    clippy::too_many_arguments,
    reason = "keep independent held authority and canonical replay inputs explicit"
)]
fn retain_owned_workspace_module(
    program: &Program,
    module: String,
    resolved: hir::ResolvedProgram,
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    mut loan_scratch: Option<&mut loan_retention::RetentionScratch>,
    selected_static_text: bool,
    authenticated_sources: Option<&[WorkspaceSource]>,
) -> Result<(WorkspaceResolvedModule, Vec<hir::ResolvedFunctionInstance>), Vec<Diagnostic>> {
    let static_text = if selected_static_text {
        static_text::retain(program, &resolved, authored, authenticated_sources)?
    } else {
        None
    };
    let scalar_helpers = static_text::scalar_helpers::capture(
        program,
        &resolved,
        programs,
        authored,
        authenticated_sources,
    )?;
    let record_closure = static_text::record_closure::capture(
        program,
        &resolved,
        programs,
        authored,
        authenticated_sources,
    )?;
    let record_source_revision = record_closure.as_ref().map(|facts| facts.revision());
    // Captured while the held source and complete resolved module are present.
    let scalar_lexical = if static_text.is_none() {
        static_text::scalar_lexical::capture(program, &resolved, authored, authenticated_sources)?
    } else {
        None
    };
    let retained_output_only = loan_scratch.is_some();
    hir::replay_agent_source_associations(program, &resolved.agents)
        .map_err(|error| vec![error])?;
    // Declaration facts, then `follows` binding facts, in one `Vec`: the two
    // JSON shapes are distinguished by their own leading key alone
    // (`{"stable_id":...` vs `{"function":...`), so storing them together
    // costs no second per-module `Vec` field -- see the field's own doc
    // comment on `WorkspaceResolvedModule`.
    let mut session_protocol_facts = session_protocol_decl::declaration_facts(program, &resolved)?;
    session_protocol_facts.extend(session_protocol_decl::follows_facts(program)?);
    session_protocol_facts.extend(agent_execution::facts(program)?);
    let types = filter_owned_vec(
        resolved.types,
        |item| {
            authored
                .get(item.id.as_str())
                .is_some_and(|owner| owner.module == program.module)
        },
        retained_output_only,
    )?;
    let functions = filter_owned_vec_accounted(
        loan_retention::prepare_functions(
            resolved.functions,
            loan_scratch.as_deref_mut(),
            program,
            authored,
        )?,
        GRAPH_ACCOUNTED_RESOLVED_FUNCTION_BYTES,
        |function| match loan_scratch.as_deref_mut() {
            Some(scratch) => scratch.measure(function),
            None => retained_function_loan_bytes(function),
        },
        |item| loan_retention::authored_function(item, program, authored),
        retained_output_only,
    )?;
    let function_templates = filter_owned_vec(
        resolved.function_templates,
        |item| {
            authored
                .get(item.id.as_str())
                .is_some_and(|owner| owner.module == program.module)
        },
        retained_output_only,
    )?;
    let (function_instances, imported_instances) = owned_generics::retain_module_instances(
        program,
        programs,
        authored,
        resolved.function_instances,
        loan_scratch,
    )?;
    let signature_types = retained_signature_type_facts(&functions, &resolved.declarations)?;
    let agents = filter_owned_vec(resolved.agents, |_| true, retained_output_only)?;
    Ok((
        WorkspaceResolvedModule {
            path: crate::bounded_output::budgeted_clone(&program.path),
            module,
            permits: resolved.permits,
            agents: agents.into(),
            types: types.into(),
            interfaces: resolved.interfaces,
            functions: functions.into(),
            function_templates: function_templates.into(),
            function_instances,
            signature_types,
            session_protocol_facts,
            native_law: false,
            static_text,
            scalar_helpers,
            record_closure,
            record_source_revision,
            scalar_lexical,
        },
        imported_instances,
    ))
}

#[allow(
    clippy::too_many_arguments,
    reason = "preserve explicit source and receiving authority"
)]
pub(super) fn retain_workspace_module(
    program: &Program,
    module: String,
    resolved: ResolvedModuleInput,
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    loan_scratch: Option<&mut loan_retention::RetentionScratch>,
    selected_static_text: bool,
    authenticated_sources: Option<&[WorkspaceSource]>,
) -> Result<(WorkspaceResolvedModule, Vec<hir::ResolvedFunctionInstance>), Vec<Diagnostic>> {
    match resolved {
        ResolvedModuleInput::Owned(resolved) => retain_owned_workspace_module(
            program,
            module,
            resolved,
            programs,
            authored,
            loan_scratch,
            selected_static_text,
            authenticated_sources,
        ),
        ResolvedModuleInput::Checked(owner) => checked_retention::retain_checked_module(
            program,
            module,
            owner,
            programs,
            authored,
            loan_scratch,
            selected_static_text,
            authenticated_sources,
        ),
    }
}
