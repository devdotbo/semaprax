//! Compact discovery facts; these never grant candidate or backend authority.
use super::*;
use std::collections::BTreeSet;

enum Shape {
    Missing,
    Refused(Vec<Diagnostic>),
    Matches {
        modules: BTreeSet<String>,
        absent: BTreeSet<String>,
    },
}
pub(super) struct Inspection {
    selector: &'static str,
    string: Option<usize>,
    array: Option<usize>,
    missing_string: bool,
    missing_array: bool,
    shape: Shape,
}
impl Inspection {
    pub(super) fn render(self) -> Value {
        let mut missing = Vec::new();
        if self.missing_string {
            missing.push("--max-string-bytes 1..64");
        }
        if self.missing_array {
            missing.push("--max-array-items 1..256");
        }
        let mut row = json!({"selector":self.selector,"shape_matches":Value::Null,"missing_policy":missing,
            "max_string_bytes":self.string,"max_array_items":self.array,
            "candidate_admission":"not_evaluated","backend_admission":"not_evaluated"});
        match self.shape {
            Shape::Missing => {}
            Shape::Refused(errors) => {
                row["shape_matches"] = json!(false);
                row["refusals"] = json!(errors
                    .into_iter()
                    .map(|d| json!({"code":d.code,"reason":d.message}))
                    .collect::<Vec<_>>());
            }
            Shape::Matches { modules, absent } => {
                row["shape_matches"] = json!(true);
                row["required_modules"] = json!(modules);
                row["imports_absent_from_current_entry_index"] = json!(absent);
                row["remaining_prerequisites"] = json!(["ordinary complete candidate dependency/signature/profile/target/ownership/verification admission"]);
            }
        }
        row
    }
}
pub(super) fn inspect(
    revision: &ProjectRevision,
    options: &JsonCodecSuitabilityOptions,
    program: &crate::ast::Program,
    declaration: &TypeDeclaration,
    path: &str,
    profile: JsonCodecProfile,
) -> Result<Inspection, Vec<Diagnostic>> {
    let (selector, string, array) = plan::profile_fields(profile);
    let missing_string = string.is_some() && options.max_string_bytes.is_none();
    let missing_array = array.is_some() && options.max_array_items.is_none();
    let shape = if missing_string || missing_array {
        Shape::Missing
    } else {
        match profile_fragment(program, declaration, profile) {
            Err(errors) => Shape::Refused(errors),
            Ok(fragment) => {
                if fragment.len() > MAX_GENERATED_BYTES {
                    return Err(refusal(
                        "codec explanation fragment exceeds the existing generated-source bound",
                    ));
                }
                let generated = crate::parse(&fragment, path).map_err(|d| vec![d])?;
                let mut modules = BTreeSet::new();
                let mut absent = BTreeSet::new();
                for import in &generated.module_uses {
                    modules.insert(import.target_module.clone());
                    let id = crate::hir::DeclarationId::new(import.persistent_id.as_str());
                    if revision
                        .entry_program()
                        .declarations
                        .declaration(&id)
                        .is_none()
                    {
                        absent.insert(import.persistent_id.clone());
                    }
                }
                Shape::Matches { modules, absent }
            }
        }
    };
    Ok(Inspection {
        selector,
        string: string.and(options.max_string_bytes),
        array: array.and(options.max_array_items),
        missing_string,
        missing_array,
        shape,
    })
}
