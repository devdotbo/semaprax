//! Explicit source-only native adapter. Ordinary emitters never call it.
//! The caller supplies finite requested-heap funding; this is not process RSS.
use super::{emit_validated, error, layout};
use crate::diagnostic::Diagnostic;
use crate::hir::{selected_fused_candidate::ValidatedFusedSelection, ResolvedExprKind};
use std::fmt::Write as _;
#[cfg(test)]
mod tests;

const ADAPTER: &str = include_str!("source_profile/adapter.c");

/// Fresh sealed source/HIR authority is required. The source façade computes
/// this identity from the exact raw bytes it holds; the hex string grants no
/// source, graph, provider or layout authority by itself.
pub(crate) fn emit_source_profile(
    owner: &ValidatedFusedSelection<'_>,
    account_bytes: usize,
    source_sha256: &str,
) -> Result<String, Diagnostic> {
    if account_bytes == 0
        || source_sha256.len() != 64
        || !source_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(error(
            "source profile requires positive caller funding and a raw-source SHA256",
        ));
    }
    let artifact = emit_validated(owner)?;
    let literal_bytes = owner
        .with_joined(|joined| {
            if !joined.still_current(owner.source(), owner.program()) {
                return Err(error("source profile lost its current sealed join"));
            }
            let ResolvedExprKind::ConstructRecord { fields, .. } =
                &joined.selection().constructor().kind
            else {
                return Err(error("selected literal constructor is absent"));
            };
            let mut literals = fields.iter().filter_map(|field| {
                if let ResolvedExprKind::String(text) = &field.value.kind {
                    Some(text.len())
                } else {
                    None
                }
            });
            let len = literals
                .next()
                .ok_or_else(|| error("selected literal String is absent"))?;
            if literals.next().is_some() {
                return Err(error("selected literal inventory is ambiguous"));
            }
            Ok(len)
        })
        .map_err(|_| error("source profile join refused"))??;
    let [staged, rows, views] = artifact.placement();
    let frame =
        layout::frame(staged, rows, views).ok_or_else(|| error("profile frame overflow"))?;
    let structure = layout::funding(artifact.count(), frame)
        .ok_or_else(|| error("profile funding overflow"))?;
    let payload = artifact
        .count()
        .checked_mul(artifact.width())
        .ok_or_else(|| error("profile payload overflow"))?;
    // The emitted native64 String runtime requests offsetof(header,data)+len+1.
    // Its actual C offsetof is asserted below; an empty literal still allocates.
    let strings = literal_bytes
        .checked_add(9)
        .and_then(|n| n.checked_mul(artifact.count()))
        .ok_or_else(|| error("profile String funding overflow"))?;
    let required = structure
        .checked_add(payload)
        .and_then(|n| n.checked_add(strings))
        .ok_or_else(|| error("profile managed funding overflow"))?;
    let mut source = artifact.source().to_owned();
    writeln!(source, "\n#define SPX_PROFILE_LIMIT ((size_t){account_bytes}u)\n#define SPX_PROFILE_REQUIRED ((size_t){required}u)\n#define SPX_PROFILE_STRINGS ((size_t){strings}u)\n#define SPX_PROFILE_PAYLOAD ((size_t){payload}u)\n#define SPX_PROFILE_SOURCE_SHA256 \"{source_sha256}\"\n_Static_assert(offsetof(struct spx_string_v10,data)==8u, \"profile String request mirror\");\n_Static_assert(SPX_PROFILE_REQUIRED==SPX_FUSED_SELECTED_FUNDING+SPX_PROFILE_STRINGS+SPX_PROFILE_PAYLOAD, \"profile funded inventory\");")
        .expect("writing to String cannot fail");
    source.push_str(ADAPTER);
    Ok(source)
}
