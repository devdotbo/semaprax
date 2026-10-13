//! Source-authored agent fixture, saved pre-migration builder and request
//! census shared by the agent-identity controls. Test-only and unrun; nothing
//! here supplies authority to the production builder.
use super::*;
use crate::ast::Program;
use crate::bounded_output::identity_reservation::IdentityReservation;
use crate::bounded_output::{active_remaining, with_limit_usage};
use std::mem::size_of;

/// Metadata-only scalar Agent fixture; no provider or tool dispatch.
fn nonclaims() -> &'static str {
    r#"["no_compiler_determinism_from_model_output","no_model_output_authority","no_provider_identity_provenance_or_quality_truth","no_secret_input_or_secret_leakage_guarantee_for_caller_supplied_content","no_credential_prompt_state_trace_or_diagnostic_exposure","no_ambient_network_filesystem_process_home_or_environment_authority","no_write_apply_mutation_or_target_execution_tool_authority","no_capability_minting_delegation_or_self_approval","no_human_approval_ui_or_policy","no_semantic_prompt_injection_proof","no_forced_cancellation_or_preemption","no_exactly_once_provider_billing_or_retry","no_durable_memory_persistence_recovery_or_resume","no_crash_reboot_or_power_loss_durability","no_distributed_or_parallel_execution","no_model_quality_accuracy_or_completion_guarantee","no_live_price_or_cost_accuracy_guarantee","no_reusable_authorization_token","no_signature_attestation_or_authenticated_provenance","no_wallet_payment_signing_asset_or_economic_authority","no_privacy_compliance_or_data_residency_guarantee","no_general_formal_proof","no_new_language_graph_cleanup_backend_or_runtime_semantics","no_current_schema_api_or_kat_modification"]"#
}

fn profile(agent_id: &str) -> String {
    concat!(
        "{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"AGENT\",",
        "\"models\":[{\"provider_id\":\"fake.local\",\"model_id\":\"fake-basic\",",
        "\"locality\":\"local\",\"quality_tier\":\"basic\",\"tokenizer_id\":\"fake.bytes-v1\",",
        "\"max_context_tokens\":4096,\"input_usd_microunits_per_million_tokens\":0,",
        "\"output_usd_microunits_per_million_tokens\":0,\"capabilities\":[\"text\"]}],",
        "\"tools\":[{\"tool_id\":\"fixture.read\",\"description\":\"Read.\",",
        "\"arguments_schema\":{\"type\":\"object\",\"fields\":[],\"additional_properties\":false},",
        "\"result_schema\":{\"type\":\"object\",\"fields\":[],\"additional_properties\":false},",
        "\"effects\":[\"read\"],\"required_capabilities\":[\"tool.read\"]}],",
        "\"policy\":{\"allowed_provider_ids\":[\"fake.local\"],\"allowed_model_ids\":[\"fake-basic\"],",
        "\"required_locality\":\"local_only\",\"minimum_quality_tier\":\"basic\",",
        "\"required_model_capabilities\":[\"text\"],\"granted_capabilities\":[\"tool.read\"],",
        "\"allowed_tool_ids\":[\"fixture.read\"]},",
        "\"limits\":{\"max_turns\":2,\"max_provider_attempts\":2,\"max_retries_per_turn\":1,",
        "\"max_concurrency\":1,\"max_elapsed_ms\":1000,\"max_provider_request_bytes\":65536,",
        "\"max_provider_response_bytes\":4096,\"max_stream_chunks\":64,",
        "\"max_total_provider_input_bytes\":131072,\"max_total_provider_output_bytes\":8192,",
        "\"max_reported_model_input_tokens\":131072,\"max_reported_model_output_tokens\":8192,",
        "\"max_usd_microunits\":0,\"max_tool_calls\":1,\"max_tool_arguments_bytes\":4096,",
        "\"max_tool_result_bytes\":4096,\"max_total_tool_bytes\":8192,",
        "\"max_retained_state_bytes\":131072,\"max_trace_events\":64,\"max_trace_bytes\":131072,",
        "\"max_evidence_bytes\":262144,\"max_builder_bytes\":1048576},\"nonclaims\":NONCLAIMS}\n"
    )
    .replace("AGENT", agent_id)
    .replace("NONCLAIMS", nonclaims())
}

fn source(embedded: bool, wait: bool) -> String {
    let profile = profile("fixture.agent");
    let runtime = profile
        .strip_prefix(
            "{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"fixture.agent\",",
        )
        .unwrap()
        .split_once(",\"nonclaims\":")
        .unwrap()
        .0;
    let runtime = format!("{{{runtime}}}");
    let mut text =
        String::from("module fixture.app;\n@id(\"fixture.agent\") agent FixtureAgent { types {\n");
    for role in [
        "task",
        "state",
        "observation",
        "proposal",
        "outcome",
        "result",
    ] {
        text.push_str(&format!(
            "@id(\"fixture.agent.type.{role}\") type {role};\n"
        ));
    }
    text.push_str("} operations {\n");
    for role in [
        "initialize",
        "observe",
        "propose",
        "authorize",
        "execute",
        "reduce",
    ] {
        text.push_str(&format!("@id(\"fixture.agent.fn.{role}\") "));
        match role {
            "propose" => text.push_str("model fn propose;\n"),
            "execute" => text.push_str("effect fn execute;\n"),
            _ if embedded => text.push_str(&format!("fn {role}(value:i64)->i64 {{ value + 1 }}\n")),
            _ => text.push_str(&format!("fn {role};\n")),
        }
    }
    text.push_str("}\n");
    if wait {
        text.push_str("model_wait_v1 { propose = \"fixture.wait\"; }\n");
    }
    text.push_str(&format!(
        "runtime_v1 {{ canonical_json {}; }} }}\n",
        serde_json::to_string(&runtime).unwrap()
    ));
    if !embedded {
        for role in ["initialize", "observe", "authorize", "reduce"] {
            text.push_str(&format!(
                "@id(\"fixture.agent.fn.{role}\") fn {role}(value:i64)->i64 {{ value + 1 }}\n"
            ));
        }
    }
    // Scalar helpers intentionally carry no model-wait runtime/owned-State claim.
    text.push_str("@id(\"fixture.wait\") fn wait(value:i64)->i64 {value}\n@id(\"fixture.wait.other\") fn other_wait(value:i64)->i64 {value}\n@id(\"fixture.main\") fn main()->i64 {observe(41)}\n@id(\"fixture.public\") fn published()->i64 {0}\n");
    text
}

/// One generous ledger for dry runs; refusal limits are derived from it.
pub(in crate::hir) const BIG: usize = 64 * 1024 * 1024;

pub(in crate::hir) fn agent_program() -> Program {
    crate::check(&source(false, true), "agent-identity.spx").expect("agent identity fixture checks")
}

pub(in crate::hir) fn agents(program: &Program) -> &[AgentDeclaration] {
    crate::ast::program_source::ProgramSource::agents(program)
}

pub(in crate::hir) fn module_name(program: &Program) -> &str {
    crate::ast::program_source::ProgramSource::module(program)
}

/// Every identity text the builder requests, in original request order.
pub(in crate::hir) fn agent_texts(declaration: &AgentDeclaration) -> Vec<&str> {
    let mut texts = vec![declaration.stable_id.as_str()];
    texts.extend(declaration.types.iter().map(|role| role.stable_id.as_str()));
    texts.extend(
        declaration
            .operations
            .iter()
            .map(|operation| operation.stable_id.as_str()),
    );
    texts.extend(
        declaration
            .model_wait
            .iter()
            .map(|binding| binding.helper_id.as_str()),
    );
    texts
}

/// Independent shared-backing request size: two counters plus text, padded.
pub(in crate::hir) fn backing(len: usize) -> usize {
    let word = size_of::<usize>();
    (2 * word + len).div_ceil(word) * word
}

/// Independently saved pre-migration builder (legacy temporary-String ids).
pub(in crate::hir) fn saved_original(
    declaration: &AgentDeclaration,
    module: &str,
) -> ResolvedAgentDeclaration {
    ResolvedAgentDeclaration {
        stable_id: DeclarationId::new(declaration.stable_id.clone()),
        name: declaration.name.clone(),
        types: declaration
            .types
            .iter()
            .map(|role| ResolvedAgentTypeRole {
                role: match role.role {
                    crate::ast::AgentTypeRole::Task => ResolvedAgentTypeRoleKind::Task,
                    crate::ast::AgentTypeRole::State => ResolvedAgentTypeRoleKind::State,
                    crate::ast::AgentTypeRole::Observation => {
                        ResolvedAgentTypeRoleKind::Observation
                    }
                    crate::ast::AgentTypeRole::Proposal => ResolvedAgentTypeRoleKind::Proposal,
                    crate::ast::AgentTypeRole::Outcome => ResolvedAgentTypeRoleKind::Outcome,
                    crate::ast::AgentTypeRole::Result => ResolvedAgentTypeRoleKind::Result,
                },
                stable_id: DeclarationId::new(role.stable_id.clone()),
            })
            .collect(),
        operations: declaration
            .operations
            .iter()
            .map(|operation| ResolvedAgentOperationRole {
                role: match operation.role {
                    crate::ast::AgentOperationRole::Initialize => {
                        ResolvedAgentOperationRoleKind::Initialize
                    }
                    crate::ast::AgentOperationRole::Observe => {
                        ResolvedAgentOperationRoleKind::Observe
                    }
                    crate::ast::AgentOperationRole::Propose => {
                        ResolvedAgentOperationRoleKind::Propose
                    }
                    crate::ast::AgentOperationRole::Authorize => {
                        ResolvedAgentOperationRoleKind::Authorize
                    }
                    crate::ast::AgentOperationRole::Execute => {
                        ResolvedAgentOperationRoleKind::Execute
                    }
                    crate::ast::AgentOperationRole::Reduce => {
                        ResolvedAgentOperationRoleKind::Reduce
                    }
                },
                kind: match operation.kind {
                    crate::ast::AgentOperationKind::Deterministic => {
                        ResolvedAgentOperationKind::Deterministic
                    }
                    crate::ast::AgentOperationKind::Model => ResolvedAgentOperationKind::Model,
                    crate::ast::AgentOperationKind::Effect => ResolvedAgentOperationKind::Effect,
                },
                stable_id: DeclarationId::new(operation.stable_id.clone()),
                embedded: operation.embedded_function_index.is_some(),
            })
            .collect(),
        source_association: None,
        runtime_v1_json: declaration.runtime_v1_json.clone(),
        model_wait: declaration.model_wait.as_ref().map(|binding| {
            Box::new(ResolvedAgentModelWaitBinding {
                helper_id: DeclarationId::new(binding.helper_id.clone()),
            })
        }),
    }
    .bind_source_association(module)
}

pub(in crate::hir) struct Census {
    /// Requested bytes of every identity request, in order.
    pub requests: Vec<usize>,
    /// Remaining ledger bytes immediately before each request.
    pub before: Vec<usize>,
    /// Index of the first agent request (agents are the final requests).
    pub agent_start: usize,
    /// Total ledger usage of the whole dry run, including setup.
    pub used: usize,
}

/// Dry run on a generous ledger through the private explicit resolver.
pub(in crate::hir) fn census(program: &Program) -> Census {
    let ((requests, before), failed, used) = with_limit_usage(BIG, || {
        let mut owner = IdentityReservation::acquire().expect("setup fits");
        let mut requests = Vec::new();
        let mut before = Vec::new();
        crate::hir::identity_resolution::resolve(program, None, |bytes| {
            before.push(active_remaining().expect("active ledger"));
            requests.push(bytes);
            owner.reserve(bytes)
        })
        .expect("census resolution");
        (requests, before)
    });
    assert!(!failed);
    let agent_requests: usize = agents(program)
        .iter()
        .map(|declaration| agent_texts(declaration).len())
        .sum();
    let agent_start = requests
        .len()
        .checked_sub(agent_requests)
        .expect("agent requests are a suffix of the census");
    Census {
        requests,
        before,
        agent_start,
        used,
    }
}

/// Invoke the complete frozen receiver, independent of the generic callback.
pub(in crate::hir) fn original_resolve(
    program: &Program,
) -> Result<
    (
        crate::hir::ResolvedProgram,
        crate::hir::FunctionResolutionWork,
    ),
    Diagnostic,
> {
    super::super::agent_identity_original::resolve_verified_original(program, None)
}

/// Reference report produced by the existing prepaid owner on its own ledger.
/// No test diagnostic is invented, and the live refusal still publishes its
/// own existing owner without substituting this independent reference.
pub(in crate::hir) fn refusal_report() -> Vec<Diagnostic> {
    let (report, failed, used) =
        with_limit_usage(IdentityReservation::setup_bytes().unwrap(), || {
            let mut owner = IdentityReservation::acquire().unwrap();
            assert!(owner.reserve(1).is_err());
            owner.publish()
        });
    assert!(failed);
    assert_eq!(used, IdentityReservation::setup_bytes().unwrap());
    report
}

/// A checked structural invariant added to the checked scalar Agent fixture.
pub(in crate::hir) fn invariant_agent_program() -> Program {
    let mut program = agent_program();
    let extra = crate::check("module invariant.agent;\n@id(\"range.type\") record Range { @id(\"range.lo\") lo:i64, @id(\"range.hi\") hi:i64, } requires lo <= hi\n@id(\"extra.main\") fn main()->i64 { 0 }\n", "invariant-agent.spx").unwrap();
    program.types.extend(extra.types);
    assert!(!crate::source_verify::verify(&program)
        .iter()
        .any(|error| error.severity.is_error()));
    program
}
