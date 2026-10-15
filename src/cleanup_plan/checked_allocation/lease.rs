//! Captured allocator/release lease over one validated checked closure.
//!
//! Capture repeats the complete projected replay and then binds every owned
//! lifecycle of every held function to one closed allocator-owned release
//! family. A foreign finalizer cannot share the lease. The lease borrows the
//! immutable validated owner for its whole lifetime, so a receiver cannot
//! swap allocators under a live owner or detach a plan from its proof.
//!
//! No engine receives this lease yet. Native, Core-Wasm and interpreter
//! receivers each require their own checked constructor routing, status
//! registration and leased release before the uninhabited success can
//! change. Escaped String/Bytes SDK results remain refused at preparation.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Engine {
    Native,
    Wasm,
    Interpreter,
}

/// Allocator-owned release families, in this fixed order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Release {
    String,
    Bytes,
    Vec,
    Box,
    StringMap,
    Collection,
}
const RELEASES: [Release; 6] = [
    Release::String,
    Release::Bytes,
    Release::Vec,
    Release::Box,
    Release::StringMap,
    Release::Collection,
];

fn release(lifecycle: &str) -> Option<Release> {
    match lifecycle {
        crate::cleanup::STRING_DROP_LIFECYCLE_ID => Some(Release::String),
        crate::cleanup::BYTES_DROP_LIFECYCLE_ID => Some(Release::Bytes),
        crate::cleanup::VEC_DROP_LIFECYCLE_ID => Some(Release::Vec),
        crate::cleanup::BOX_DROP_LIFECYCLE_ID => Some(Release::Box),
        crate::string_ops::MAP_DROP_LIFECYCLE_ID => Some(Release::StringMap),
        crate::map_ops::DROP_ID => Some(Release::Collection),
        _ => None,
    }
}

pub(crate) struct Lease<'v, 'source> {
    validated: &'v Validated<'source>,
    used: [bool; 6],
}

pub(crate) fn capture<'v, 'source>(
    validated: &'v Validated<'source>,
) -> Result<Lease<'v, 'source>, Refusal> {
    validated.replay().map_err(Refusal::Projection)?;
    let program = validated.program();
    let mut used = [false; 6];
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        for flag in &function.cleanup.flags {
            let family = release(flag.lifecycle.as_str()).ok_or_else(|| {
                lease_error(&format!(
                    "function `{}` lifecycle `{}` is not an allocator-owned release",
                    function.id, flag.lifecycle,
                ))
            })?;
            used[family as usize] = true;
        }
    }
    Ok(Lease { validated, used })
}

impl<'v, 'source> Lease<'v, 'source> {
    pub(crate) fn validated(&self) -> &'v Validated<'source> {
        self.validated
    }

    /// Release families an engine must route through this one allocator pair.
    pub(crate) fn releases(&self) -> impl Iterator<Item = Release> + '_ {
        RELEASES
            .into_iter()
            .filter(move |family| self.used[*family as usize])
    }

    /// Every engine refuses until its receiver independently routes checked
    /// constructors, registers the allocation status and binds each release.
    pub(crate) fn receive(&self, engine: Engine) -> Result<std::convert::Infallible, Refusal> {
        Err(lease_error(match engine {
            Engine::Native => "native checked constructor routing and leased release are pending",
            Engine::Wasm => "versioned Core-Wasm checked host import and out-cell are pending",
            Engine::Interpreter => "interpreter fallible materialization receiver is pending",
        }))
    }
}

fn lease_error(message: &str) -> Refusal {
    Refusal::Lease(Diagnostic::io(
        "SPX-B104",
        format!("checked allocation lease: {message}"),
    ))
}
