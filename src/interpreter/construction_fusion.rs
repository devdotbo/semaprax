//! Invocation-held interpreter range state. No source admission or selector.
//! The selected factory must bind exact current validated source/HIR before
//! installing a Plan; ordinary evaluators have no plan and allocate no state.
use super::{Flow, Value};
mod carriers;
pub(crate) mod selected;
use crate::construction_fusion::domain::owner_core::{Child, Core, Loan, Receipt, Runtime};
use crate::construction_fusion::domain::{Issuer, MAX_VIEWS, ModuleIdentity, Refusal};
use crate::hir::ResolvedExpr;
pub(super) use carriers::{Backing, BorrowedSliceValue, OwnedBytesValue, WholeBacking};
use std::alloc::Layout;

pub(super) struct Plan<'a> {
    producer: &'a ResolvedExpr,
    push: &'a ResolvedExpr,
    rows: usize,
    width: usize,
}
// No production constructor: only a full selected validation owner may install
// this plan in the eventual factory. A raw descriptor cannot construct it.
#[derive(Default)]
pub(super) struct Registry<'a> {
    plan: Option<Plan<'a>>,
    state: Option<Box<State>>,
    failed: bool,
}
#[derive(Debug, PartialEq)]
pub(super) struct Lease {
    context: usize,
    index: usize,
    receipt: Receipt,
    owner: bool,
}
impl Clone for Lease {
    fn clone(&self) -> Self {
        Self {
            context: self.context,
            index: self.index,
            receipt: self.receipt,
            owner: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct View {
    context: usize,
    index: usize,
    nonce: u64,
}
struct ActiveView {
    loan: Loan<'static>,
    scope: u64,
    nonce: u64,
}
struct State {
    core: Core<'static, Allocator>,
    children: Vec<Child<'static>>,
    views: [Option<ActiveView>; MAX_VIEWS],
    scopes: [u64; 513],
    depth: usize,
    next: u64,
    issued: usize,
    published: usize,
}
struct Allocator;
impl Runtime for Allocator {
    type Owner = Box<[u8]>;
    fn reserve_metadata(&mut self, layout: Layout) -> Result<(), Refusal> {
        if crate::bounded_output::active_remaining().is_none()
            || !crate::bounded_output::reserve_active_required(layout.size())
        {
            return Err(Refusal::Metadata);
        }
        Ok(())
    }
    // Language clone uses Evaluator's existing event/payload fields. This
    // allocator cannot mint a second allowance through Core::deep_clone.
    fn charge_clone(&mut self, _: usize) -> Result<(), Refusal> {
        Err(Refusal::Phase)
    }
    fn allocate(&mut self, extent: usize) -> Self::Owner {
        vec![0; extent].into_boxed_slice()
    }
    fn bytes(owner: &Self::Owner) -> &[u8] {
        owner
    }
    fn bytes_mut(owner: &mut Self::Owner) -> &mut [u8] {
        owner
    }
    fn release(&mut self, owner: Self::Owner) {
        drop(owner);
    }
}
fn guard(_: Refusal) -> Flow {
    Flow::Guard("fused interpreter context authority refused")
}
impl Registry<'_> {
    fn state(&self, context: usize) -> Result<&State, Flow> {
        let state = self
            .state
            .as_deref()
            .ok_or(Flow::Guard("fused context is absent"))?;
        if std::ptr::from_ref(state) as usize != context {
            return Err(guard(Refusal::Identity));
        }
        Ok(state)
    }
    fn state_mut(&mut self, context: usize) -> Result<&mut State, Flow> {
        self.state(context)?;
        Ok(self.state.as_deref_mut().unwrap())
    }
    fn checked_child(state: &State, lease: &Lease) -> Result<usize, Flow> {
        let child = state
            .children
            .get(lease.index)
            .ok_or_else(|| guard(Refusal::Identity))?;
        state
            .core
            .authenticate_receipt(child, lease.receipt)
            .map_err(guard)
    }
    pub(super) fn produce(
        &mut self,
        expression: &ResolvedExpr,
        width: u64,
        events: &mut u32,
        payload: &mut u64,
    ) -> Result<Option<OwnedBytesValue>, Flow> {
        let result = self.produce_inner(expression, width, events, payload);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn produce_inner(
        &mut self,
        expression: &ResolvedExpr,
        width: u64,
        events: &mut u32,
        payload: &mut u64,
    ) -> Result<Option<OwnedBytesValue>, Flow> {
        let Some(plan) = &self.plan else {
            return Ok(None);
        };
        if !std::ptr::eq(plan.producer, expression) {
            return Ok(None);
        }
        if self.failed || usize::try_from(width).ok() != Some(plan.width) {
            self.failed = true;
            return Err(guard(Refusal::Identity));
        }
        if self.state.is_none() {
            let extent = plan
                .rows
                .checked_mul(plan.width)
                .ok_or_else(|| guard(Refusal::Range))?;
            let count = events
                .checked_add(1)
                .filter(|n| *n <= crate::byte_data_capacity::MAX_BYTES_COPY_SITES)
                .ok_or_else(|| guard(Refusal::Metadata))?;
            let total = payload
                .checked_add(extent as u64)
                .filter(|n| *n <= crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES)
                .ok_or_else(|| guard(Refusal::Metadata))?;
            let mut allocator = Allocator;
            // Registry box, exact payload, all core tables and stable holders
            // are charged to the SAME active physical account before allocation.
            allocator
                .reserve_metadata(
                    Layout::array::<u8>(
                        std::mem::size_of::<State>()
                            - std::mem::size_of::<Core<'static, Allocator>>()
                            - std::mem::size_of::<Vec<Child<'static>>>(),
                    )
                    .map_err(|_| guard(Refusal::Metadata))?,
                )
                .map_err(guard)?;
            allocator
                .reserve_metadata(
                    Layout::array::<u8>(extent).map_err(|_| guard(Refusal::Metadata))?,
                )
                .map_err(guard)?;
            let mut preparation =
                Core::<Allocator>::prepare_owned_metadata(plan.rows, allocator).map_err(guard)?;
            let backing = preparation.allocate(extent);
            let (core, children) = Core::partition_owned_prepared(
                backing,
                plan.rows,
                plan.width,
                Issuer::new(),
                ModuleIdentity::new(),
                preparation,
            )
            .map_err(|(error, _)| guard(error))?;
            self.state = Some(Box::new(State {
                core,
                children,
                views: std::array::from_fn(|_| None),
                scopes: [0; 513],
                depth: 0,
                next: 1,
                issued: 0,
                published: 0,
            }));
            *events = count;
            *payload = total;
        }
        let state = self.state.as_deref_mut().unwrap();
        if state.issued != state.published || state.issued >= state.children.len() {
            self.failed = true;
            return Err(guard(Refusal::Unpublished));
        }
        let index = state.issued;
        state.issued += 1;
        let lease = Lease {
            context: std::ptr::from_ref(&*state) as usize,
            index,
            receipt: state.children[index].receipt(),
            owner: true,
        };
        Ok(Some(OwnedBytesValue {
            allocation: *events,
            bytes: Backing::Leased(lease),
        }))
    }
    pub(super) fn publish(&mut self, expression: &ResolvedExpr, value: &Value) -> Result<(), Flow> {
        if !self
            .plan
            .as_ref()
            .is_some_and(|plan| std::ptr::eq(plan.push, expression))
        {
            return Ok(());
        }
        let mut found = None;
        if let Value::Record(record) = value {
            for value in record.fields.values() {
                if let Value::Bytes(bytes) = value {
                    if let Backing::Leased(lease) = &bytes.bytes {
                        if found.replace(lease).is_some() {
                            return Err(guard(Refusal::Identity));
                        }
                    }
                }
            }
        }
        self.publish_lease(expression, found.ok_or_else(|| guard(Refusal::Identity))?)
    }
    fn publish_lease(&mut self, expression: &ResolvedExpr, lease: &Lease) -> Result<(), Flow> {
        if !self
            .plan
            .as_ref()
            .is_some_and(|plan| std::ptr::eq(plan.push, expression))
        {
            return Err(guard(Refusal::Identity));
        }
        let state = self
            .state
            .as_deref_mut()
            .ok_or_else(|| guard(Refusal::Phase))?;
        if self.failed || state.issued != state.published + 1 {
            return Err(guard(Refusal::Unpublished));
        }
        if !lease.owner
            || lease.index != state.published
            || lease.context != std::ptr::from_ref(&*state) as usize
        {
            return Err(guard(Refusal::Identity));
        }
        Self::checked_child(state, lease)?;
        state.published += 1;
        Ok(())
    }
    pub(super) fn len(&self, lease: &Lease) -> Result<usize, Flow> {
        Self::checked_child(self.state(lease.context)?, lease)
    }
    pub(super) fn borrow(&mut self, lease: &Lease) -> Result<View, Flow> {
        if self.failed {
            return Err(guard(Refusal::Phase));
        }
        let state = self.state_mut(lease.context)?;
        if state.published != state.children.len() {
            return Err(guard(Refusal::Phase));
        }
        let length = Self::checked_child(state, lease)?;
        let index = state
            .views
            .iter()
            .position(Option::is_none)
            .ok_or_else(|| guard(Refusal::Metadata))?;
        let nonce = state.next;
        let next = nonce
            .checked_add(1)
            .ok_or_else(|| guard(Refusal::Exhausted))?;
        let loan = state
            .core
            .borrow(&state.children[lease.index], 0..length)
            .map_err(guard)?;
        state.views[index] = Some(ActiveView {
            loan,
            scope: state.scopes[state.depth],
            nonce,
        });
        state.next = next;
        Ok(View {
            context: lease.context,
            index,
            nonce,
        })
    }
    fn receive<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T, Flow>,
    ) -> Result<T, Flow> {
        if self.failed {
            return Err(guard(Refusal::Phase));
        }
        let result = operation(self);
        self.failed |= result.is_err();
        result
    }
    pub(super) fn read<T>(
        &mut self,
        view: View,
        start: usize,
        end: usize,
        consumer: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, Flow> {
        self.receive(|registry| {
            let state = registry.state_mut(view.context)?;
            let held = state
                .views
                .get(view.index)
                .and_then(Option::as_ref)
                .ok_or_else(|| guard(Refusal::Identity))?;
            if held.nonce != view.nonce {
                return Err(guard(Refusal::Identity));
            }
            state
                .core
                .read_range(&held.loan, start..end, consumer)
                .map_err(guard)
        })
    }
    fn read_pair<T>(
        &mut self,
        left: View,
        right: View,
        consumer: impl FnOnce(&[u8], &[u8]) -> T,
    ) -> Result<T, Flow> {
        self.receive(|registry| {
            let state = registry.state_mut(left.context)?;
            if right.context != left.context {
                return Err(guard(Refusal::Identity));
            }
            let held_left = state
                .views
                .get(left.index)
                .and_then(Option::as_ref)
                .filter(|held| held.nonce == left.nonce)
                .ok_or_else(|| guard(Refusal::Identity))?;
            let held_right = state
                .views
                .get(right.index)
                .and_then(Option::as_ref)
                .filter(|held| held.nonce == right.nonce)
                .ok_or_else(|| guard(Refusal::Identity))?;
            state
                .core
                .read_pair(&held_left.loan, &held_right.loan, consumer)
                .map_err(guard)
        })
    }
    pub(super) fn read_owner<T>(
        &mut self,
        lease: &Lease,
        consumer: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, Flow> {
        self.receive(|registry| {
            let state = registry.state_mut(lease.context)?;
            let length = Self::checked_child(state, lease)?;
            let mut loan = state
                .core
                .borrow(&state.children[lease.index], 0..length)
                .map_err(guard)?;
            let result = state.core.read(&loan, consumer).map_err(guard);
            let cleanup = state.core.end_borrow(&mut loan).map_err(guard);
            match result {
                Err(error) => Err(error),
                Ok(value) => {
                    cleanup?;
                    Ok(value)
                }
            }
        })
    }
    pub(super) fn store(
        &mut self,
        lease: &mut Lease,
        index: usize,
        bytes: &[u8],
    ) -> Result<(), Flow> {
        if self.failed {
            return Err(guard(Refusal::Phase));
        }
        if !lease.owner {
            return Err(guard(Refusal::Identity));
        }
        let state = self.state_mut(lease.context)?;
        Self::checked_child(state, lease)?;
        state
            .core
            .store_range(&mut state.children[lease.index], index, bytes)
            .map_err(guard)?;
        lease.receipt = state.children[lease.index].receipt();
        Ok(())
    }
    pub(super) fn drop_lease(&mut self, lease: &mut Lease) -> Result<(), Flow> {
        if !lease.owner {
            return Ok(());
        }
        let state = self.state_mut(lease.context)?;
        Self::checked_child(state, lease)?;
        state
            .core
            .drop_owner(&mut state.children[lease.index])
            .map_err(guard)?;
        lease.owner = false;
        Ok(())
    }
    pub(super) fn enter(&mut self) -> Result<Option<u64>, Flow> {
        let Some(state) = self.state.as_deref_mut() else {
            return Ok(None);
        };
        let depth = state
            .depth
            .checked_add(1)
            .filter(|n| *n < state.scopes.len())
            .ok_or_else(|| guard(Refusal::Metadata))?;
        let scope = state.next;
        state.next = scope
            .checked_add(1)
            .ok_or_else(|| guard(Refusal::Exhausted))?;
        state.depth = depth;
        state.scopes[depth] = scope;
        Ok(Some(scope))
    }
    pub(super) fn leave(&mut self, scope: Option<u64>) -> Result<(), Flow> {
        let Some(scope) = scope else {
            return Ok(());
        };
        let state = self
            .state
            .as_deref_mut()
            .ok_or_else(|| guard(Refusal::Identity))?;
        if state.scopes[state.depth] != scope || state.depth == 0 {
            return Err(guard(Refusal::Identity));
        }
        for view in &mut state.views {
            if view.as_ref().is_some_and(|view| view.scope == scope) {
                let mut view = view.take().unwrap();
                state.core.end_borrow(&mut view.loan).map_err(guard)?;
            }
        }
        state.depth -= 1;
        Ok(())
    }
}
impl Registry<'_> {
    pub(super) fn slice(
        &mut self,
        bytes: &OwnedBytesValue,
        root: super::ValueId,
    ) -> Result<BorrowedSliceValue, Flow> {
        match &bytes.bytes {
            Backing::Ordinary(bytes) => Ok(BorrowedSliceValue::whole(root, bytes.clone())),
            Backing::Whole(_) | Backing::Partition(_) | Backing::PartitionView(_) => Err(
                Flow::Guard("partition owner requires an ordinary checked loan receiver"),
            ),
            Backing::Leased(lease) => {
                let end = self.len(lease)?;
                let view = self.borrow(lease)?;
                Ok(BorrowedSliceValue {
                    invocation_root: root.into(),
                    backing: None,
                    start: 0,
                    end,
                    lease: Some(view),
                    partition: None,
                })
            }
        }
    }
}

impl Registry<'_> {
    pub(super) fn preflight_value(&self, value: &Value) -> Result<(), Flow> {
        if matches!(value, Value::PartitionVec(_) | Value::PartitionIter(_)) {
            return Err(Flow::Guard(
                "partition container requires its canonical receiving registry",
            ));
        }
        if matches!(value, Value::Bytes(bytes) if matches!(bytes.bytes, Backing::Whole(_) | Backing::Partition(_) | Backing::PartitionView(_)))
        {
            return Err(Flow::Guard(
                "partition owner requires its ordinary canonical receiver",
            ));
        }
        if self.state.is_none() {
            return Ok(());
        }
        match value {
            Value::Bytes(bytes) => {
                if let Backing::Leased(lease) = &bytes.bytes {
                    let state = self.state(lease.context)?;
                    Self::checked_child(state, lease)?;
                    state
                        .core
                        .check_move(&state.children[lease.index])
                        .map_err(guard)?;
                }
            }
            Value::Record(record) => {
                for value in record.fields.values() {
                    self.preflight_value(value)?;
                }
            }
            Value::Vec(vector) => {
                for value in &vector.values {
                    self.preflight_value(value)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    pub(super) fn settle_value(&mut self, value: &mut Value) -> Result<(), Flow> {
        if matches!(value, Value::PartitionVec(_) | Value::PartitionIter(_)) {
            return Err(Flow::Guard(
                "partition container requires its canonical receiving registry",
            ));
        }
        if matches!(value, Value::Bytes(bytes) if matches!(bytes.bytes, Backing::Whole(_) | Backing::Partition(_) | Backing::PartitionView(_)))
        {
            return Err(Flow::Guard(
                "partition owner requires its ordinary canonical receiver",
            ));
        }
        if self.state.is_none() {
            return Ok(());
        }
        match value {
            Value::Bytes(bytes) => {
                if let Backing::Leased(lease) = &mut bytes.bytes {
                    self.drop_lease(lease)?;
                }
            }
            Value::Record(_) | Value::Variant(_) | Value::Vec(_) if self.contains_lease(value) => {
                return Err(Flow::Guard(
                    "private aggregate settlement requires exact typed leaf order",
                ));
            }
            _ => {}
        }
        Ok(())
    }
    fn contains_lease(&self, value: &Value) -> bool {
        match value {
            Value::Bytes(bytes) => matches!(bytes.bytes, Backing::Leased(_)),
            Value::Record(record) => record
                .fields
                .values()
                .any(|value| self.contains_lease(value)),
            Value::Variant(variant) => variant
                .fields
                .values()
                .any(|value| self.contains_lease(value)),
            Value::Vec(vector) => vector.values.iter().any(|value| self.contains_lease(value)),
            _ => false,
        }
    }
    fn end_view(&mut self, view: View) -> Result<(), Flow> {
        let state = self.state_mut(view.context)?;
        let slot = state
            .views
            .get_mut(view.index)
            .ok_or_else(|| guard(Refusal::Identity))?;
        if !slot.as_ref().is_some_and(|held| held.nonce == view.nonce) {
            return Err(guard(Refusal::Identity));
        }
        let mut held = slot.take().unwrap();
        state.core.end_borrow(&mut held.loan).map_err(guard)
    }
    pub(super) fn compare_bytes(
        &mut self,
        a: &OwnedBytesValue,
        b: &OwnedBytesValue,
    ) -> Result<std::cmp::Ordering, Flow> {
        if matches!(
            a.bytes,
            Backing::Whole(_) | Backing::Partition(_) | Backing::PartitionView(_)
        ) || matches!(
            b.bytes,
            Backing::Whole(_) | Backing::Partition(_) | Backing::PartitionView(_)
        ) {
            return Err(Flow::Guard(
                "partition bytes require an ordinary paired receiving registry",
            ));
        }
        let av = match &a.bytes {
            Backing::Whole(_) | Backing::Partition(_) | Backing::PartitionView(_) => {
                return Err(Flow::Guard("closed partition comparison"));
            }
            Backing::Ordinary(_) => None,
            Backing::Leased(lease) => Some(self.borrow(lease)?),
        };
        let bv = match &b.bytes {
            Backing::Whole(_) | Backing::Partition(_) | Backing::PartitionView(_) => {
                return Err(Flow::Guard("closed partition comparison"));
            }
            Backing::Ordinary(_) => None,
            Backing::Leased(lease) => match self.borrow(lease) {
                Ok(view) => Some(view),
                Err(error) => {
                    if let Some(view) = av {
                        let _ = self.end_view(view);
                    }
                    return Err(error);
                }
            },
        };
        let result = (|| match (&a.bytes, &b.bytes, av, bv) {
            (Backing::Ordinary(left), Backing::Ordinary(right), _, _) => Ok(left.cmp(right)),
            (Backing::Ordinary(left), Backing::Leased(lease), _, Some(view)) => {
                self.read(view, 0, self.len(lease)?, |right| left.as_ref().cmp(right))
            }
            (Backing::Leased(lease), Backing::Ordinary(right), Some(view), _) => {
                self.read(view, 0, self.len(lease)?, |left| left.cmp(right.as_ref()))
            }
            (Backing::Leased(_), Backing::Leased(_), Some(left_view), Some(right_view)) => {
                self.read_pair(left_view, right_view, <[u8]>::cmp)
            }
            _ => Err(guard(Refusal::Identity)),
        })();
        // Settle both temporary loans, retaining the original selected error.
        let right_cleanup = bv.map_or(Ok(()), |view| self.end_view(view));
        let left_cleanup = av.map_or(Ok(()), |view| self.end_view(view));
        match result {
            Err(error) => Err(error),
            Ok(order) => {
                right_cleanup?;
                left_cleanup?;
                Ok(order)
            }
        }
    }
}
impl super::Evaluator<'_> {
    pub(super) fn settle_fused_environment(
        &mut self,
        environment: &mut super::Environment,
        base: usize,
    ) -> Result<(), Flow> {
        if self.fused.state.is_none() {
            return Ok(());
        }
        for (_, value) in environment.bindings[base..].iter_mut().rev() {
            self.fused.settle_value(value)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
