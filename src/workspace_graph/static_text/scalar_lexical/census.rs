//! Closed ordinary scalar lexical family census, shared by source capture and
//! paid reception. One metered borrowed walk visits every requires, body and
//! ensures node, including dead branches. It admits only Value scalar
//! signatures, scalar literals, unprojected scalar places, unary/binary
//! operators, blocks, `if`, condition-latched loops, scalar `let`/assignment
//! and typed lexical exits. `Never` is admitted only on an exit and on the
//! block/`if` control nodes that carry one; targets, latches, cleanup and
//! loan plans remain independently replayed by the paid Session. Every Call,
//! Invoke, function value, closure, String/Bytes/Str/Slice/Vec/nominal/array
//! carrier, native or host operation, effect, yield, match, try, record,
//! projection and unsafe node leaves the family, as does every cleanup or
//! loan selector that a higher Graph family reads.
use crate::hir::{
    self, OwnershipMode, ResolvedExpr, ResolvedExprKind as K, ResolvedFunction,
    ResolvedStatement as S, ResolvedType,
};

pub(in crate::workspace_graph) const DEPTH: usize = crate::cache_codec::MAX_DEPTH + 1;
type Frame<'a> = Option<hir::lexical_exit::ChildCursor<'a>>;

/// Fixed cursor backing of one census, priced before the walk begins.
pub(in crate::workspace_graph) fn cursor_bytes() -> usize {
    DEPTH * std::mem::size_of::<Frame<'static>>()
}

/// Scalar Graph flags derived during the same metered walk. They feed only
/// the shared base-lattice mapping; no other Graph classifier is consulted.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::workspace_graph) struct Flags {
    pub(in crate::workspace_graph) lexical: bool,
    pub(in crate::workspace_graph) usize: bool,
    pub(in crate::workspace_graph) body_loop: bool,
}
impl Flags {
    pub(in crate::workspace_graph) fn merge(&mut self, member: Self) {
        self.lexical |= member.lexical;
        self.usize |= member.usize;
        self.body_loop |= member.body_loop;
    }
}

/// `Ok(None)` is outside the family; only the caller's own charge refuses.
pub(in crate::workspace_graph) fn function<E>(
    function: &ResolvedFunction,
    charge: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<Option<Flags>, E> {
    let Some(units) = function.params.len().checked_add(1) else {
        return Ok(None);
    };
    charge(units)?;
    if !function.effects.is_empty()
        || function.yields.is_some()
        || !hir::is_scalar_resolved_type(&function.return_type)
        || !plans(function)
    {
        return Ok(None);
    }
    let mut flags = Flags {
        usize: function.return_type == ResolvedType::Usize,
        ..Flags::default()
    };
    for param in &function.params {
        if param.ownership != OwnershipMode::Value || !hir::is_scalar_resolved_type(&param.ty) {
            return Ok(None);
        }
        flags.usize |= param.ty == ResolvedType::Usize;
    }
    let mut stack: [Frame<'_>; DEPTH] = [None; DEPTH];
    let body = function.requires.len();
    for (index, root) in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .enumerate()
    {
        // Graph's base while family reads the body only; contracts still
        // belong to the closed census.
        let in_body = index == body;
        stack[0] = Some(hir::lexical_exit::ChildCursor::new(root));
        let mut depth = 1;
        while depth != 0 {
            charge(1)?;
            let Some(mut cursor) = stack[depth - 1].take() else {
                return Ok(None);
            };
            if !cursor.is_started() && !node(cursor.expression(), in_body, &mut flags, charge)? {
                return Ok(None);
            }
            if let Some(child) = cursor.next() {
                if depth == DEPTH {
                    return Ok(None);
                }
                stack[depth - 1] = Some(cursor);
                stack[depth] = Some(hir::lexical_exit::ChildCursor::new(child));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
    }
    Ok(Some(flags))
}

/// Selectors read by the native, owned-variant, loan and nested Graph
/// families. Their presence leaves this family; it never grants a schema.
fn plans(function: &ResolvedFunction) -> bool {
    use crate::cleanup_plan::{
        CLEANUP_PLAN_SCHEMA_V14, CLEANUP_PLAN_SCHEMA_V15, CLEANUP_PLAN_SCHEMA_V16,
        CLEANUP_PLAN_SCHEMA_V17, CLEANUP_PLAN_SCHEMA_V6, CLEANUP_PLAN_SCHEMA_V7,
        CLEANUP_PLAN_SCHEMA_V8, CLEANUP_PLAN_SCHEMA_V9,
    };
    function.cleanup.schema != crate::cleanup::CLEANUP_INVENTORY_SCHEMA_V2
        && !matches!(
            function.cleanup_plan.schema,
            CLEANUP_PLAN_SCHEMA_V6
                | CLEANUP_PLAN_SCHEMA_V7
                | CLEANUP_PLAN_SCHEMA_V8
                | CLEANUP_PLAN_SCHEMA_V9
                | CLEANUP_PLAN_SCHEMA_V14
                | CLEANUP_PLAN_SCHEMA_V15
                | CLEANUP_PLAN_SCHEMA_V16
                | CLEANUP_PLAN_SCHEMA_V17
        )
        && function.loan_plan.loans.is_empty()
}

fn node<E>(
    expression: &ResolvedExpr,
    in_body: bool,
    flags: &mut Flags,
    charge: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    let never = expression.ty == ResolvedType::Never;
    if expression.ownership != OwnershipMode::Value
        || !(never || hir::is_scalar_resolved_type(&expression.ty))
    {
        return Ok(false);
    }
    flags.usize |= expression.ty == ResolvedType::Usize;
    Ok(match &expression.kind {
        K::Int(_)
        | K::Int32(_)
        | K::Char(_)
        | K::Uint8(_)
        | K::Usize(_)
        | K::Float32(_)
        | K::Float64(_)
        | K::Bool(_)
        | K::Unary { .. }
        | K::Binary { .. } => !never,
        K::Place(place) => !never && place.projections.is_empty(),
        K::If { .. } => true,
        K::LexicalExit { .. } => {
            flags.lexical = true;
            never
        }
        K::Block { statements, .. } => {
            charge(statements.len())?;
            statements
                .iter()
                .all(|statement| self::statement(statement, in_body, flags))
        }
        _ => false,
    })
}

fn statement(statement: &S, in_body: bool, flags: &mut Flags) -> bool {
    match statement {
        S::Let { binding, .. }
        | S::Assign {
            binding,
            field: None,
            ..
        } => {
            binding.ownership == OwnershipMode::Value && hir::is_scalar_resolved_type(&binding.ty)
        }
        S::While { .. }
        | S::LexicalWhile {
            latch: hir::lexical_exit::LoopLatchKind::Condition,
            ..
        } => {
            flags.body_loop |= in_body;
            true
        }
        _ => false,
    }
}
