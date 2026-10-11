//! Backing newly allocated by a selected Clone, rather than logical work units.
//! Arc/Rc identity clones share held bytes; only their new collection slots are
//! charged. Owned strings, type carriers, projections and flat tails allocate.
use super::*;
use std::mem::size_of;

pub(crate) trait ClonePayload {
    fn clone_payload_bytes(&self) -> Option<usize>;
}
impl ClonePayload for DeclarationId {
    fn clone_payload_bytes(&self) -> Option<usize> {
        Some(self.as_str().len())
    }
}
impl ClonePayload for FunctionInstanceId {
    fn clone_payload_bytes(&self) -> Option<usize> {
        Some(self.as_str().len())
    }
}
macro_rules! shared_or_copy {
    ($($ty:ty),* $(,)?) => { $(impl ClonePayload for $ty {
        fn clone_payload_bytes(&self) -> Option<usize> { Some(0) }
    })* };
}
shared_or_copy!(
    ExpressionId,
    crate::hir::ValueId,
    StorageId,
    StatusSourceId,
    u32,
    u64,
    usize,
    bool,
    LivenessFlagId,
    BlockId,
    EdgeId
);
impl<T> ClonePayload for std::rc::Rc<T> {
    fn clone_payload_bytes(&self) -> Option<usize> {
        Some(0)
    }
}
impl<T: ClonePayload> ClonePayload for Option<T> {
    fn clone_payload_bytes(&self) -> Option<usize> {
        self.as_ref()
            .map_or(Some(0), ClonePayload::clone_payload_bytes)
    }
}
impl<A: ClonePayload, B: ClonePayload> ClonePayload for (A, B) {
    fn clone_payload_bytes(&self) -> Option<usize> {
        self.0
            .clone_payload_bytes()?
            .checked_add(self.1.clone_payload_bytes()?)
    }
}
impl<T: ClonePayload> ClonePayload for Vec<T> {
    fn clone_payload_bytes(&self) -> Option<usize> {
        self.iter()
            .try_fold(self.len().checked_mul(size_of::<T>())?, |total, value| {
                total.checked_add(value.clone_payload_bytes()?)
            })
    }
}
impl ClonePayload for CleanupPlace {
    fn clone_payload_bytes(&self) -> Option<usize> {
        self.projections.clone_payload_bytes()
    }
}
impl ClonePayload for crate::hir::FunctionExecutionId {
    fn clone_payload_bytes(&self) -> Option<usize> {
        match self {
            Self::Monomorphic(id) => id.clone_payload_bytes(),
            Self::Generic(id) => id.clone_payload_bytes(),
        }
    }
}
impl ClonePayload for crate::hir::ControlTarget {
    fn clone_payload_bytes(&self) -> Option<usize> {
        match self {
            Self::Function(owner) | Self::Loop { owner, .. } => owner.clone_payload_bytes(),
        }
    }
}
impl ClonePayload for ResolvedType {
    fn clone_payload_bytes(&self) -> Option<usize> {
        // This one active fixed cursor is reserved by the selected entry before
        // any clone preflight runs. Wide type lists retain one ancestor cursor.
        let mut frames = [None; 514];
        frames[0] = Some((self, 0usize));
        let mut len = 1;
        let mut bytes = 0usize;
        let mut work = MAX_REPLAY_WORK_UNITS;
        while len != 0 {
            work = work.checked_sub(1)?;
            let (ty, next) = frames[len - 1]?;
            if next == 0 {
                let own = match ty {
                    Self::TypeParameter { owner, .. } => owner.as_str().len(),
                    Self::Nominal {
                        declaration,
                        arguments,
                    } => declaration
                        .as_str()
                        .len()
                        .checked_add(arguments.len().checked_mul(size_of::<Self>())?)?,
                    Self::Function { parameters, .. } => parameters
                        .len()
                        .checked_mul(size_of::<Self>())?
                        .checked_add(size_of::<Self>())?,
                    Self::Never
                    | Self::Unit
                    | Self::I64
                    | Self::I32
                    | Self::Char
                    | Self::U8
                    | Self::Usize
                    | Self::ArrayU8(_)
                    | Self::F32
                    | Self::F64
                    | Self::Bool
                    | Self::String
                    | Self::Bytes
                    | Self::Str
                    | Self::SliceU8
                    | Self::StringMap
                    | Self::OnceFunction
                    | Self::OnceFunctionI64
                    | Self::OnceFunctionI64Pair
                    | Self::MutFunctionI64 => 0,
                };
                bytes = bytes.checked_add(own)?;
            }
            let child = match ty {
                Self::Nominal { arguments, .. } => arguments.get(next),
                Self::Function { parameters, result } => parameters
                    .get(next)
                    .or_else(|| (next == parameters.len()).then_some(result.as_ref())),
                _ => None,
            };
            if let Some(child) = child {
                if len == frames.len() {
                    return None;
                }
                frames[len - 1] = Some((ty, next.checked_add(1)?));
                frames[len] = Some((child, 0));
                len += 1;
            } else {
                frames[len - 1] = None;
                len -= 1;
            }
        }
        Some(bytes)
    }
}
impl ClonePayload for StagedCopyResultSource {
    fn clone_payload_bytes(&self) -> Option<usize> {
        match self {
            Self::Body { instance, .. } => instance.clone_payload_bytes(),
            Self::TryResidual {
                source_instance,
                target_instance,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                ..
            } => {
                let mut bytes = source_instance
                    .clone_payload_bytes()?
                    .checked_add(target_instance.clone_payload_bytes()?)?;
                for id in [result, ok_case, ok_field, err_case, err_field] {
                    bytes = bytes.checked_add(id.clone_payload_bytes()?)?;
                }
                Some(bytes)
            }
            Self::TryOptionNone {
                source_instance,
                target_instance,
                option,
                some_case,
                some_field,
                none_case,
                ..
            } => {
                let mut bytes = source_instance
                    .clone_payload_bytes()?
                    .checked_add(target_instance.clone_payload_bytes()?)?;
                for id in [option, some_case, some_field, none_case] {
                    bytes = bytes.checked_add(id.clone_payload_bytes()?)?;
                }
                Some(bytes)
            }
        }
    }
}
impl ClonePayload for Observations {
    fn clone_payload_bytes(&self) -> Option<usize> {
        self.parts()
            .1
            .len()
            .checked_mul(size_of::<std::rc::Rc<SkeletonObservation>>())
    }
}
impl ClonePayload for ExprSkeletonPath {
    fn clone_payload_bytes(&self) -> Option<usize> {
        self.observations
            .clone_payload_bytes()?
            .checked_add(self.owned_source.clone_payload_bytes()?)
    }
}
impl ClonePayload for SkeletonPath {
    fn clone_payload_bytes(&self) -> Option<usize> {
        self.observations.clone_payload_bytes()
    }
}

/// Rc::new allocates its two counters and the actual observation together.
#[repr(C)]
struct RcBacking<T> {
    strong: std::cell::Cell<usize>,
    weak: std::cell::Cell<usize>,
    value: T,
}
pub(super) fn rc_backing_bytes<T>() -> usize {
    size_of::<RcBacking<T>>()
}
pub(super) fn observation_rc_bytes() -> usize {
    rc_backing_bytes::<SkeletonObservation>()
}
pub(super) trait IntoReplay<T>: Into<T> {
    fn conversion_bytes() -> usize;
}
impl<T> IntoReplay<T> for T {
    fn conversion_bytes() -> usize {
        0
    }
}
impl IntoReplay<std::rc::Rc<SkeletonObservation>> for SkeletonObservation {
    fn conversion_bytes() -> usize {
        observation_rc_bytes()
    }
}

pub(super) fn conversion_bytes<T, V: IntoReplay<T>>(_: &V) -> usize {
    V::conversion_bytes()
}
