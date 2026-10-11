//! Selected module assembly and the exact construction prefix.
//!
//! The prefix is lowered only where its statements are the very HIR nodes the
//! checked [`Selected`] input borrows. Row operands stage in source order: the
//! scalar, then the ordinary String materialization, then the Bytes operand,
//! where the host carves metadata from the held invocation account, charges
//! the single fused event and only then allocates the backing. Every row
//! publishes before the next issue. Owner liveness, failure cleanup, scope
//! ends and the successful commit all come from the canonical cleanup plan
//! (`cleanup`); this module never orders a finalizer itself.
use super::cleanup::{Failure, Owned, Replay};
use super::facts::Selected;
use super::{import, Refusal, ENTRY, IMPORTS, NAMESPACE};
use crate::hir::{
    ExpressionId, ResolvedBinding, ResolvedExpr as E, ResolvedExprKind as K,
    ResolvedStatement as S, ResolvedType as T, ValueId,
};
use crate::loan_plan::{Loan, LoanCause, LoanPointPhase};
use wasm_encoder::{
    BlockType, CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection,
    Function, FunctionSection, ImportSection, Instruction as I, MemorySection, MemoryType, Module,
    TypeSection, ValType,
};

/// Module-private String literal bytes start here; the host bounds-checks
/// every literal range against the exported memory before copying it.
const LITERAL_BASE: u32 = 1_024;
const LITERAL_LIMIT: u32 = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Repr {
    I64,
    Usize,
    Bool,
    U8,
    /// Selected row Vec handle, held in its cleanup-plan carrier.
    Vector,
    /// Borrowed ordinary String carrier (the row's String owner handle).
    Text,
    /// Ordinary `str_as_bytes` view of a String carrier.
    Slice,
    /// Leased payload view handle from the 64-entry view table.
    Leased,
    /// `byte_get` result: the byte, or -1 for `None`.
    OptionU8,
}
impl Repr {
    pub(super) fn val(self) -> ValType {
        match self {
            Self::Bool | Self::U8 | Self::OptionU8 => ValType::I32,
            _ => ValType::I64,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Phase {
    Building,
    Complete,
}

pub(super) struct Binding<'a> {
    pub(super) id: &'a ValueId,
    pub(super) local: u32,
    pub(super) repr: Repr,
}

/// One open leased view: its local, the exact `SliceView` loan the canonical
/// loan plan attaches to its producing site, and which of that loan's end
/// points have been lowered. Other loans at the same site (the transient
/// shared loan of the row Vec operand) never end the view.
pub(super) struct View<'a> {
    pub(super) local: u32,
    loan: &'a Loan,
    lowered: Vec<bool>,
}

/// One open lexical scope: its leased views.
pub(super) struct Scope<'a> {
    pub(super) views: Vec<View<'a>>,
}

pub(super) struct Lower<'a> {
    pub(super) selected: &'a Selected<'a>,
    pub(super) code: Vec<I<'static>>,
    pub(super) locals: Vec<ValType>,
    pub(super) bindings: Vec<Binding<'a>>,
    pub(super) scopes: Vec<Scope<'a>>,
    pub(super) cleanup: Replay<'a>,
    pub(super) domain: u32,
    pub(super) phase: Phase,
    literals: Vec<u8>,
}

pub(super) fn unsupported(what: &'static str) -> Refusal {
    Refusal::Unsupported(what)
}

pub(super) fn module(selected: &Selected<'_>) -> Result<Vec<u8>, Refusal> {
    let mut lower = Lower {
        selected,
        code: Vec::new(),
        locals: Vec::new(),
        bindings: Vec::new(),
        scopes: Vec::new(),
        cleanup: Replay::empty(&selected.function.cleanup_plan),
        domain: 0,
        phase: Phase::Building,
        literals: Vec::new(),
    };
    lower.domain = lower.local(ValType::I64);
    lower.replay_init()?;
    lower.body()?;
    lower.census()?;
    let mut types = TypeSection::new();
    let mut imports = ImportSection::new();
    for (index, (name, params, results)) in IMPORTS.iter().enumerate() {
        types
            .ty()
            .function(params.iter().copied(), results.iter().copied());
        let index = u32::try_from(index).map_err(|_| unsupported("import index"))?;
        imports.import(NAMESPACE, name, EntityType::Function(index));
    }
    let count = u32::try_from(IMPORTS.len()).map_err(|_| unsupported("import index"))?;
    types.ty().function([], [ValType::I64]);
    let mut functions = FunctionSection::new();
    functions.function(count);
    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: 1,
        maximum: Some(1),
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    exports.export(ENTRY, ExportKind::Func, count);
    let mut function = Function::new_with_locals_types(lower.locals.iter().copied());
    for instruction in &lower.code {
        function.instruction(instruction);
    }
    function.instruction(&I::End);
    let mut code = CodeSection::new();
    code.function(&function);
    let mut data = DataSection::new();
    let base = i32::try_from(LITERAL_BASE).map_err(|_| unsupported("literal base"))?;
    data.active(
        0,
        &ConstExpr::i32_const(base),
        lower.literals.iter().copied(),
    );
    let mut module = Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&memories)
        .section(&exports)
        .section(&code)
        .section(&data);
    Ok(module.finish())
}

impl<'a> Lower<'a> {
    pub(super) fn op(&mut self, instruction: I<'static>) {
        self.code.push(instruction);
    }
    pub(super) fn local(&mut self, ty: ValType) -> u32 {
        self.locals.push(ty);
        u32::try_from(self.locals.len() - 1).expect("bounded selected local count")
    }
    /// Call a host import whose i32 result is zero on success. The host throws
    /// on refusal, so a nonzero value is a host invariant failure.
    pub(super) fn checked(&mut self, function: u32) {
        self.op(I::Call(function));
        self.op(I::If(BlockType::Empty));
        self.op(I::Unreachable);
        self.op(I::End);
    }
    /// Store a fresh nonzero handle; zero is a host invariant failure.
    pub(super) fn owned(&mut self, local: u32) {
        self.op(I::LocalTee(local));
        self.op(I::I64Eqz);
        self.op(I::If(BlockType::Empty));
        self.op(I::Unreachable);
        self.op(I::End);
    }
    pub(super) fn lookup(&self, id: &ValueId) -> Result<(u32, Repr), Refusal> {
        self.bindings
            .iter()
            .rev()
            .find(|binding| binding.id == id)
            .map(|binding| (binding.local, binding.repr))
            .ok_or(unsupported("unbound place"))
    }
    fn literal(&mut self, text: &str) -> Result<(i32, i32), Refusal> {
        let offset = u32::try_from(self.literals.len()).map_err(|_| unsupported("literal"))?;
        let length = u32::try_from(text.len()).map_err(|_| unsupported("literal"))?;
        let start = LITERAL_BASE + offset;
        if start
            .checked_add(length)
            .is_none_or(|end| end > LITERAL_LIMIT)
        {
            return Err(unsupported("literal table"));
        }
        self.literals.extend_from_slice(text.as_bytes());
        let start = i32::try_from(start).map_err(|_| unsupported("literal"))?;
        let length = i32::try_from(length).map_err(|_| unsupported("literal"))?;
        Ok((start, length))
    }

    /// Open a leased view produced at `site`. The canonical loan plan must
    /// attach exactly one `SliceView` loan there. Its ends are rederived from
    /// the authoritative `end_edges`: on each such edge, and on every edge
    /// entering its destination, the loan must be dead, so a guarded end at
    /// that point is exact on every executed path. Any disagreement refuses.
    pub(super) fn open_view(&mut self, local: u32, site: &ExpressionId) -> Result<(), Refusal> {
        let selected = self.selected;
        let plan = &selected.function.loan_plan;
        let mut loans = plan
            .loans
            .iter()
            .filter(|loan| &loan.site == site && loan.cause == LoanCause::SliceView);
        let loan = loans.next().ok_or(Refusal::Loans)?;
        if loans.next().is_some() || loan.end_edges.is_empty() {
            return Err(Refusal::Loans);
        }
        let mut ends = Vec::with_capacity(loan.end_edges.len());
        for edge in &loan.end_edges {
            let edge = plan.edges.get(usize::from(*edge)).ok_or(Refusal::Loans)?;
            let to = plan
                .endpoints
                .get(usize::from(edge.to))
                .ok_or(Refusal::Loans)?;
            if edge.live.contains(&loan.id)
                || to.live_before.contains(&loan.id)
                || !to.kills.contains(&loan.id)
                || plan
                    .edges
                    .iter()
                    .any(|entering| entering.to == edge.to && entering.live.contains(&loan.id))
            {
                return Err(Refusal::Loans);
            }
            if ends.last() != Some(&to.point) {
                ends.push(to.point.clone());
            }
        }
        if ends != loan.ends {
            return Err(Refusal::Loans);
        }
        self.scopes
            .last_mut()
            .ok_or(unsupported("view scope"))?
            .views
            .push(View {
                local,
                loan,
                lowered: vec![false; loan.ends.len()],
            });
        Ok(())
    }
    /// End, in loan-plan vector order, every open leased view whose own loan
    /// the canonical loan plan ends at this program point. A loan end is the
    /// destination of an edge from a live to a dead point, so the loan is dead
    /// on every path reaching it; the guarded end therefore runs once per
    /// path, whichever of the loan's ends (a short-circuit join, either match
    /// arm) that path reaches. Views end only here (or on a failure path, for
    /// exactly the loans live at that point).
    pub(super) fn loan_points(&mut self, at: &ExpressionId, phase: LoanPointPhase) {
        let selected = self.selected;
        let mut ending = Vec::new();
        for loan in &selected.function.loan_plan.loans {
            for scope in &mut self.scopes {
                for view in &mut scope.views {
                    if !std::ptr::eq(view.loan, loan) {
                        continue;
                    }
                    for (index, end) in loan.ends.iter().enumerate() {
                        if &end.expression == at && end.phase == phase && !view.lowered[index] {
                            view.lowered[index] = true;
                            if !ending.contains(&view.local) {
                                ending.push(view.local);
                            }
                        }
                    }
                }
            }
        }
        for view in ending {
            self.end_view(view);
        }
    }
    /// Leased views a failure at `at` must end: exactly the open views whose
    /// loans the plan holds live before `at`, in its live-set order. An open
    /// view whose loan is dead at `at` was ended at a lowered loan end on
    /// every path here; one with no lowered end yet is a lowering/plan
    /// disagreement.
    pub(super) fn live_views_at(&self, at: &ExpressionId) -> Result<Vec<u32>, Refusal> {
        let open: Vec<(u32, &Loan, bool)> = self
            .scopes
            .iter()
            .flat_map(|scope| {
                scope
                    .views
                    .iter()
                    .map(|view| (view.local, view.loan, view.lowered.contains(&true)))
            })
            .collect();
        if open.is_empty() {
            return Ok(Vec::new());
        }
        let plan = &self.selected.function.loan_plan;
        let point = plan
            .endpoints
            .iter()
            .position(|endpoint| {
                &endpoint.point.expression == at && endpoint.point.phase == LoanPointPhase::Before
            })
            .ok_or(Refusal::Loans)?;
        let endpoint = &plan.endpoints[point];
        let mut covered = vec![false; open.len()];
        let mut views = Vec::new();
        for id in endpoint.live_before.iter() {
            let loan = plan
                .loans
                .iter()
                .find(|loan| loan.id == *id)
                .ok_or(Refusal::Loans)?;
            for (index, (local, view_loan, _)) in open.iter().enumerate() {
                if !covered[index] && std::ptr::eq(*view_loan, loan) {
                    // An open view's loan must be live on every edge entering
                    // `at`, not merely in the union summary.
                    if plan
                        .edges
                        .iter()
                        .any(|edge| usize::from(edge.to) == point && !edge.live.contains(id))
                    {
                        return Err(Refusal::Loans);
                    }
                    covered[index] = true;
                    views.push(*local);
                }
            }
        }
        if open
            .iter()
            .zip(&covered)
            .any(|((_, _, ended), covered)| !covered && !ended)
        {
            return Err(Refusal::Loans);
        }
        Ok(views)
    }
    pub(super) fn end_view(&mut self, view: u32) {
        self.op(I::LocalGet(view));
        self.op(I::I64Eqz);
        self.op(I::I32Eqz);
        self.op(I::If(BlockType::Empty));
        self.op(I::LocalGet(self.domain));
        self.op(I::LocalGet(view));
        self.checked(import::END_BORROW);
        self.op(I::I64Const(0));
        self.op(I::LocalSet(view));
        self.op(I::End);
    }
    /// Close the innermost scope at the end of `block`. The block's own
    /// after-point loan ends are processed first; a view with any loan end
    /// not yet lowered would outlive its scope on that path, so lowering
    /// refuses.
    pub(super) fn close_scope(&mut self, block: &E) -> Result<(), Refusal> {
        self.loan_points(&block.id, LoanPointPhase::After);
        let scope = self.scopes.pop().ok_or(unsupported("scope"))?;
        if scope.views.iter().any(|view| view.lowered.contains(&false)) {
            return Err(Refusal::Loans);
        }
        Ok(())
    }

    fn body(&mut self) -> Result<(), Refusal> {
        let selected = self.selected;
        let function = selected.function;
        let K::Block { statements, tail } = &function.body.kind else {
            return Err(unsupported("function body"));
        };
        let [S::Let {
            binding: vector,
            value: allocation,
            ..
        }, S::Let {
            binding: counter,
            value: start,
            ..
        }, S::While {
            condition, body, ..
        }, suffix @ ..] = statements.as_slice()
        else {
            return Err(unsupported("construction prefix"));
        };
        if vector.id != selected.vector.id
            || counter.id != selected.counter.id
            || !std::ptr::eq(condition.as_ref(), selected.condition)
            || !std::ptr::eq(body.as_ref(), selected.body)
            || suffix.len() != selected.suffix.len()
            || !std::ptr::eq(suffix.as_ptr(), selected.suffix.as_ptr())
            || !std::ptr::eq(tail.as_ref(), selected.tail)
        {
            return Err(unsupported("construction prefix identity"));
        }
        self.allocation(vector, allocation)?;
        if self.expr(start)? != Repr::I64 {
            return Err(unsupported("induction start"));
        }
        let induction = self.local(ValType::I64);
        self.op(I::LocalSet(induction));
        self.bindings.push(Binding {
            id: &counter.id,
            local: induction,
            repr: Repr::I64,
        });
        self.construction(condition, body, induction)?;
        self.op(I::LocalGet(self.domain));
        self.checked(import::COMPLETE);
        self.phase = Phase::Complete;
        self.scopes.push(Scope { views: Vec::new() });
        for statement in suffix {
            self.statement(statement, true)?;
        }
        if self.expr(tail)? != Repr::I64 {
            return Err(unsupported("function result"));
        }
        let result = self.local(ValType::I64);
        self.op(I::LocalSet(result));
        self.close_scope(&function.body)?;
        self.post(&function.body.id, Owned::None)?;
        // No postconditions exist in this profile (checked); the commit exit's
        // non-result cleanup runs, and only then is the result published. The
        // canonical scalar result source names the function body block, whose
        // value is the already staged tail.
        self.commit(&function.body.id)?;
        self.op(I::LocalGet(result));
        Ok(())
    }

    /// `vec_with_capacity<Row>(rows)`: take the administrative construction
    /// pin, then allocate the ordinary row Vec at its own site. Neither carves
    /// lease metadata nor allocates payload.
    fn allocation(&mut self, vector: &'a ResolvedBinding, value: &'a E) -> Result<(), Refusal> {
        let selected = self.selected;
        let K::Call {
            callee,
            type_arguments,
            instance: None,
            args,
        } = &value.kind
        else {
            return Err(unsupported("row Vec allocation"));
        };
        let [capacity] = args.as_slice() else {
            return Err(unsupported("row Vec allocation"));
        };
        let rows = u64::from(selected.rows);
        if callee.as_str() != crate::vec_ops::WITH_CAPACITY_ID
            || type_arguments.as_slice() != std::slice::from_ref(&selected.item.ty)
            || !matches!(capacity.kind, K::Usize(value) if value == rows)
        {
            return Err(unsupported("row Vec allocation"));
        }
        let rows = i64::from(selected.rows);
        let width = i64::try_from(selected.width).map_err(|_| unsupported("width"))?;
        self.op(I::I64Const(rows));
        self.op(I::I64Const(width));
        self.op(I::Call(import::START));
        self.op(I::LocalSet(self.domain));
        // The capacity literal is an operand of the allocation call.
        if self.expr(capacity)? != Repr::Usize {
            return Err(unsupported("row Vec capacity"));
        }
        let requested = self.local(ValType::I64);
        self.op(I::LocalSet(requested));
        let handle = self.local(ValType::I64);
        self.op(I::LocalGet(self.domain));
        self.op(I::LocalGet(requested));
        self.op(I::Call(import::VEC_NEW));
        self.op(I::LocalTee(handle));
        self.op(I::I64Eqz);
        self.op(I::If(BlockType::Empty));
        self.fail(
            &value.id,
            Failure::VecAllocation,
            crate::wasm::aggregate::STATUS_VEC_ALLOCATION_FAILURE,
        )?;
        self.op(I::End);
        self.post(&value.id, Owned::Leaf(handle))?;
        self.bindings.push(Binding {
            id: &vector.id,
            local: self.vector_carrier(&vector.id)?,
            repr: Repr::Vector,
        });
        Ok(())
    }

    fn construction(
        &mut self,
        condition: &'a E,
        body: &'a E,
        induction: u32,
    ) -> Result<(), Refusal> {
        let selected = self.selected;
        let K::Block { statements, tail } = &body.kind else {
            return Err(unsupported("row body"));
        };
        let [S::Let {
            binding: item,
            value: constructor,
            ..
        }, S::Assign {
            binding: destination,
            field: None,
            value: push,
            ..
        }, S::Assign {
            binding: latch,
            field: None,
            value: step,
            ..
        }] = statements.as_slice()
        else {
            return Err(unsupported("row body"));
        };
        if item.id != selected.item.id
            || destination.id != selected.vector.id
            || latch.id != selected.counter.id
            || !std::ptr::eq(constructor, selected.constructor)
            || !std::ptr::eq(push, selected.push)
            || !std::ptr::eq(step, selected.step)
            || !matches!(tail.kind, K::Int(0))
        {
            return Err(unsupported("row body identity"));
        }
        self.op(I::Block(BlockType::Empty));
        self.op(I::Loop(BlockType::Empty));
        if self.expr(condition)? != Repr::Bool {
            return Err(unsupported("row condition"));
        }
        self.op(I::I32Eqz);
        self.op(I::BrIf(1));
        self.scopes.push(Scope { views: Vec::new() });
        let mark = self.bindings.len();
        let row = self.row(constructor, induction)?;
        self.publish(push, item, row)?;
        if self.expr(step)? != Repr::I64 {
            return Err(unsupported("row latch"));
        }
        self.op(I::LocalSet(induction));
        if self.expr(tail)? != Repr::I64 {
            return Err(unsupported("row body value"));
        }
        self.op(I::Drop);
        self.close_scope(body)?;
        self.block_end(body)?;
        self.post(&body.id, Owned::None)?;
        self.bindings.truncate(mark);
        self.op(I::Br(0));
        self.op(I::End);
        self.op(I::End);
        Ok(())
    }

    /// Stage one fresh row's operands in authored order. Each owner handle
    /// enters the plan at its own operand; the record then initializes the row.
    fn row(&mut self, constructor: &'a E, induction: u32) -> Result<[u32; 3], Refusal> {
        let selected = self.selected;
        let crate::hir::ResolvedTypeDeclarationKind::Record { fields } = &selected.record.kind
        else {
            return Err(unsupported("row record"));
        };
        let K::ConstructRecord {
            fields: initializers,
            ..
        } = &constructor.kind
        else {
            return Err(unsupported("row constructor"));
        };
        if initializers.len() != fields.len() {
            return Err(unsupported("row constructor"));
        }
        let mut roles = [None; 3];
        let mut owned = Vec::new();
        for initializer in initializers {
            let field = fields
                .iter()
                .find(|field| field.id == initializer.field)
                .ok_or(unsupported("row field"))?;
            let value = &initializer.value;
            match field.ty {
                T::I64 => {
                    if self.expr(value)? != Repr::I64 {
                        return Err(unsupported("row scalar"));
                    }
                    let local = self.local(ValType::I64);
                    self.op(I::LocalSet(local));
                    set(&mut roles[0], local)?;
                }
                T::String => {
                    let K::String(text) = &value.kind else {
                        return Err(unsupported("row String operand"));
                    };
                    let (offset, length) = self.literal(text)?;
                    let local = self.local(ValType::I64);
                    self.op(I::LocalGet(self.domain));
                    self.op(I::I32Const(offset));
                    self.op(I::I32Const(length));
                    self.op(I::Call(import::STRING_LITERAL));
                    self.owned(local);
                    self.post(&value.id, Owned::Leaf(local))?;
                    owned.push((field.id.clone(), local));
                    set(&mut roles[1], local)?;
                }
                T::Bytes if std::ptr::eq(value, selected.producer) => {
                    // The original Bytes operand: metadata carving, the single
                    // fused event and the backing all happen here.
                    let local = self.local(ValType::I64);
                    self.op(I::LocalGet(self.domain));
                    self.op(I::LocalGet(induction));
                    self.op(I::Call(import::ISSUE));
                    self.owned(local);
                    self.post(&value.id, Owned::Leaf(local))?;
                    owned.push((field.id.clone(), local));
                    set(&mut roles[2], local)?;
                }
                _ => return Err(unsupported("row field type")),
            }
        }
        self.post(&constructor.id, Owned::Record(owned))?;
        match roles {
            [Some(scalar), Some(text), Some(owner)] => Ok([scalar, text, owner]),
            _ => Err(unsupported("row roles")),
        }
    }

    /// `items = vec_push<Row>(items, item)`: atomic row publication. A refused
    /// push consumed nothing; the plan's exact failure exit then runs.
    fn publish(
        &mut self,
        push: &'a E,
        item: &'a ResolvedBinding,
        [scalar, text, owner]: [u32; 3],
    ) -> Result<(), Refusal> {
        let selected = self.selected;
        let K::Call {
            callee,
            instance: None,
            args,
            ..
        } = &push.kind
        else {
            return Err(unsupported("row publication"));
        };
        let [old, fresh] = args.as_slice() else {
            return Err(unsupported("row publication"));
        };
        let (K::Place(old_place), K::Place(fresh_place)) = (&old.kind, &fresh.kind) else {
            return Err(unsupported("row publication"));
        };
        if callee.as_str() != crate::vec_ops::PUSH_ID
            || old_place.root != selected.vector.id
            || fresh_place.root != item.id
            || !old_place.projections.is_empty()
            || !fresh_place.projections.is_empty()
        {
            return Err(unsupported("row publication"));
        }
        let vector = self.vector_carrier(&selected.vector.id)?;
        // Argument staging transfers happen left to right before the call.
        self.post(&old.id, Owned::None)?;
        self.post(&fresh.id, Owned::None)?;
        let renewed = self.local(ValType::I64);
        for local in [self.domain, vector, scalar, text, owner] {
            self.op(I::LocalGet(local));
        }
        self.op(I::Call(import::ROW_PUSH));
        self.op(I::LocalTee(renewed));
        self.op(I::I64Eqz);
        self.op(I::If(BlockType::Empty));
        self.fail(
            &push.id,
            Failure::VecPush,
            crate::wasm::aggregate::STATUS_VEC_PUSH_FULL,
        )?;
        self.op(I::End);
        self.post(&push.id, Owned::Leaf(renewed))?;
        Ok(())
    }
}

fn set(slot: &mut Option<u32>, local: u32) -> Result<(), Refusal> {
    if slot.replace(local).is_some() {
        return Err(unsupported("duplicate row role"));
    }
    Ok(())
}
