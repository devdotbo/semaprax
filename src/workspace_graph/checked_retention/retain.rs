//! Retain the complete existing checked owner through immutable local views.
use super::*;

#[allow(
    clippy::too_many_arguments,
    reason = "keep current source authority explicit"
)]
pub(super) fn retain_checked_module(
    program: &Program,
    module: String,
    owner: CheckedModuleLease,
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    mut loan_scratch: Option<&mut loan_retention::RetentionScratch>,
    selected_static_text: bool,
    authenticated_sources: Option<&[WorkspaceSource]>,
) -> Result<(WorkspaceResolvedModule, Vec<hir::ResolvedFunctionInstance>), Vec<Diagnostic>> {
    let static_text = if selected_static_text {
        static_text::retain(program, &owner, authored, authenticated_sources)?
    } else {
        None
    };
    let scalar_helpers = static_text::scalar_helpers::capture(
        program,
        &owner,
        programs,
        authored,
        authenticated_sources,
    )?;
    let record_closure = static_text::record_closure::capture(
        program,
        &owner,
        programs,
        authored,
        authenticated_sources,
    )?;
    let record_source_revision = record_closure.as_ref().map(|facts| facts.revision());
    hir::replay_agent_source_associations(program, &owner.agents).map_err(|error| vec![error])?;
    let mut session_protocol_facts = session_protocol_decl::declaration_facts(program, &owner)?;
    session_protocol_facts.extend(session_protocol_decl::follows_facts(program)?);
    session_protocol_facts.extend(agent_execution::facts(program)?);
    let local = |id: &hir::DeclarationId| {
        authored
            .get(id.as_str())
            .is_some_and(|item| item.module == program.module)
    };
    let types = Retained::select(owner.clone(), |p| &p.types, |item| local(&item.id))?;
    let functions = Retained::select(
        owner.clone(),
        |p| &p.functions,
        |item| loan_retention::authored_function(item, program, authored),
    )?;
    let function_templates = Retained::select(
        owner.clone(),
        |p| &p.function_templates,
        |item| local(&item.id),
    )?;
    let agents = Retained::select(owner.clone(), |p| &p.agents, |_| true)?;
    // Preserve every cold-equivalent loan proof debit. Sharing avoids capacity
    // shrink; it does not authorize omitting proof validation or its census.
    if let Some(scratch) = loan_scratch.as_deref_mut() {
        scratch.prepare(functions.iter())?;
    }
    for function in &functions {
        let bytes = match loan_scratch.as_deref_mut() {
            Some(scratch) => scratch.measure(function)?,
            None => retained_function_loan_bytes(function)?,
        };
        reserve_builder_structure(bytes)?;
    }
    // Instances remain a separately owned compatibility overlay, since current
    // imported wrapper attachment mutates this field after exact equality checks.
    let (function_instances, imported_instances) = owned_generics::retain_module_instances(
        program,
        programs,
        authored,
        owner.function_instances.clone(),
        loan_scratch,
    )?;
    let signature_types = retained_signature_type_facts(&functions, &owner.declarations)?;
    Ok((
        WorkspaceResolvedModule {
            path: crate::bounded_output::budgeted_clone(&program.path),
            module,
            permits: owner.permits.clone(),
            interfaces: owner.interfaces.clone(),
            agents,
            types,
            functions,
            function_templates,
            function_instances,
            signature_types,
            session_protocol_facts,
            native_law: false,
            static_text,
            scalar_helpers,
            record_closure,
            record_source_revision,
        },
        imported_instances,
    ))
}
