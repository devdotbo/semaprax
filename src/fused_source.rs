//! Explicit source-only fused-construction-v1 execution.
//!
//! Callers provide source text and a finite account, never a parsed/HIR proof,
//! joined grant or replacement artifact. Ordinary routes do not call this API.
use crate::diagnostic::Diagnostic;
use crate::hir::selected_fused_candidate::{
    validate_selected, SelectedRefusal, ValidatedFusedSelection,
};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const PROFILE: &str = "fused-construction-v1";
pub const ENVELOPE_SCHEMA: &str = "semaprax.fused-source.v1";
/// New profile administrative ceiling, independent of compiler builder quotas,
/// ordinary Bytes/String limits, interpreter fuel and total process memory.
pub const MAX_MANAGED_BYTES: usize = 1_073_741_824;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAccount {
    bytes: usize,
}
impl RuntimeAccount {
    pub fn new(bytes: usize) -> Result<Self, Diagnostic> {
        if bytes == 0 || bytes > MAX_MANAGED_BYTES {
            return Err(refusal("managed account requires 1..=1073741824 bytes"));
        }
        Ok(Self { bytes })
    }
    pub const fn bytes(self) -> usize {
        self.bytes
    }
}

/// Bound result envelope; its result field exists only after receiver cleanup
/// and successful publication. This data grants no source or runtime authority.
#[derive(Debug)]
pub struct RunReport {
    envelope: String,
    returned: bool,
}
impl RunReport {
    pub fn envelope(&self) -> &str {
        &self.envelope
    }
    pub const fn returned(&self) -> bool {
        self.returned
    }
}

/// Read-free scoped factory. The AST is parsed once and borrowed immutably by
/// the full selection for the complete receiver call. There is no fallback.
fn with_selected<R>(
    source: &str,
    path: &Path,
    receive: impl FnOnce(&ValidatedFusedSelection<'_>, &str) -> Result<R, Vec<Diagnostic>>,
) -> Result<R, Vec<Diagnostic>> {
    let parsed = crate::parse(source, path).map_err(|error| vec![error])?;
    let owner = validate_selected(&parsed).map_err(selection_errors)?;
    let [entry] = owner.program().functions.as_slice() else {
        return Err(vec![refusal("source profile requires exactly one entry")]);
    };
    if entry.id != owner.program().entrypoint
        || entry.name != "main"
        || !entry.params.is_empty()
        || entry.return_type != crate::hir::ResolvedType::I64
    {
        return Err(vec![refusal("source profile requires main() -> i64")]);
    }
    let digest = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    );
    receive(&owner, &digest)
}
fn selection_errors(error: SelectedRefusal) -> Vec<Diagnostic> {
    use crate::source_verify::fused_construction::candidate::CandidateRefusal;
    match error {
        SelectedRefusal::Source(CandidateRefusal::Source(errors)) => errors,
        SelectedRefusal::Resolve(error)
        | SelectedRefusal::Validation(error)
        | SelectedRefusal::Capacity(error) => vec![error],
        other => vec![refusal(format!("selected source refused: {other:?}"))],
    }
}
fn refusal(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-F130", message)
}

/// Execute one selected invocation with a caller-owned finite requested-layout
/// account. Runtime frame construction, execution and cleanup share this ledger.
/// Compiler parsing/validation/joining, diagnostic/status text and report
/// serialization are outside this runtime accounting scope. It is not a bound on allocator overhead or process memory.
pub fn run_interpreter(
    source: &str,
    path: &Path,
    account: RuntimeAccount,
    fuel: usize,
) -> Result<RunReport, Vec<Diagnostic>> {
    crate::interpreter::InterpreterOptions::new(
        crate::interpreter::InterpreterOptions::default().max_bytes,
        fuel,
    )
    .map_err(|error| vec![error])?;
    with_selected(source, path, |owner, digest| {
        use crate::interpreter::construction_fusion::selected::{execute_accounted, Failure};
        let (answer, refused, used) = execute_accounted(owner, fuel, account.bytes);
        let mut value = serde_json::json!({"schema":ENVELOPE_SCHEMA,"profile":PROFILE,"source_sha256":digest,"target":"interpreter","account_limit":account.bytes,"managed_reserved":used,"account_policy":"interpreter.requested-layout.v1","max_steps":fuel,"account_refused":refused});
        let returned = match answer {
            Ok(answer) if !refused => {
                value["outcome"] = "returned".into();
                value["result"] = answer.result.into();
                value["steps_used"] = answer.steps.into();
                true
            }
            Ok(_) => {
                value["outcome"] = "administrative_refusal".into();
                value["reason"] = "managed_account".into();
                false
            }
            Err(Failure::Authority) => {
                value["outcome"] = "administrative_refusal".into();
                value["reason"] = "receiving_authority".into();
                false
            }
            Err(Failure::FuelExhausted {
                steps,
                cleanup_complete,
            }) => {
                value["outcome"] = "fuel_exhausted".into();
                value["steps_used"] = steps.into();
                value["cleanup_complete"] = cleanup_complete.into();
                false
            }
            Err(Failure::DepthExceeded {
                steps,
                cleanup_complete,
            }) => {
                value["outcome"] = "call_depth_exceeded".into();
                value["steps_used"] = steps.into();
                value["cleanup_complete"] = cleanup_complete.into();
                false
            }
            Err(Failure::Execution {
                status,
                steps,
                cleanup_complete,
            }) => {
                value["steps_used"] = steps.into();
                value["cleanup_complete"] = cleanup_complete.into();
                if let Some(status) = status {
                    value["outcome"] = "language_failure".into();
                    value["status"] =
                        serde_json::json!({ "domain": status.domain_id(), "code": status.code() });
                    value["normalized_status"] =
                        serde_json::from_str::<serde_json::Value>(status.to_json().as_str())
                            .map_err(|_| vec![refusal("receiver status serialization refused")])?;
                } else {
                    value["outcome"] = "administrative_refusal".into();
                    value["reason"] = if refused {
                        "managed_account"
                    } else {
                        "runtime_guard"
                    }
                    .into();
                }
                false
            }
        };
        Ok(RunReport {
            envelope: value.to_string(),
            returned,
        })
    })
}

/// Emit a complete native entry from the same held selection. Native has no
/// interpreter fuel contract; its account policy is target-specific.
pub fn emit_native(
    source: &str,
    path: &Path,
    account: RuntimeAccount,
) -> Result<String, Vec<Diagnostic>> {
    with_selected(source, path, |owner, digest| {
        crate::codegen::emit_fused_source_profile(owner, account.bytes, digest)
            .map_err(|error| vec![error])
    })
}
#[cfg(test)]
mod tests;

/// An inseparable compiler-produced module/host/manifest pairing. Public callers
/// can inspect or publish its fixed artifacts; they cannot construct a package
/// from a replacement module or a numeric runtime grant.
pub struct WasmPackage {
    files: Vec<(&'static str, Vec<u8>)>,
}
impl WasmPackage {
    pub(crate) fn from_files(files: Vec<(&'static str, Vec<u8>)>) -> Self {
        Self { files }
    }
    pub fn files(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.files
            .iter()
            .map(|(name, bytes)| (*name, bytes.as_slice()))
    }
    pub fn publish(&self, output: &Path) -> Result<(), Diagnostic> {
        crate::wasm::publish_fused_source_package(self, output)
    }
}
/// Generate the paired private-host package from one held selected source. This
/// supplies no public-provider ABI and no cross-backend interpreter fuel claim.
pub fn emit_wasm_package(
    source: &str,
    path: &Path,
    account: RuntimeAccount,
) -> Result<WasmPackage, Vec<Diagnostic>> {
    with_selected(source, path, |owner, digest| {
        crate::wasm::emit_fused_source_package(owner, account.bytes, digest)
            .map_err(|error| vec![error])
    })
}
