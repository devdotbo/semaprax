//! Selected translation unit and private receiving entry.
//!
//! The unit is the ordinary prelude for the same program, the selected
//! runtime, the heap frame type, one closed-domain function and one
//! internal-linkage entry. It has no `main`, exported symbol, public carrier or
//! provider route; the default emitter never reaches this module.
use super::{
    cleanup::PlanProof,
    crossing, error, layout, lower,
    selection::{Sites, ValidatedOwner},
    RUNTIME,
};
use crate::diagnostic::Diagnostic;
use crate::{ast, hir};
use std::fmt::Write as _;

pub(crate) const ENTRY: &str = "spx_fused_selected_entry_v1";

/// Emitted private source of the validated route, and the dimensions it binds.
pub(crate) struct Artifact {
    source: String,
    count: usize,
    width: usize,
    placement: [usize; 3],
}
impl Artifact {
    pub(crate) fn source(&self) -> &str {
        &self.source
    }
    pub(crate) fn count(&self) -> usize {
        self.count
    }
    pub(crate) fn width(&self) -> usize {
        self.width
    }
    /// Private heap-frame carriers [staged rows, row Vec headers, leased views].
    pub(crate) fn placement(&self) -> [usize; 3] {
        self.placement
    }
}

pub(super) struct Unit {
    pub(super) source: String,
    pub(super) placement: [usize; 3],
}

/// Accepting API for the full validated selected owner. The owner must still
/// hold exactly the supplied programs; the receiver then proves crossings,
/// layout and canonical cleanup order itself before emitting anything.
pub(crate) fn emit_validated<'a, O: ValidatedOwner<'a>>(
    source: &ast::Program,
    program: &hir::ResolvedProgram,
    owner: &'a O,
) -> Result<Artifact, Diagnostic> {
    if !owner.still_current(source, program) || !std::ptr::eq(owner.program(), program) {
        return Err(error("validated owner does not hold the supplied programs"));
    }
    let sites = Sites::from_validated(owner);
    let unit = unit(&sites)?;
    Ok(Artifact {
        source: unit.source,
        count: sites.rows as usize,
        width: sites.width as usize,
        placement: unit.placement,
    })
}

/// Shared receiver core. Ordinary HIR validation still runs inside the
/// prelude hook as a necessary check; it is not selection authority.
pub(super) fn unit(sites: &Sites<'_>) -> Result<Unit, Diagnostic> {
    if !layout::native64() {
        return Err(error("the selected carriers are defined for native64 only"));
    }
    if !sites.held() || sites.program.functions.len() != 1 {
        return Err(error("the selected function is not the held program's only function"));
    }
    let count = usize::try_from(sites.rows).map_err(|_| error("row count overflow"))?;
    let width = usize::try_from(sites.width).map_err(|_| error("width overflow"))?;
    let extent = count
        .checked_mul(width)
        .filter(|extent| {
            *extent != 0 && *extent <= crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES as usize
        })
        .ok_or_else(|| error("selected backing extent is outside the individual Bytes envelope"))?;
    if count == 0 || count > 4096 {
        return Err(error("selected row count is outside the 4096-row strategy ceiling"));
    }
    // Row placement: the selected row equals the ordinary leaf stride, so the
    // ordinary Vec allocation boundary funds it unchanged.
    let ordinary = super::super::native_vec::owned_leaf::layout(sites.program, &sites.item.ty)
        .ok_or_else(|| error("the selected row has no ordinary leaf layout"))?;
    if ordinary.stride as usize != layout::row() {
        return Err(error("the selected row differs from the ordinary leaf stride"));
    }
    crossing::refuse_crossings(sites)?;
    let cleanup = PlanProof::admit(sites.function)?;
    let lowered = lower::lower(sites, &cleanup)?;
    let [staged, rows, views] = lowered.placement;
    let frame = layout::frame(staged, rows, views)
        .ok_or_else(|| error("the selected frame exceeds its bound"))?;
    let metadata =
        layout::metadata(count, frame).ok_or_else(|| error("selected metadata overflow"))?;
    let mut output = super::super::native_emit::emit_selected_fused_row_prelude(sites.program)?;
    output.push_str(
        "#ifndef SPX_FUSED_FAIL_STOP\n#define SPX_FUSED_FAIL_STOP(message) spx_runtime_invariant_failure(message)\n#endif\n",
    );
    output.push_str(RUNTIME);
    output.push_str("typedef struct {\n    spx_fused_domain domain;\n");
    for (field, ty) in &lowered.frame {
        writeln!(output, "    {ty} {field};").expect("writing to a string cannot fail");
    }
    output.push_str("} spx_fused_frame;\n#if UINTPTR_MAX == UINT64_MAX\n");
    for (ty, size) in layout::mirror() {
        writeln!(
            output,
            "_Static_assert(sizeof({ty}) == {size}u, \"receiver layout mirror: {ty}\");"
        )
        .expect("writing to a string cannot fail");
    }
    writeln!(
        output,
        "_Static_assert(sizeof(spx_fused_frame) == {frame}u, \"receiver layout mirror: frame\");\n#endif"
    )
    .expect("writing to a string cannot fail");
    let order = lowered.order.map(|role| role.c()).join(", ");
    write!(
        output,
        "typedef struct {{ const unsigned char *ptr; uint64_t len; }} spx_fused_text;\n\
         static const uint32_t spx_fused_order[3] = {{ {order} }};\n\
         #define SPX_FUSED_SELECTED_COUNT ((size_t){count}u)\n\
         #define SPX_FUSED_SELECTED_WIDTH ((size_t){width}u)\n\
         #define SPX_FUSED_SELECTED_EXTENT ((size_t){extent}u)\n\
         #define SPX_FUSED_SELECTED_METADATA ((size_t){metadata}u)\n\
         _Static_assert(SPX_FUSED_SELECTED_EXTENT <= SPX_FUSED_EXTENT, \"selected extent fits one Bytes value\");\n",
    )
    .expect("writing to a string cannot fail");
    output.push_str(&lowered.function);
    output.push_str(&entry());
    Ok(Unit {
        source: output,
        placement: lowered.placement,
    })
}

/// Private outcomes, not language statuses: 0 published result, 1 selected
/// language failure (status token written), 2 admission refusal before any
/// effect, 3 abandoned invocation. Other outputs stay untouched.
fn entry() -> String {
    format!(
        r#"enum {{ SPX_FUSED_PUBLISHED = 0, SPX_FUSED_LANGUAGE_FAILURE = 1, SPX_FUSED_REFUSED = 2, SPX_FUSED_ABANDONED = 3 }};
/* Admission funds the heap frame (header and private carriers) and the
   tables in the held metadata ledger, and holds one ordinary backing event,
   before any program effect. The frame outlives the selected function, its
   row Vec and every popped child until settlement. */
static __attribute__((unused)) int {ENTRY}(
    struct spx_context *spx_ctx, spx_fused_issuer *spx_issuer, spx_fused_ledger *spx_ledger,
    spx_fused_payload *spx_payload, int64_t *spx_result_out, spx_status_token *spx_status_out
) {{
    if (spx_ctx == NULL || spx_issuer == NULL || spx_ledger == NULL || spx_payload == NULL ||
        spx_result_out == NULL || spx_status_out == NULL || !spx_ledger->sealed ||
        !spx_payload->sealed || !spx_issuer->sealed)
        spx_runtime_invariant_failure("selected fused-row entry without held authority");
    if (spx_fused_metadata_bytes(SPX_FUSED_SELECTED_COUNT, sizeof(spx_fused_frame)) != SPX_FUSED_SELECTED_METADATA)
        spx_runtime_invariant_failure("selected fused-row metadata mirror");
    size_t spx_debited = spx_ledger->debited, spx_events = spx_payload->events;
    size_t spx_cumulative = spx_payload->cumulative, spx_held = spx_payload->held_events;
    void *spx_storage = spx_fused_admit(spx_issuer, spx_ledger, spx_payload, SPX_FUSED_SELECTED_COUNT,
        SPX_FUSED_SELECTED_WIDTH, sizeof(spx_fused_frame));
    if (spx_storage == NULL) return SPX_FUSED_REFUSED;
    spx_fused_frame *spx_frame = (spx_fused_frame *)spx_storage;
    spx_fused_bind(&spx_frame->domain, spx_issuer, spx_ledger, spx_payload, SPX_FUSED_SELECTED_COUNT,
        SPX_FUSED_SELECTED_WIDTH, sizeof(spx_fused_frame));
    int64_t spx_result = INT64_C(0);
    spx_status_token spx_status = {function}(spx_ctx, spx_frame, &spx_result);
    int spx_settled = spx_fused_finish(&spx_frame->domain);
    uint32_t spx_activated = spx_frame->domain.activated, spx_failed = spx_frame->domain.failure;
    size_t spx_metadata = spx_frame->domain.metadata;
    free(spx_storage);
    if (!spx_settled) return SPX_FUSED_ABANDONED;
    if (spx_status != SPX_STATUS_SUCCESS) {{
        *spx_status_out = spx_status;
        return SPX_FUSED_LANGUAGE_FAILURE;
    }}
    /* Result postconditions: the exact metadata debit, exactly one ordinary
       event and extent consumed from the admitted hold, no sticky failure. */
    if (!spx_activated || spx_failed || spx_metadata != SPX_FUSED_SELECTED_METADATA ||
        spx_ledger->debited - spx_debited != spx_metadata ||
        spx_payload->events - spx_events != 1u ||
        spx_payload->cumulative - spx_cumulative != SPX_FUSED_SELECTED_EXTENT ||
        spx_payload->held_events != spx_held)
        spx_runtime_invariant_failure("selected fused-row result postcondition");
    *spx_result_out = spx_result;
    return SPX_FUSED_PUBLISHED;
}}
"#,
        function = lower::FUNCTION
    )
}
