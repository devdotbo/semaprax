//! Selected executor input and the checks every producer of it must pass.
//!
//! [`Selected`] borrows typed facts; it is not authority. Its producers are
//! [`from_joined`] (inside `accept`, from the validated owner's fresh join)
//! and the test-only unvalidated precursor. Both must call [`check`] against
//! the exact current source and program before lowering.
use super::{layout, Refusal};
use crate::byte_data_capacity::MAX_BYTES_COPY_SITES;
use crate::construction_fusion::native_provenance::{Carrier, Mapping};
use crate::hir::fused_construction::{join::Joined, Role};
use crate::hir::{
    ResolvedBinding, ResolvedExpr as E, ResolvedFieldDeclaration, ResolvedFunction,
    ResolvedProgram, ResolvedStatement as S, ResolvedType as T, ResolvedTypeDeclaration,
    ResolvedTypeDeclarationKind,
};

/// This executor's own row bound. The capacity rule's private ceiling has the
/// same value; each executor must still check its own layouts independently.
const MAX_SELECTED_ROWS: u32 = 4_096;

pub(super) struct Selected<'a> {
    pub(super) source: &'a crate::ast::Program,
    pub(super) program: &'a ResolvedProgram,
    pub(super) function: &'a ResolvedFunction,
    pub(super) record: &'a ResolvedTypeDeclaration,
    pub(super) vector: &'a ResolvedBinding,
    pub(super) counter: &'a ResolvedBinding,
    pub(super) item: &'a ResolvedBinding,
    pub(super) condition: &'a E,
    pub(super) body: &'a E,
    pub(super) constructor: &'a E,
    pub(super) push: &'a E,
    pub(super) step: &'a E,
    /// The single original `bytes_zeroed` operand the backing replaces.
    pub(super) producer: &'a E,
    pub(super) byte_field: &'a ResolvedFieldDeclaration,
    pub(super) suffix: &'a [S],
    pub(super) tail: &'a E,
    pub(super) rows: u32,
    pub(super) width: u64,
    pub(super) order: [&'static str; 3],
    /// Independent provenance used only to refuse disagreement. The lowering
    /// takes every carrier from its producing field read, never from here.
    pub(super) mapping: Option<&'a Mapping<'a>>,
}

impl Selected<'_> {
    /// Refuse unless the independent provenance, when present, names exactly
    /// `carrier` for this view expression.
    pub(super) fn cross_check(&self, expression: &E, carrier: Carrier) -> Result<(), Refusal> {
        match self.mapping {
            None => Ok(()),
            Some(mapping) => match mapping.fact(expression) {
                Some(fact) if fact.carrier() == carrier => Ok(()),
                _ => Err(Refusal::Carrier),
            },
        }
    }
}

/// Build the executor input from one fresh, current join. Every node comes
/// from the joined selection over the exact owner program; nothing is cached.
pub(super) fn from_joined<'a>(
    joined: &Joined<'a>,
    source: &'a crate::ast::Program,
    program: &'a ResolvedProgram,
    mapping: Option<&'a Mapping<'a>>,
) -> Result<Selected<'a>, Refusal> {
    let selection = joined.selection();
    if !std::ptr::eq(selection.program(), program) || !joined.still_current(source, program) {
        return Err(Refusal::Stale);
    }
    let mut order = [""; 3];
    let mut byte_field = None;
    let fields = selection.fields();
    if fields.len() != 3 {
        return Err(Refusal::Join);
    }
    for ((slot, role), field) in order.iter_mut().zip(selection.order()).zip(fields) {
        *slot = match (role, &field.ty) {
            (Role::Induction, T::I64) => "scalar",
            (Role::Text, T::String) => "text",
            (Role::Payload, T::Bytes) => {
                byte_field = Some(field);
                "payload"
            }
            _ => return Err(Refusal::Join),
        };
    }
    Ok(Selected {
        source,
        program,
        function: selection.function(),
        record: selection.record(),
        vector: selection.vector(),
        counter: selection.counter(),
        item: selection.item(),
        condition: selection.condition(),
        body: selection.body(),
        constructor: selection.constructor(),
        push: selection.push(),
        step: selection.step(),
        producer: selection.producer(),
        byte_field: byte_field.ok_or(Refusal::Join)?,
        suffix: selection.suffix(),
        tail: selection.tail(),
        rows: selection.rows(),
        width: selection.width(),
        order,
        mapping,
    })
}

/// Field roles in declaration order. Exactly one i64, String and Bytes field,
/// and the Bytes field is the selected producer's field.
pub(super) fn roles(
    record: &ResolvedTypeDeclaration,
    byte_field: &ResolvedFieldDeclaration,
) -> Result<[&'static str; 3], Refusal> {
    let ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        return Err(Refusal::Join);
    };
    let [a, b, c] = fields.as_slice() else {
        return Err(Refusal::Join);
    };
    let mut order = [""; 3];
    for (slot, field) in order.iter_mut().zip([a, b, c]) {
        *slot = match field.ty {
            T::I64 => "scalar",
            T::String => "text",
            T::Bytes if field.id == byte_field.id => "payload",
            _ => return Err(Refusal::Join),
        };
    }
    for role in ["scalar", "text", "payload"] {
        if order.iter().filter(|actual| **actual == role).count() != 1 {
            return Err(Refusal::Join);
        }
    }
    Ok(order)
}

/// Executor-side checks shared by every producer of [`Selected`].
pub(super) fn check(
    selected: &Selected<'_>,
    source: &crate::ast::Program,
    program: &ResolvedProgram,
) -> Result<(), Refusal> {
    if !std::ptr::eq(selected.source, source)
        || !std::ptr::eq(selected.program, program)
        || selected
            .mapping
            .is_some_and(|mapping| !mapping.still_held(source, program))
    {
        return Err(Refusal::Stale);
    }
    let function = selected.function;
    let [only] = program.functions.as_slice() else {
        return Err(Refusal::Join);
    };
    if !std::ptr::eq(only, function)
        || !program.function_instances.is_empty()
        || !function.params.is_empty()
        || function.return_type != T::I64
        || !function.requires.is_empty()
        || !function.ensures.is_empty()
        || !function.effects.is_empty()
    {
        return Err(Refusal::Join);
    }
    if selected.rows <= MAX_BYTES_COPY_SITES {
        return Err(Refusal::Legacy);
    }
    let extent = u64::from(selected.rows)
        .checked_mul(selected.width)
        .ok_or(Refusal::Layout)?;
    if selected.rows > MAX_SELECTED_ROWS
        || selected.width == 0
        || extent > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES
        || !layout::fits_ordinary_vec(selected.rows)
    {
        return Err(Refusal::Layout);
    }
    if roles(selected.record, selected.byte_field)? != selected.order {
        return Err(Refusal::Join);
    }
    // Every leased view comes from the exact selected Bytes field of the
    // selected vector; every other view stays an ordinary String carrier.
    for fact in selected.mapping.map_or(&[][..], |mapping| mapping.facts()) {
        let leased = fact.carrier() == Carrier::LeasedSlice;
        let payload = fact.field().id == selected.byte_field.id;
        if leased != payload || fact.vector() != &selected.vector.id {
            return Err(Refusal::Carrier);
        }
    }
    authenticate_plans(program, function)
}

/// The attached cleanup and loan plans must be the exact canonical plans of
/// the current HIR. The ordinary validators rebuild and compare (and replay)
/// every function; the selected function is additionally compared directly,
/// and lexical-control plans stay outside this closed profile.
pub(super) fn authenticate_plans(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(), Refusal> {
    crate::cleanup_plan::validate_program(program).map_err(|_| Refusal::Cleanup)?;
    let cleanup =
        crate::cleanup_plan::build_plan(program, function).map_err(|_| Refusal::Cleanup)?;
    if cleanup != function.cleanup_plan || function.cleanup_plan.lexical_control.is_some() {
        return Err(Refusal::Cleanup);
    }
    crate::loan_plan::validate_program(program).map_err(|_| Refusal::Loans)?;
    let loans = crate::loan_plan::build_plan(program, function).map_err(|_| Refusal::Loans)?;
    if loans != function.loan_plan {
        return Err(Refusal::Loans);
    }
    Ok(())
}
