//! Context attachment and status bridge for the selected runtime.
//!
//! The context gains one explicit registry reference, never an inline table
//! or a global. The entry bridge holds the registry, attaches it before the
//! first Bytes producer and detaches it after final cleanup, where
//! `spx_shb_registry_finish` proves every backing was released exactly once.
//! Outcomes map onto existing status domains through the owning Rust
//! constants, including the shared partition operation owner. Provider
//! allocation remains a physical fail-stop without a fabricated status.
use std::fmt::Write as _;

use super::closure::{Consumer, Selection, Support};
use crate::byte_ops::{CAPACITY_EXCEEDED_CODE, SET_INDEX_OUT_OF_BOUNDS_CODE, SET_STATUS_DOMAIN};
use crate::vec_ops::{
    ALLOCATION_FAILURE_CODE, GET_OUT_OF_BOUNDS_CODE, PUSH_FULL_CODE, STATUS_DOMAIN,
};

/// Rust mirror of the runtime outcome enum, in declaration order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub(super) enum Outcome {
    Ok = 0,
    BufferIndex,
    BufferBound,
    VecFull,
    VecIndex,
    VecCapacityRefused,
    PartitionWidthZero,
    PartitionExtentOverflow,
    PartitionExtentMismatch,
    PartitionMetadataCapacity,
    ProviderAllocation,
}

impl Outcome {
    pub(super) const ALL: [Self; 11] = [
        Self::Ok,
        Self::BufferIndex,
        Self::BufferBound,
        Self::VecFull,
        Self::VecIndex,
        Self::VecCapacityRefused,
        Self::PartitionWidthZero,
        Self::PartitionExtentOverflow,
        Self::PartitionExtentMismatch,
        Self::PartitionMetadataCapacity,
        Self::ProviderAllocation,
    ];

    pub(super) const fn c_name(self) -> &'static str {
        match self {
            Self::Ok => "SPX_SHB_OK",
            Self::BufferIndex => "SPX_SHB_BUFFER_INDEX",
            Self::BufferBound => "SPX_SHB_BUFFER_BOUND",
            Self::VecFull => "SPX_SHB_VEC_FULL",
            Self::VecIndex => "SPX_SHB_VEC_INDEX",
            Self::VecCapacityRefused => "SPX_SHB_VEC_CAPACITY_REFUSED",
            Self::PartitionWidthZero => "SPX_SHB_PARTITION_WIDTH_ZERO",
            Self::PartitionExtentOverflow => "SPX_SHB_PARTITION_EXTENT_OVERFLOW",
            Self::PartitionExtentMismatch => "SPX_SHB_PARTITION_EXTENT_MISMATCH",
            Self::PartitionMetadataCapacity => "SPX_SHB_PARTITION_METADATA_CAPACITY",
            Self::ProviderAllocation => "SPX_SHB_PROVIDER_ALLOCATION",
        }
    }

    /// The existing domain and code, or `None` when no status is assigned.
    pub(super) fn status(self) -> Option<(&'static str, u32)> {
        let partition = super::closure::PARTITION_STATUS_CODES;
        match self {
            Self::Ok => None,
            Self::BufferIndex => Some((SET_STATUS_DOMAIN, SET_INDEX_OUT_OF_BOUNDS_CODE)),
            Self::BufferBound => Some((SET_STATUS_DOMAIN, CAPACITY_EXCEEDED_CODE)),
            Self::VecFull => Some((STATUS_DOMAIN, PUSH_FULL_CODE)),
            Self::VecIndex => Some((STATUS_DOMAIN, GET_OUT_OF_BOUNDS_CODE)),
            Self::VecCapacityRefused => Some((STATUS_DOMAIN, ALLOCATION_FAILURE_CODE)),
            Self::PartitionWidthZero => partition.map(|codes| (PARTITION_DOMAIN, codes[0])),
            Self::PartitionExtentOverflow => partition.map(|codes| (PARTITION_DOMAIN, codes[1])),
            Self::PartitionExtentMismatch => partition.map(|codes| (PARTITION_DOMAIN, codes[2])),
            Self::PartitionMetadataCapacity => {
                partition.map(|codes| (PARTITION_DOMAIN, codes[3]))
            }
            Self::ProviderAllocation => None,
        }
    }
}

const PARTITION_DOMAIN: &str = crate::construction_fusion::partition::operation::STATUS_DOMAIN;

/// Add the explicit registry reference to the emitted invocation context.
/// Mirrors the Vec authority extension and asserts each anchor applied once.
pub(in crate::codegen) fn extend_context(runtime: String) -> String {
    let extended = runtime
        .replacen(
            "struct spx_context {",
            "struct spx_shb_registry;\n\nstruct spx_context {",
            1,
        )
        .replacen(
            "    uint32_t call_depth;",
            "    uint32_t call_depth;\n    struct spx_shb_registry *shared_bytes;",
            1,
        )
        .replacen(
            "        context->call_depth == UINT32_C(0)",
            "        context->call_depth == UINT32_C(0) &&\n        context->shared_bytes == NULL",
            1,
        )
        .replacen(
            "    context->call_depth = UINT32_C(0);",
            "    context->call_depth = UINT32_C(0);\n    context->shared_bytes = NULL;",
            1,
        );
    assert_eq!(
        extended.matches("shared_bytes").count(),
        3,
        "shared Bytes context anchor changed"
    );
    assert_eq!(extended.matches("struct spx_shb_registry;").count(), 1);
    extended
}

/// Emit the selected runtime and its bridge for a complete closure only.
pub(in crate::codegen) fn emit_selected_runtime(
    output: &mut impl crate::codegen::COutput,
    selection: &Selection,
) {
    for consumer in selection.consumers() {
        assert!(
            matches!(consumer.support(), Support::Shared(_)),
            "{consumer:?} reached emission outside the closure"
        );
    }
    let partition = selection.consumers().contains(&Consumer::Partition);
    assert!(!partition || super::closure::PARTITION_STATUS_CODES.is_some());
    output.push_str(super::RUNTIME);
    output.push_str("\n");
    output.push_str(&bridge_c());
}

fn bridge_c() -> String {
    let mut c = String::from(
        "static __attribute__((unused)) spx_status_token spx_shb_status(\n    struct spx_context *spx_ctx, uint32_t outcome\n) {\n    const char *domain = NULL;\n    uint32_t code = UINT32_C(0);\n    switch (outcome) {\n",
    );
    for outcome in Outcome::ALL {
        match (outcome, outcome.status()) {
            (Outcome::Ok, _) => {
                writeln!(c, "    case {}: return SPX_STATUS_SUCCESS;", outcome.c_name()).unwrap()
            }
            (_, Some((domain, code))) => writeln!(
                c,
                "    case {}: domain = \"{domain}\"; code = UINT32_C({code}); break;",
                outcome.c_name()
            )
            .unwrap(),
            (_, None) => {}
        }
    }
    c.push_str(
        r#"    default: spx_runtime_invariant_failure("shared Bytes outcome has no assigned status");
    }
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(spx_ctx, domain, code, SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_FALSE, &token)) {
        spx_runtime_invariant_failure("shared Bytes status could not be recorded");
    }
    return token;
}

static __attribute__((unused)) spx_shb_registry *spx_shb_held(struct spx_context *spx_ctx) {
    if (spx_ctx == NULL || spx_ctx->state != SPX_CONTEXT_INITIALIZED || spx_ctx->shared_bytes == NULL)
        spx_runtime_invariant_failure("shared Bytes registry is not attached to this invocation");
    spx_shb_require(spx_ctx->shared_bytes);
    return spx_ctx->shared_bytes;
}

/* The entry bridge owns both objects for the whole invocation. */
static __attribute__((unused)) void spx_shb_attach(
    struct spx_context *spx_ctx, spx_shb_registry *registry, spx_shb_issuer *issuer
) {
    if (spx_ctx == NULL || spx_ctx->state != SPX_CONTEXT_INITIALIZED || spx_ctx->shared_bytes != NULL)
        spx_runtime_invariant_failure("shared Bytes registry attached twice");
    spx_shb_registry_init(registry, issuer);
    spx_ctx->shared_bytes = registry;
}

static __attribute__((unused)) void spx_shb_detach(struct spx_context *spx_ctx) {
    spx_shb_registry *registry = spx_shb_held(spx_ctx);
    spx_shb_registry_finish(registry);
    spx_ctx->shared_bytes = NULL;
}
"#,
    );
    c
}

#[cfg(test)]
pub(super) fn bridge_c_for_tests() -> String {
    bridge_c()
}
