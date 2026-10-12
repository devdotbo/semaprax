//! Private checked UTF8 receiving; ordinary System String execution is unchanged.
//! Payload/control construction belongs to the supplied physical provider. This
//! safe boundary owns its returned Arc backing, never reconstructs String from
//! a foreign pointer, and keeps that same provider/module alive through Drop.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "private checked interpreter payload receiver awaits complete metadata and public SDK joins"
    )
)]
use super::*;
use crate::cleanup_plan::{checked_allocation::Validated, AllocationCase};
use crate::diagnostic::Diagnostic;
mod operations;
#[cfg(test)]
mod tests;

/// The caller supplies a strongly owned physical provider, including its module
/// lifetime. Every successful backing must be newly unique and immutable. Its
/// own payload AND control-storage acquisition must be fallible/prefunded; this
/// receiver never disguises an engine Arc::new as a nullable request.
pub(crate) trait Utf8Provider: std::fmt::Debug + Send + Sync {
    fn allocate(&self, pieces: &[&str]) -> Result<Arc<[u8]>, AllocationCase>;
}
#[derive(Clone, Debug)]
pub(super) struct Backing {
    // Drop backing first, retaining its provider/module throughout destruction.
    bytes: Arc<[u8]>,
    provider: Arc<dyn Utf8Provider>,
}
impl PartialEq for Backing {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.provider, &other.provider) && self.bytes == other.bytes
    }
}
impl AsRef<[u8]> for Backing {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}
#[derive(Debug, PartialEq)]
pub(crate) struct OwnedUtf8 {
    backing: Backing,
}
impl OwnedUtf8 {
    pub(crate) fn as_str(&self) -> &str {
        std::str::from_utf8(self.backing.as_ref()).expect("received immutable UTF8 backing")
    }
    pub(in crate::interpreter) fn borrow(&self) -> Backing {
        self.backing.clone()
    }
}

pub(super) struct Invocation<'hir> {
    pub(in crate::interpreter) program: &'hir hir::ResolvedProgram,
    provider: Arc<dyn Utf8Provider>,
    physical: Option<AllocationCase>,
    // Acquired before first physical request; refusal moves its owned receipt.
    reports: [Option<NormalizedStatus>; 2],
}
impl<'hir> Invocation<'hir> {
    fn new(program: &'hir hir::ResolvedProgram, provider: Arc<dyn Utf8Provider>) -> Self {
        // This control/report acquisition is explicit metadata, not a provider
        // payload request. Its funding is an outstanding whole-engine join.
        let reports = [AllocationCase::PayloadRefused, AllocationCase::SizeOverflow].map(|case| {
            Some(
                NormalizedStatus::try_new(
                    crate::cleanup_plan::ALLOCATION_DOMAIN_V1,
                    case.code(),
                    StatusClass::Adapter,
                    Retryability::Known(false),
                )
                .expect("closed allocation status table"),
            )
        });
        Self {
            program,
            provider,
            physical: None,
            reports,
        }
    }
    pub(in crate::interpreter) fn contract_result(
        &self,
        function: &ResolvedFunction,
        value: &OwnedUtf8,
    ) -> Result<OwnedUtf8, Flow> {
        // This is the checked provisional result alias, not an owning source
        // read. Full source/HIR replay independently protects result storage
        // from consumption; clause reads still call clone_value normally.
        if function.ensures.is_empty()
            || function.return_type != ResolvedType::String
            || function.body.ownership != hir::OwnershipMode::Own
            || !self
                .program
                .functions
                .iter()
                .any(|held| std::ptr::eq(held, function))
            || !Arc::ptr_eq(&self.provider, &value.backing.provider)
        {
            return Err(Flow::Guard(
                "checked contract result is not this invocation's current owner",
            ));
        }
        Ok(OwnedUtf8 {
            backing: value.backing.clone(),
        })
    }
    fn materialize(&mut self, pieces: &[&str]) -> Result<OwnedUtf8, Flow> {
        if self.physical.is_some() {
            return Err(Flow::Guard("checked allocation invocation already refused"));
        }
        let length = pieces
            .iter()
            .try_fold(0usize, |total, part| total.checked_add(part.len()));
        let Some(length) = length else {
            return Err(self.refuse(AllocationCase::SizeOverflow));
        };
        let bytes = match self.provider.allocate(pieces) {
            Ok(bytes) => bytes,
            Err(reason) => return Err(self.refuse(reason)),
        };
        if Arc::strong_count(&bytes) != 1 || Arc::weak_count(&bytes) != 0 || bytes.len() != length {
            return Err(Flow::Guard(
                "checked UTF8 provider returned shared or wrong-sized backing",
            ));
        }
        let mut offset = 0;
        for part in pieces {
            if &bytes[offset..offset + part.len()] != part.as_bytes() {
                return Err(Flow::Guard(
                    "checked UTF8 provider returned different requested bytes",
                ));
            }
            offset += part.len();
        }
        Ok(OwnedUtf8 {
            backing: Backing {
                bytes,
                provider: Arc::clone(&self.provider),
            },
        })
    }
    fn refuse(&mut self, reason: AllocationCase) -> Flow {
        self.physical = Some(reason);
        let status = self.reports[reason.code() as usize - 1]
            .take()
            .expect("closed allocation receipt was acquired before execution");
        Flow::Failure(status)
    }
}

pub(crate) struct Projection<'owner, 'source> {
    owner: &'owner Validated<'source>,
}
impl<'owner, 'source> Projection<'owner, 'source> {
    pub(crate) fn receive(owner: &'owner Validated<'source>) -> Result<Self, Diagnostic> {
        owner.replay()?;
        let program = owner.program();
        if !program.function_instances.is_empty() {
            return Err(Diagnostic::io(
                "SPX-B104",
                "checked interpreter generic receiver is not joined",
            ));
        }
        for function in &program.functions {
            if !function.effects.is_empty()
                || !leaf(&function.return_type)
                || function.params.iter().any(|parameter| !leaf(&parameter.ty))
            {
                return Err(Diagnostic::io(
                    "SPX-B104",
                    "checked interpreter carrier receiver is not joined",
                ));
            }
            for root in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                inspect(program, root)?;
                let mut frames = [const { None }; 514];
                frames[0] = Some(hir::lexical_exit::ChildCursor::new(root));
                let mut length = 1;
                while length != 0 {
                    if let Some(child) = frames[length - 1].as_mut().unwrap().next() {
                        inspect(program, child)?;
                        if length == frames.len() {
                            return Err(Diagnostic::io(
                                "SPX-B104",
                                "checked interpreter depth exceeds 512",
                            ));
                        }
                        frames[length] = Some(hir::lexical_exit::ChildCursor::new(child));
                        length += 1;
                    } else {
                        frames[length - 1] = None;
                        length -= 1;
                    }
                }
            }
        }
        Ok(Self { owner })
    }
    /// Actual existing evaluator, with its original fuel/call/postcondition and
    /// failure unwind. Runtime frame/index metadata is still a separate join;
    /// this method claims checked UTF8 payload receiving, not whole-engine OOM.
    pub(super) fn run(&self, provider: Arc<dyn Utf8Provider>, fuel: usize) -> Result<Value, Flow> {
        self.owner
            .replay()
            .map_err(|_| Flow::Guard("checked interpreter projection drift"))?;
        let program = self.owner.program();
        let entry = program
            .functions
            .iter()
            .find(|function| function.id == *self.owner.entry())
            .ok_or(Flow::Guard("checked interpreter entry is not held"))?;
        let mut evaluator = Evaluator::new_prepared(
            FunctionLookup::Checked(program),
            BTreeMap::new(),
            &program.declarations,
            fuel,
            0,
            PreparedCancellation::Never,
        );
        evaluator.checked_allocation = Some(Invocation::new(program, provider));
        evaluator.call_frame(entry, Vec::new(), 0)
    }
}
fn leaf(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::Char
            | ResolvedType::Bool
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::String
            | ResolvedType::Str
    )
}
fn inspect(program: &hir::ResolvedProgram, expression: &ResolvedExpr) -> Result<(), Diagnostic> {
    use crate::string_ops::StringOp as S;
    let joined = leaf(&expression.ty)
        && match &expression.kind {
            ResolvedExprKind::Call {
                callee, instance, ..
            } => {
                instance.is_none()
                    && (matches!(
                        crate::string_ops::by_id(callee.as_str()),
                        Some(
                            S::Take
                                | S::Concat
                                | S::Len
                                | S::IsEmpty
                                | S::StartsWith
                                | S::Contains
                                | S::FromStr
                        )
                    ) || program
                        .functions
                        .iter()
                        .any(|function| function.id == *callee))
            }
            ResolvedExprKind::LiteralFormat { .. }
            | ResolvedExprKind::FunctionReference { .. }
            | ResolvedExprKind::HostCommandCall(_)
            | ResolvedExprKind::NativeRustImportCall(_) => false,
            _ => true,
        };
    if joined {
        Ok(())
    } else {
        Err(Diagnostic::io(
            "SPX-B104",
            "checked interpreter expression receiver is not joined",
        ))
    }
}
