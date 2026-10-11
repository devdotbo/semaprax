//! Private selected executor: only the immutable full-validation owner enters.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "private selected executor awaits three-backend qualification"
    )
)]
//! Ordinary interpreter admission and carriers are unchanged. All backing is
//! held by this invocation; a numeric row/view token cannot escape its frame.
use super::{Backing, Flow, OwnedBytesValue, Plan, Registry, Value, View};
use crate::ast::{BinaryOp, UnaryOp};
use crate::cleanup_plan::{ExitContinuation, StatusLane, StorageId};
use crate::hir::fused_construction::{Role, Selection};
use crate::hir::{
    OwnershipMode, ResolvedExpr as E, ResolvedExprKind as K, ResolvedMatchPattern as Pattern,
    ResolvedStatement as S, ValueId,
};
use std::alloc::Layout;
mod execution;
#[cfg(test)]
mod tests;

const DEPTH: usize = crate::cache_codec::MAX_DEPTH;

/// Deliberately private observation, not the public interpret envelope/profile.
#[derive(Debug)]
pub(crate) struct Outcome {
    pub(crate) result: i64,
    pub(crate) steps: usize,
    pub(crate) byte_events: u32,
    pub(crate) byte_payload: u64,
    pub(crate) string_materializations: u64,
    pub(crate) string_bytes: u64,
}
/// Failure never publishes a result. The selected status remains unchanged
/// while canonical cleanup runs; a cleanup invariant cannot replace it.
#[derive(Debug)]
pub(crate) enum Failure {
    Authority,
    FuelExhausted {
        steps: usize,
        cleanup_complete: bool,
    },
    DepthExceeded {
        steps: usize,
        cleanup_complete: bool,
    },
    Execution {
        status: Option<crate::conformance::NormalizedStatus>,
        steps: usize,
        cleanup_complete: bool,
    },
}
impl Failure {
    fn execution(flow: &Flow, steps: usize) -> Self {
        if matches!(flow, Flow::Exhausted) {
            return Self::FuelExhausted {
                steps,
                cleanup_complete: false,
            };
        }
        if matches!(flow, Flow::DepthExceeded) {
            return Self::DepthExceeded {
                steps,
                cleanup_complete: false,
            };
        }
        Self::Execution {
            status: match flow {
                Flow::Failure(status) => Some(status.clone()),
                _ => None,
            },
            steps,
            cleanup_complete: false,
        }
    }
}

pub(crate) fn execute(
    owner: &crate::hir::selected_fused_candidate::ValidatedFusedSelection<'_>,
    fuel: usize,
) -> Result<Outcome, Failure> {
    with_current(owner, |selection| execute_selection(selection, fuel))?
}

/// Compiler proof joining precedes the runtime requested-layout account.
/// Only the sealed owner enters; no joined/selection authority is public.
pub(crate) fn execute_accounted(
    owner: &crate::hir::selected_fused_candidate::ValidatedFusedSelection<'_>,
    fuel: usize,
    limit: usize,
) -> (Result<Outcome, Failure>, bool, usize) {
    with_current(owner, |selection| {
        crate::bounded_output::with_limit_usage(limit, || execute_selection(selection, fuel))
    })
    .unwrap_or_else(|failure| (Err(failure), false, 0))
}

#[cfg(test)]
pub(crate) fn execute_accounted_observed(
    owner: &crate::hir::selected_fused_candidate::ValidatedFusedSelection<'_>,
    fuel: usize,
    limit: usize,
) -> (
    (
        Result<Outcome, Failure>,
        crate::test_allocation_observer::Metrics,
    ),
    bool,
    usize,
) {
    with_current(owner, |selection| {
        crate::bounded_output::with_limit_usage(limit, || {
            crate::test_allocation_observer::measure(|| execute_selection(selection, fuel))
        })
    })
    .expect("current sealed source owner")
}

fn with_current<R>(
    owner: &crate::hir::selected_fused_candidate::ValidatedFusedSelection<'_>,
    receive: impl FnOnce(&Selection<'_>) -> R,
) -> Result<R, Failure> {
    owner
        .with_joined(|joined| {
            if !joined.still_current(owner.source(), owner.program()) {
                return Err(Failure::Authority);
            }
            Ok(receive(joined.selection()))
        })
        .map_err(|_| Failure::Authority)?
}

fn execute_selection(selection: &Selection<'_>, fuel: usize) -> Result<Outcome, Failure> {
    // Box allocation includes every fixed frame/header before allocation.
    reserve(Layout::new::<Engine<'_, '_>>()).map_err(|flow| Failure::execution(&flow, 0))?;
    let mut engine =
        Box::new(Engine::new(selection, fuel).map_err(|flow| Failure::execution(&flow, 0))?);
    let body = &selection.function().body;
    let result = engine.expression(body, 0);
    let settled = match &result {
        // Fuel and depth exhaustion are out of band: no plan exit, no status.
        Err(Flow::Exhausted | Flow::DepthExceeded) => engine.settle_empty(),
        _ => engine.settle_exit(result.is_ok()),
    };
    match result {
        Ok(Flat::Scalar(Value::Int(result))) if settled.is_ok() => {
            let (string_materializations, string_bytes) =
                (engine.string_materializations, engine.string_bytes);
            Ok(Outcome {
                result,
                steps: engine.steps,
                byte_events: engine.events,
                byte_payload: engine.payload,
                string_materializations,
                string_bytes,
            })
        }
        // Actual engine failure is retained even if settlement refuses.
        Err(flow) => {
            let mut failure = Failure::execution(&flow, engine.steps);
            match &mut failure {
                Failure::Execution {
                    cleanup_complete, ..
                }
                | Failure::FuelExhausted {
                    cleanup_complete, ..
                }
                | Failure::DepthExceeded {
                    cleanup_complete, ..
                } => *cleanup_complete = settled.is_ok(),
                Failure::Authority => {}
            }
            Err(failure)
        }
        Ok(_) => Err(Failure::execution(&refuse(), engine.steps)),
    }
}

fn refuse() -> Flow {
    Flow::Guard("selected flat interpreter authority refused")
}
fn reserve(layout: Layout) -> Result<(), Flow> {
    if crate::bounded_output::active_remaining().is_none()
        || !crate::bounded_output::reserve_active_required(layout.size())
    {
        return Err(refuse());
    }
    Ok(())
}
struct Row {
    priority: i64,
    text: Option<Box<[u8]>>,
    payload: Option<OwnedBytesValue>,
}
#[derive(Clone, Copy)]
struct TextView {
    row: usize,
    context: usize,
    scope: u64,
    generation: u64,
}
#[derive(Clone, Copy)]
enum Span {
    Text(TextView),
    Payload(View, usize),
}
enum Flat {
    Scalar(Value),
    Vector(bool),
    Row,
    Text(TextView),
    Slice(Span),
    Moved,
}
struct Slot<'a> {
    id: &'a ValueId,
    value: Flat,
}
struct Engine<'s, 'a> {
    selection: &'s Selection<'a>,
    registry: Registry<'a>,
    rows: Vec<Row>,
    pending: Option<Row>,
    slots: Vec<Option<Slot<'a>>>,
    len: usize,
    allocated: bool,
    generation: u64,
    staged_vector: bool,
    events: u32,
    payload: u64,
    utf8: super::super::Utf8MaterializationBudget,
    string_materializations: u64,
    string_bytes: u64,
    steps: usize,
    fuel: usize,
    failed_site: Option<&'a E>,
}
impl<'s, 'a> Engine<'s, 'a> {
    fn new(selection: &'s Selection<'a>, fuel: usize) -> Result<Self, Flow> {
        let count = binding_count(&selection.function().body)?;
        reserve(Layout::array::<Option<Slot<'_>>>(count).map_err(|_| refuse())?)?;
        let mut slots = Vec::new();
        slots.try_reserve_exact(count).map_err(|_| refuse())?;
        if slots.capacity() != count {
            return Err(refuse());
        }
        slots.resize_with(count, || None);
        Ok(Self {
            selection,
            registry: Registry {
                plan: Some(Plan {
                    producer: selection.producer(),
                    push: selection.push(),
                    rows: selection.rows() as usize,
                    width: selection.width() as usize,
                }),
                state: None,
                failed: false,
            },
            rows: Vec::new(),
            pending: None,
            slots,
            len: 0,
            allocated: false,
            generation: 1,
            staged_vector: false,
            events: 0,
            payload: 0,
            utf8: super::super::Utf8MaterializationBudget::UnlimitedLegacy,
            string_materializations: 0,
            string_bytes: 0,
            steps: 0,
            fuel,
            failed_site: None,
        })
    }
    fn charge(&mut self) -> Result<(), Flow> {
        if self.steps >= self.fuel {
            return Err(Flow::Exhausted);
        }
        self.steps += 1;
        Ok(())
    }
    fn bind(&mut self, id: &'a ValueId, value: Flat) -> Result<(), Flow> {
        if self.slots[..self.len]
            .iter()
            .flatten()
            .any(|slot| slot.id == id)
        {
            return Err(refuse());
        }
        *self.slots.get_mut(self.len).ok_or_else(refuse)? = Some(Slot { id, value });
        self.len += 1;
        Ok(())
    }
    fn slot(&self, id: &ValueId) -> Result<usize, Flow> {
        self.slots[..self.len]
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|slot| slot.id == id))
            .ok_or_else(refuse)
    }
    fn read(&mut self, id: &ValueId, own: bool) -> Result<Flat, Flow> {
        let index = self.slot(id)?;
        let slot = self.slots[index].as_mut().ok_or_else(refuse)?;
        match &slot.value {
            Flat::Vector(true) if own => Ok(std::mem::replace(&mut slot.value, Flat::Moved)),
            Flat::Vector(_) => Ok(Flat::Vector(false)),
            Flat::Row if own => Ok(std::mem::replace(&mut slot.value, Flat::Moved)),
            Flat::Text(row) => Ok(Flat::Text(*row)),
            Flat::Slice(span) => Ok(Flat::Slice(*span)),
            Flat::Scalar(value) => Ok(Flat::Scalar(copy_scalar(value)?)),
            _ => Err(refuse()),
        }
    }
    fn truncate(&mut self, base: usize) -> Result<(), Flow> {
        while self.len > base {
            let slot = self.slots[self.len - 1].take().ok_or_else(refuse)?;
            if matches!(slot.value, Flat::Vector(true) | Flat::Row) {
                return Err(refuse());
            }
            self.len -= 1;
        }
        Ok(())
    }
    fn bytes<T>(&mut self, span: Span, receive: impl FnOnce(&[u8]) -> T) -> Result<T, Flow> {
        match span {
            Span::Text(view) => {
                let state = self.registry.state(view.context)?;
                if view.generation != self.generation
                    || !state.scopes[..=state.depth].contains(&view.scope)
                {
                    return Err(refuse());
                }
                Ok(receive(
                    self.rows
                        .get(view.row)
                        .and_then(|r| r.text.as_deref())
                        .ok_or_else(refuse)?,
                ))
            }
            Span::Payload(view, length) => self.registry.read(view, 0, length, receive),
        }
    }
    fn allocate(&mut self, expression: &'a E, count: u64) -> Result<Flat, Flow> {
        if !std::ptr::eq(expression, self.selection.allocation())
            || self.allocated
            || count != u64::from(self.selection.rows())
        {
            return Err(refuse());
        }
        // Vec header lives in the already funded Engine. Actual backing uses
        // target Rust Row size/alignment, never the semantic owned-field charge.
        reserve(Layout::array::<Row>(count as usize).map_err(|_| refuse())?)?;
        self.rows
            .try_reserve_exact(count as usize)
            .map_err(|_| refuse())?;
        if self.rows.capacity() != count as usize {
            return Err(refuse());
        }
        self.allocated = true;
        Ok(Flat::Vector(true))
    }
    fn construct(
        &mut self,
        expression: &'a E,
        fields: &'a [crate::hir::ResolvedFieldInitializer],
        depth: usize,
    ) -> Result<Flat, Flow> {
        if !std::ptr::eq(expression, self.selection.constructor()) || self.pending.is_some() {
            return Err(refuse());
        }
        self.pending = Some(Row {
            priority: 0,
            text: None,
            payload: None,
        });
        for (field, role) in fields.iter().zip(self.selection.order()) {
            let outcome = (|| {
                match role {
                    Role::Induction => {
                        let Flat::Scalar(Value::Int(value)) =
                            self.expression(&field.value, depth + 1)?
                        else {
                            return Err(refuse());
                        };
                        self.pending.as_mut().unwrap().priority = value;
                    }
                    Role::Text => {
                        if !std::ptr::eq(&field.value, self.selection.text()) {
                            return Err(refuse());
                        }
                        self.charge()?;
                        let K::String(text) = &field.value.kind else {
                            return Err(refuse());
                        };
                        self.utf8.charge(text.len())?;
                        self.string_materializations = self
                            .string_materializations
                            .checked_add(1)
                            .ok_or_else(refuse)?;
                        self.string_bytes = self
                            .string_bytes
                            .checked_add(u64::try_from(text.len()).map_err(|_| refuse())?)
                            .ok_or_else(refuse)?;
                        reserve(Layout::array::<u8>(text.len()).map_err(|_| refuse())?)?;
                        let mut copy = Vec::new();
                        copy.try_reserve_exact(text.len()).map_err(|_| refuse())?;
                        if copy.capacity() != text.len() {
                            return Err(refuse());
                        }
                        copy.extend_from_slice(text.as_bytes());
                        self.pending.as_mut().unwrap().text = Some(copy.into_boxed_slice());
                    }
                    Role::Payload => {
                        self.charge()?;
                        let K::Call { args, .. } = &field.value.kind else {
                            return Err(refuse());
                        };
                        self.charge()?; // ordinary builtin invocation
                        let [width] = args.as_slice() else {
                            return Err(refuse());
                        };
                        let Flat::Scalar(Value::Usize(width)) =
                            self.expression(width, depth + 1)?
                        else {
                            return Err(refuse());
                        };
                        let bytes = self
                            .registry
                            .produce(&field.value, width, &mut self.events, &mut self.payload)?
                            .ok_or_else(refuse)?;
                        self.pending.as_mut().unwrap().payload = Some(bytes);
                    }
                }
                Ok::<(), Flow>(())
            })();
            if let Err(failure) = outcome {
                self.failed_site.get_or_insert(&field.value);
                return Err(failure);
            }
        }
        Ok(Flat::Row)
    }
    fn publish(&mut self, expression: &'a E, owner: Flat, row: Flat) -> Result<Flat, Flow> {
        if !matches!(owner, Flat::Vector(true))
            || !matches!(row, Flat::Row)
            || self.rows.len() >= self.rows.capacity()
        {
            return Err(refuse());
        }
        let row = self.pending.as_ref().ok_or_else(refuse)?;
        let Some(OwnedBytesValue {
            bytes: Backing::Leased(lease),
            ..
        }) = &row.payload
        else {
            return Err(refuse());
        };
        self.registry.publish_lease(expression, lease)?;
        self.rows.push(self.pending.take().ok_or_else(refuse)?);
        Ok(Flat::Vector(true))
    }
    fn settle_rows(&mut self) -> Result<(), Flow> {
        // This is the selected Vec lifecycle, invoked by an actual canonical
        // whole-vector action: ascending elements, declared field order.
        for row in &mut self.rows {
            for role in self.selection.order() {
                match role {
                    Role::Text => drop(row.text.take()),
                    Role::Payload => {
                        if let Some(mut bytes) = row.payload.take() {
                            let Backing::Leased(lease) = &mut bytes.bytes else {
                                return Err(refuse());
                            };
                            self.registry.drop_lease(lease)?;
                        }
                    }
                    Role::Induction => {}
                }
            }
        }
        self.rows.clear();
        Ok(())
    }
    fn settle_exit(&mut self, success: bool) -> Result<(), Flow> {
        let plan = &self.selection.function().cleanup_plan;
        let mut matching = plan.exits.iter().filter(|exit| match &exit.continuation {
            ExitContinuation::CommitResult {
                source: crate::cleanup_plan::CleanupResultSource::Scalar { expression },
            } => success && *expression == self.selection.function().body.id,
            // Every selected failure is an operation failure; the exact
            // lane, not the expression alone, names its exit.
            ExitContinuation::ReturnFailure { source } => {
                !success
                    && source.lane == StatusLane::OperationFailure
                    && self
                        .failed_site
                        .is_some_and(|site| source.expression == site.id)
            }
            _ => false,
        });
        let exit = matching.next().ok_or_else(refuse)?;
        if matching.next().is_some() {
            return Err(refuse());
        }
        for action in &exit.finalize_in_order {
            if action.active_case.is_some() {
                return Err(refuse());
            }
            match &action.source.storage {
                StorageId::Value(id) if action.source.projections.is_empty() => {
                    if let Ok(index) = self.slot(id) {
                        let slot = self.slots[index].as_mut().ok_or_else(refuse)?;
                        if matches!(slot.value, Flat::Vector(true)) {
                            slot.value = Flat::Moved;
                            self.settle_rows()?;
                        } else if matches!(slot.value, Flat::Row) {
                            return Err(refuse());
                        }
                    }
                }
                StorageId::CallArgument {
                    call,
                    parameter_index: 0,
                    ..
                } if *call == self.selection.push().id
                    && action.source.projections.is_empty()
                    && self.staged_vector =>
                {
                    self.staged_vector = false;
                    self.settle_rows()?;
                }
                _ => {
                    // Staged/partial record cleanup needs the exact leaf action.
                    // It is not a Vec lifecycle and must never use field-order sweep.
                    self.settle_pending_action(action)?;
                }
            }
        }
        if self
            .rows
            .iter()
            .any(|row| row.text.is_some() || row.payload.is_some())
            || self
                .pending
                .as_ref()
                .is_some_and(|row| row.text.is_some() || row.payload.is_some())
        {
            return Err(refuse());
        }
        Ok(())
    }
    /// Fuel or depth exhaustion selects no language status and has no
    /// canonical plan exit, so no finalizer may run for it. Settlement succeeds
    /// only for an independently certified empty ownership state: no Vec
    /// allocation, row, staged row or staged Vec operand, no range context
    /// (hence no owner or loan) and no owner or view in any binding slot.
    /// Anything live leaves cleanup incomplete; the frame's physical Drop is
    /// not canonical cleanup.
    fn settle_empty(&self) -> Result<(), Flow> {
        let held = self
            .slots
            .iter()
            .flatten()
            .any(|slot| !matches!(slot.value, Flat::Scalar(_) | Flat::Moved));
        if self.allocated
            || !self.rows.is_empty()
            || self.pending.is_some()
            || self.staged_vector
            || self.registry.state.is_some()
            || held
        {
            return Err(refuse());
        }
        Ok(())
    }
    fn settle_pending_action(
        &mut self,
        action: &crate::cleanup_plan::FinalizeAction,
    ) -> Result<(), Flow> {
        let Some(row) = &mut self.pending else {
            return Ok(());
        };
        let role = self
            .selection
            .fields()
            .iter()
            .zip(self.selection.order())
            .find(|(field, _)| {
                action.source.projections.as_slice() == std::slice::from_ref(&field.id)
            })
            .map(|(_, role)| role);
        let role = role.or_else(|| match &action.source.storage {
            StorageId::Temporary(id) if *id == self.selection.text().id => Some(Role::Text),
            StorageId::Temporary(id) if *id == self.selection.producer().id => Some(Role::Payload),
            _ => None,
        });
        match role {
            Some(Role::Text) => drop(row.text.take()),
            Some(Role::Payload) => {
                if let Some(mut bytes) = row.payload.take() {
                    let Backing::Leased(lease) = &mut bytes.bytes else {
                        return Err(refuse());
                    };
                    self.registry.drop_lease(lease)?;
                }
            }
            Some(Role::Induction) => {}
            None if row.text.is_some() || row.payload.is_some() => return Err(refuse()),
            None => {}
        }
        Ok(())
    }
}
fn copy_scalar(value: &Value) -> Result<Value, Flow> {
    Ok(match value {
        Value::Int(v) => Value::Int(*v),
        Value::Usize(v) => Value::Usize(*v),
        Value::Uint8(v) => Value::Uint8(*v),
        Value::Bool(v) => Value::Bool(*v),
        Value::OptionU8(v) => Value::OptionU8(*v),
        _ => return Err(refuse()),
    })
}

// Source scope slots are not the same universe as lowered HIR temporaries.
// Fund the complete static binding inventory, including hidden traversal locals
// and pattern fields; iteration reuse then never grows this flat backing.
fn binding_count(root: &E) -> Result<usize, Flow> {
    type Cursor<'a> = crate::hir::lexical_exit::ChildCursor<'a>;
    reserve(Layout::new::<[Option<Cursor<'_>>; DEPTH + 1]>())?;
    let mut frames: [Option<Cursor<'_>>; DEPTH + 1] = std::array::from_fn(|_| None);
    frames[0] = Some(Cursor::new(root));
    let mut depth = 1usize;
    let mut remaining = crate::cache_codec::MAX_NODES;
    let mut count = 0usize;
    while depth != 0 {
        remaining = remaining.checked_sub(1).ok_or_else(refuse)?;
        let cursor = frames[depth - 1].as_mut().ok_or_else(refuse)?;
        if !cursor.is_started() {
            match &cursor.expression().kind {
                K::Block { statements, .. } => {
                    remaining = remaining.checked_sub(statements.len()).ok_or_else(refuse)?;
                    count = count
                        .checked_add(
                            statements
                                .iter()
                                .filter(|s| matches!(s, S::Let { .. }))
                                .count(),
                        )
                        .ok_or_else(refuse)?;
                }
                K::Match { arms, .. } => {
                    remaining = remaining.checked_sub(arms.len()).ok_or_else(refuse)?;
                    for arm in arms {
                        let Pattern::Variant { fields, .. } = &arm.pattern else {
                            return Err(refuse());
                        };
                        remaining = remaining.checked_sub(fields.len()).ok_or_else(refuse)?;
                        count = count.checked_add(fields.len()).ok_or_else(refuse)?;
                    }
                }
                _ => {}
            }
        }
        if let Some(child) = cursor.next() {
            *frames.get_mut(depth).ok_or_else(refuse)? = Some(Cursor::new(child));
            depth += 1;
        } else {
            frames[depth - 1] = None;
            depth -= 1;
        }
    }
    Ok(count)
}
