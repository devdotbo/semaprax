//! Saved eager profile-row oracle from 5bf43b4dd.
use super::*;
pub(super) fn rows(
    revision: &ProjectRevision,
    options: &JsonCodecSuitabilityOptions,
    program: &crate::ast::Program,
    declaration: &TypeDeclaration,
    path: &str,
) -> Result<Vec<Value>, Vec<Diagnostic>> {
    let mut rows = Vec::with_capacity(12);
    for profile in profiles(
        options.max_string_bytes.unwrap_or(1),
        options.max_array_items.unwrap_or(1),
    ) {
        let (selector, string, array) = plan::profile_fields(profile);
        let mut missing = Vec::new();
        if string.is_some() && options.max_string_bytes.is_none() {
            missing.push("--max-string-bytes 1..64");
        }
        if array.is_some() && options.max_array_items.is_none() {
            missing.push("--max-array-items 1..256");
        }
        let mut row = json!({"selector":selector,"shape_matches":Value::Null,"missing_policy":missing,
            "max_string_bytes":if string.is_some() { options.max_string_bytes } else { None },
            "max_array_items":if array.is_some() { options.max_array_items } else { None },
            "candidate_admission":"not_evaluated","backend_admission":"not_evaluated"});
        if row["missing_policy"]
            .as_array()
            .expect("policy list")
            .is_empty()
        {
            match profile_fragment(program, declaration, profile) {
                Err(errors) => {
                    row["shape_matches"] = json!(false);
                    row["refusals"] = diagnostics(errors);
                }
                Ok(fragment) => {
                    if fragment.len() > MAX_GENERATED_BYTES {
                        return Err(refusal("codec explanation fragment exceeds the existing generated-source bound"));
                    }
                    let generated = crate::parse(&fragment, path).map_err(|d| vec![d])?;
                    let mut modules = std::collections::BTreeSet::new();
                    let mut absent = std::collections::BTreeSet::new();
                    for import in &generated.module_uses {
                        modules.insert(import.target_module.as_str());
                        let id = crate::hir::DeclarationId::new(import.persistent_id.as_str());
                        if revision
                            .entry_program()
                            .declarations
                            .declaration(&id)
                            .is_none()
                        {
                            absent.insert(import.persistent_id.as_str());
                        }
                    }
                    row["shape_matches"] = json!(true);
                    row["required_modules"] = json!(modules);
                    row["imports_absent_from_current_entry_index"] = json!(absent);
                    row["remaining_prerequisites"]=json!(["ordinary complete candidate dependency/signature/profile/target/ownership/verification admission"]);
                }
            }
        }
        rows.push(row);
    }
    Ok(rows)
}
