//! Capsule v8 updates documentation only; v2–v7 derivation remains byte-identical.
use super::*;

pub(super) const DIGEST_DOMAIN: &[u8] = b"semaprax.project-scaffold.digest.v8\0";

pub(super) fn derive(name: &str, template: &str) -> Result<ProjectScaffoldV1, Vec<Diagnostic>> {
    let mut artifact =
        derive_project_scaffold_v1_with_layout(name, template, ScaffoldLayout::Tables)?;
    let guide = artifact
        .files
        .iter_mut()
        .find(|file| file.path == "AGENTS.md")
        .expect("every scaffold has an agent guide");
    let current = guide.utf8()
        .replace("Before writing source, list\nthe language topics with `semaprax help language topics` and read only the one\nyou need with `semaprax help language <topic>`.",
                 "Start with `semaprax help language author:routes`, then select the route for\nyour project's profile and target. `semaprax help language topics` lists exact\nsections; `semaprax help library <module|name|stable-id>` gives dependency and\nprofile prerequisites. Read one bounded card before requesting the full reference.")
        .replace("A function body is statements followed by exactly one tail expression. There",
                 "A function body ends with a value expression or an admitted terminal return. There")
        .replace("is no `return`, `break`, range `for`, `else if`, cast, tuple, or unit value.",
                 "is no range `for`, cast, tuple, or unit value. For the admitted statement-tail\n  `return`, `break`, and `continue` profile, read `semaprax help language author:lexical-exits`.")
        .replace("is no `return`, `break`, range `for`, cast, tuple, or unit value.",
                 "is no range `for`, cast, tuple, or unit value. For the admitted statement-tail\n  `return`, `break`, and `continue` profile, read `semaprax help language author:lexical-exits`.")
        .replace("- `if` always has `else`; a `while` body ends with the bool that decides\n  whether to loop again.",
                 "- Value `if` requires `else`; statement `if` permits an omitted `else` and `else if`.\n  A `while` condition decides whether to loop; its body ends with a tail expression.");
    guide.bytes = current.into_bytes();
    guide.sha256 = ordinary_sha256(&guide.bytes);
    artifact.layout = ScaffoldLayout::Current;
    artifact.schema = PROJECT_SCAFFOLD_SCHEMA_V8;
    artifact.digest = artifact_digest(
        artifact.template,
        artifact.layout,
        &render_descriptor_without_digest(&artifact),
    );
    if artifact.canonical_bytes().len() > MAX_PROJECT_SCAFFOLD_DESCRIPTOR_BYTES {
        return Err(capacity(
            "project scaffold descriptor exceeds its exact byte limit",
        ));
    }
    Ok(artifact)
}
