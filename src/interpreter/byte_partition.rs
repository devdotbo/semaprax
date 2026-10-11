//! Safe invocation-held ordinary partition consumers. No Value/source installer.
//! Internal returns and popped children keep scoped handles into this exact
//! registry. Public copy-out must finish before its invocation owner drops it.
use crate::construction_fusion::domain::owner_core::{Child, Core, Loan, Receipt, Runtime};
use crate::construction_fusion::{
    domain::{Issuer, ModuleIdentity, Refusal},
    partition::{operation, Preparation},
};
use std::{
    alloc::Layout,
    sync::atomic::{AtomicU64, Ordering},
};
pub(crate) mod authority;
pub(crate) mod loans;
static NEXT_INVOCATION: AtomicU64 = AtomicU64::new(1);
/// Implemented only by the actual evaluator account adapter. The handle must
/// borrow/share the SAME monotonic invocation account without allocation or
/// reset; it is not a second allowance. Language code cannot supply this trait.
pub(crate) trait InvocationRuntime: Runtime {
    fn held_account(&self) -> Self;
}
struct State<R: Runtime> {
    core: Core<'static, R>,
    children: Vec<Child<'static>>,
}
/// Affine private receipt: no Clone, serializer, pointer adapter, or public ABI.
pub(crate) struct Owner {
    invocation: u64,
    context: usize,
    child: usize,
    receipt: Receipt,
}
pub(crate) struct View {
    invocation: u64,
    context: usize,
    loan: Loan<'static>,
}
pub(crate) struct Registry<R: InvocationRuntime> {
    runtime: R,
    invocation: u64,
    contexts: Vec<Box<State<R>>>,
    header_paid: bool,
    failed: Option<Refusal>,
}
impl<R: InvocationRuntime> Registry<R> {
    pub(crate) fn new(runtime: R) -> Result<Self, Refusal> {
        let invocation = NEXT_INVOCATION
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| Refusal::Exhausted)?;
        Ok(Self {
            runtime,
            invocation,
            contexts: Vec::new(),
            header_paid: false,
            failed: None,
        })
    }
    fn select<T>(&mut self, result: Result<T, Refusal>) -> Result<T, Refusal> {
        if let Err(error) = result {
            if self.failed.is_none() {
                self.failed = Some(error);
            }
        }
        result
    }
    fn active(&self) -> Result<(), Refusal> {
        self.failed.map_or(Ok(()), Err)
    }
    fn preparation_failure(&mut self, error: Preparation) -> Preparation {
        if let Preparation::Authority(refusal) = error {
            let _ = self.select::<()>(Err(refusal));
        }
        error
    }
    fn state(&mut self, invocation: u64, context: usize) -> Result<&mut State<R>, Refusal> {
        if invocation != self.invocation {
            return Err(Refusal::Identity);
        }
        self.contexts
            .get_mut(context)
            .map(Box::as_mut)
            .ok_or(Refusal::Identity)
    }
    /// Core receives only a handle to the held caller account; no replacement
    /// runtime can be supplied at this operation.
    /// Checks/reservations precede ANY context/table/carrier allocation. On every
    /// precommit refusal the identical owner is returned; no partial publication.
    pub(crate) fn partition(
        &mut self,
        owner: R::Owner,
        count: u64,
        width: u64,
    ) -> Result<Vec<Owner>, (Preparation, R::Owner)> {
        self.partition_with_capacity(owner, count, width, None)
    }
    fn partition_with_capacity(
        &mut self,
        owner: R::Owner,
        count: u64,
        width: u64,
        capacity: Option<&authority::LoanCapacity<'_>>,
    ) -> Result<Vec<Owner>, (Preparation, R::Owner)> {
        if let Err(error) = self.active() {
            return Err((Preparation::Authority(error), owner));
        }
        let geometry = match operation::prepare(R::bytes(&owner).len() as u64, count, width) {
            Ok(g) => g,
            Err(error) => return Err((self.preparation_failure(error), owner)),
        };
        let next_capacity = if self.contexts.len() == self.contexts.capacity() {
            match self.contexts.capacity().checked_mul(2).map(|n| n.max(1)) {
                Some(n) => Some(n),
                None => {
                    return Err((
                        self.preparation_failure(Preparation::Authority(Refusal::Metadata)),
                        owner,
                    ))
                }
            }
        } else {
            None
        };
        let mut runtime = self.runtime.held_account();
        let reserve = (|| {
            if !self.header_paid {
                runtime.reserve_metadata(Layout::new::<Self>())?;
            }
            if let Some(capacity) = next_capacity {
                // Full replacement is charged while the old backing is live.
                runtime.reserve_metadata(
                    Layout::array::<Box<State<R>>>(capacity).map_err(|_| Refusal::Metadata)?,
                )?;
            }
            runtime.reserve_metadata(Layout::new::<State<R>>())?;
            runtime.reserve_metadata(
                Layout::array::<Owner>(geometry.count).map_err(|_| Refusal::Metadata)?,
            )?;
            runtime.reserve_metadata(Layout::new::<Vec<Owner>>())?;
            Ok::<(), Refusal>(())
        })();
        if let Err(error) = reserve {
            self.select::<()>(Err(error)).ok();
            return Err((Preparation::Authority(error), owner));
        }
        // Empty + maximal semantic usize needs no host-addressable width.
        let host_width = if geometry.count == 0 {
            1
        } else {
            match usize::try_from(width) {
                Ok(n) => n,
                Err(_) => {
                    return Err((
                        self.preparation_failure(Preparation::Authority(Refusal::Range)),
                        owner,
                    ))
                }
            }
        };
        let prepared = match capacity {
            Some(capacity) => Core::partition_owned_ordinary(
                owner,
                geometry.count,
                host_width,
                Issuer::new(),
                ModuleIdentity::new(),
                runtime,
                capacity,
            ),
            None => Core::partition_owned(
                owner,
                geometry.count,
                host_width,
                Issuer::new(),
                ModuleIdentity::new(),
                runtime,
            ),
        };
        let (core, children) = match prepared {
            Ok(value) => value,
            Err((error, owner)) => {
                self.select::<()>(Err(error)).ok();
                return Err((Preparation::Authority(error), owner));
            }
        };
        self.header_paid = true;
        if let Some(capacity) = next_capacity {
            let mut replacement = Vec::new();
            replacement
                .try_reserve_exact(capacity)
                .expect("partition registry allocation invariant failure");
            assert_eq!(replacement.capacity(), capacity);
            replacement.append(&mut self.contexts);
            self.contexts = replacement;
        }
        let context = self.contexts.len();
        let mut output = Vec::new();
        output
            .try_reserve_exact(geometry.count)
            .expect("partition carrier allocation invariant failure");
        assert_eq!(output.capacity(), geometry.count);
        for (child, value) in children.iter().enumerate() {
            output.push(Owner {
                invocation: self.invocation,
                context,
                child,
                receipt: value.receipt(),
            });
        }
        self.contexts.push(Box::new(State { core, children }));
        Ok(output)
    }
    fn apply<T>(
        &mut self,
        owner: &Owner,
        cleanup: bool,
        f: impl FnOnce(&mut Core<'static, R>, &mut Child<'static>) -> Result<T, Refusal>,
    ) -> Result<T, Refusal> {
        let result = (|| {
            if !cleanup {
                self.active()?;
            }
            let state = self.state(owner.invocation, owner.context)?;
            let child = state
                .children
                .get_mut(owner.child)
                .ok_or(Refusal::Identity)?;
            state.core.authenticate_receipt(child, owner.receipt)?;
            f(&mut state.core, child)
        })();
        self.select(result)
    }
    pub(crate) fn moved(&mut self, owner: &mut Owner) -> Result<(), Refusal> {
        owner.receipt = self.apply(owner, false, |core, child| {
            core.move_owner(child)?;
            Ok(child.receipt())
        })?;
        Ok(())
    }
    pub(crate) fn store(
        &mut self,
        owner: &mut Owner,
        index: usize,
        bytes: &[u8],
    ) -> Result<(), Refusal> {
        owner.receipt = self.apply(owner, false, |core, child| {
            core.store_range(child, index, bytes)?;
            Ok(child.receipt())
        })?;
        Ok(())
    }
    pub(crate) fn deep_clone(&mut self, owner: &Owner) -> Result<R::Owner, Refusal> {
        self.apply(owner, false, |core, child| core.deep_clone(child))
    }
    /// Public boundary receives an ordinary fresh owner only after provider
    /// settlement. No interior pointer or registry handle crosses the boundary.
    /// Failed settlement releases the staged copy and preserves its refusal;
    /// its ordinary event/payload debit is never refunded.
    pub(crate) fn copy_out_and_settle(&mut self, owner: &mut Owner) -> Result<R::Owner, Refusal> {
        let copied = self.deep_clone(owner)?;
        match self.settle(owner) {
            Ok(()) => Ok(copied),
            Err(error) => {
                self.runtime.release(copied);
                Err(error)
            }
        }
    }
    pub(crate) fn borrow(&mut self, owner: &Owner) -> Result<View, Refusal> {
        let loan = self.apply(owner, false, |core, child| {
            let extent = core.authenticate_receipt(child, owner.receipt)?;
            core.borrow(child, 0..extent)
        })?;
        Ok(View {
            invocation: self.invocation,
            context: owner.context,
            loan,
        })
    }
    pub(crate) fn read<T>(
        &mut self,
        view: &View,
        f: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, Refusal> {
        let result = (|| {
            self.active()?;
            self.state(view.invocation, view.context)?
                .core
                .read(&view.loan, f)
        })();
        self.select(result)
    }
    pub(crate) fn read_range<T>(
        &mut self,
        view: &View,
        start: usize,
        end: usize,
        f: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, Refusal> {
        let result = (|| {
            self.active()?;
            self.state(view.invocation, view.context)?
                .core
                .read_range(&view.loan, start..end, f)
        })();
        self.select(result)
    }
    pub(crate) fn end_view(&mut self, view: &mut View) -> Result<(), Refusal> {
        let result = self
            .state(view.invocation, view.context)
            .and_then(|state| state.core.end_borrow(&mut view.loan));
        self.select(result)
    }
    /// Exact canonical leaf action, never a mixed-container pre-finalizer sweep.
    pub(crate) fn settle(&mut self, owner: &mut Owner) -> Result<(), Refusal> {
        self.apply(owner, true, |core, child| core.drop_owner(child))
    }
}
#[cfg(test)]
mod tests;
