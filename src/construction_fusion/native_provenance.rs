//! Private producer-bound carrier facts; never type-only Slice selection.
use super::*;
use hir::{
    OwnershipMode as O, ResolvedExpr as E, ResolvedExprKind as K, ResolvedStatement as S,
    ResolvedType as T,
};
use std::mem::size_of;
mod layout;
mod source;
mod traversal;
use traversal::Traversal;
#[cfg(test)]
mod tests;
pub(crate) use layout::TargetLayout;
const DEPTH: usize = crate::cache_codec::MAX_DEPTH + 1;
const LOCALS: usize = 64;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Carrier {
    OrdinaryStr,
    OrdinarySlice,
    LeasedSlice,
}
#[derive(Clone, Copy)]
struct Origin<'a> {
    carrier: Carrier,
    producer: &'a E,
    field: &'a hir::ResolvedFieldDeclaration,
    vector: &'a hir::ValueId,
    traversal: Traversal<'a>,
}
#[derive(Clone, Copy)]
enum Value<'a> {
    Vector(&'a hir::ValueId),
    View(Origin<'a>),
}
#[derive(Clone, Copy)]
struct Alias<'a> {
    binding: &'a hir::ResolvedBinding,
    value: Value<'a>,
}
#[derive(Clone, Copy)]
struct Frame<'a> {
    expression: &'a E,
    cursor: hir::lexical_exit::ChildCursor<'a>,
    base: usize,
    statement: usize,
    child: usize,
    tail: bool,
    binding: Option<&'a hir::ResolvedBinding>,
    traversal: Option<Traversal<'a>>,
}
struct Scratch<'a> {
    frames: [Option<Frame<'a>>; DEPTH],
    aliases: [Option<Alias<'a>>; LOCALS],
    len: usize,
}
#[derive(Clone, Copy)]
pub(crate) struct Fact<'a> {
    expression: &'a E,
    origin: Origin<'a>,
}
impl Fact<'_> {
    pub(crate) fn carrier(&self) -> Carrier {
        self.origin.carrier
    }
    pub(crate) fn producer(&self) -> &E {
        self.origin.producer
    }
    pub(crate) fn field(&self) -> &hir::ResolvedFieldDeclaration {
        self.origin.field
    }
    pub(crate) fn vector(&self) -> &hir::ValueId {
        self.origin.vector
    }
}
pub(crate) struct Mapping<'a> {
    descriptor: Descriptor<'a>,
    facts: Vec<Fact<'a>>,
    layout: TargetLayout,
}
impl Mapping<'_> {
    pub(crate) fn fact(&self, expression: &E) -> Option<&Fact<'_>> {
        self.facts.iter().find(|fact| {
            std::ptr::eq(fact.expression, expression) && fact.expression.id == expression.id
        })
    }
    pub(crate) fn facts(&self) -> &[Fact<'_>] {
        &self.facts
    }
    pub(crate) fn layout(&self) -> &TargetLayout {
        &self.layout
    }
    pub(crate) fn still_held(&self, source: &ast::Program, program: &hir::ResolvedProgram) -> bool {
        self.descriptor.still_held(source, program)
    }
}
fn debit(work: &mut usize, n: usize) -> Option<()> {
    *work = work.checked_sub(n)?;
    Some(())
}
fn frame<'a>(expression: &'a E, base: usize, traversal: Option<Traversal<'a>>) -> Frame<'a> {
    Frame {
        expression,
        cursor: hir::lexical_exit::ChildCursor::new(expression),
        base,
        statement: 0,
        child: 0,
        tail: false,
        binding: None,
        traversal,
    }
}
fn next<'a>(frame: &mut Frame<'a>) -> Option<&'a E> {
    frame.binding = None;
    if let K::Block { statements, tail } = &frame.expression.kind {
        while let Some(statement) = statements.get(frame.statement) {
            if let Some(child) = statement.child(frame.child) {
                frame.child += 1;
                if let S::Let {
                    binding,
                    mutable: false,
                    ..
                } = statement
                {
                    frame.binding = Some(binding);
                }
                return Some(child);
            }
            frame.statement += 1;
            frame.child = 0;
        }
        if !frame.tail {
            frame.tail = true;
            return Some(tail);
        }
        None
    } else {
        frame.cursor.next()
    }
}
fn alias<'a>(scratch: &Scratch<'a>, id: &hir::ValueId, work: &mut usize) -> Option<Alias<'a>> {
    for entry in &scratch.aliases[..scratch.len] {
        debit(work, 1)?;
        if let Some(entry) = entry {
            if &entry.binding.id == id {
                return Some(*entry);
            }
        }
    }
    None
}
fn vector_type(ty: &T, descriptor: &Descriptor<'_>) -> bool {
    ty == &descriptor.typed.vector.ty
}
fn classify<'a>(
    descriptor: &Descriptor<'a>,
    sources: &[source::Node<'a>],
    scratch: &Scratch<'a>,
    e: &'a E,
    traversal: Option<Traversal<'a>>,
    work: &mut usize,
) -> Option<Option<Value<'a>>> {
    if let K::Place(place) = &e.kind {
        if matches!(e.ty, T::Str | T::SliceU8) || vector_type(&e.ty, descriptor) {
            if !place.projections.is_empty() {
                return None;
            }
            let entry = alias(scratch, &place.root, work)?;
            if entry.binding.ty != e.ty
                || !(e.ownership == entry.binding.ownership
                    || vector_type(&e.ty, descriptor) && e.ownership == O::Borrow)
            {
                return None;
            }
            if matches!(entry.value, Value::View(_)) {
                source::variable(sources, e.span, &entry.binding.name, work)?;
            }
            return Some(Some(entry.value));
        }
    }
    if let K::VecFieldRead {
        origin: _,
        element,
        field,
        bytes,
        args,
    } = &e.kind
    {
        let [root, index] = args.as_slice() else {
            return None;
        };
        if element != &descriptor.typed.item.ty
            || index.ty != T::Usize
            || index.ownership != O::Value
        {
            return None;
        }
        let K::Place(place) = &root.kind else {
            return None;
        };
        if !place.projections.is_empty()
            || root.ty != descriptor.typed.vector.ty
            || !matches!(root.ownership, O::Own | O::Borrow)
        {
            return None;
        }
        let Value::Vector(vector) = alias(scratch, &place.root, work)?.value else {
            return None;
        };
        let traversal = traversal?;
        if place != traversal.owner
            || index.ty != traversal.index.ty
            || index.ownership != traversal.index.ownership
            || !matches!(&index.kind,K::Place(p) if p.root==traversal.index.id && p.projections.is_empty())
        {
            return None;
        }
        let selected = hir::vec_field::field(&descriptor.program.declarations, element, field)?;
        let carrier = match (&selected.declaration.ty, *bytes, &e.ty, e.ownership) {
            (T::String, false, T::Str, O::Borrow) => Carrier::OrdinaryStr,
            (T::String, true, T::SliceU8, O::Borrow) => Carrier::OrdinarySlice,
            (T::Bytes, false, T::SliceU8, O::Borrow) => Carrier::LeasedSlice,
            (T::I64, false, T::I64, O::Value) => {
                source::field(
                    sources,
                    e.span,
                    &selected.declaration.name,
                    *bytes,
                    traversal,
                    work,
                )?;
                return Some(None);
            }
            _ => return None,
        };
        source::field(
            sources,
            e.span,
            &selected.declaration.name,
            *bytes,
            traversal,
            work,
        )?;
        return Some(Some(Value::View(Origin {
            carrier,
            producer: e,
            field: selected.declaration,
            vector,
            traversal,
        })));
    }
    if let K::BorrowPlace { operation, place } = &e.kind {
        if operation.as_str() != crate::byte_ops::STR_AS_BYTES_ID
            || !place.projections.is_empty()
            || e.ty != T::SliceU8
            || e.ownership != O::Borrow
        {
            return None;
        }
        let entry = alias(scratch, &place.root, work)?;
        let Value::View(mut origin) = entry.value else {
            return None;
        };
        if entry.binding.ty != T::Str || origin.carrier != Carrier::OrdinaryStr {
            return None;
        }
        source::str_bytes(sources, e.span, &entry.binding.name, work)?;
        origin.carrier = Carrier::OrdinarySlice;
        return Some(Some(Value::View(origin)));
    }
    if let K::Call {
        callee,
        instance: None,
        type_arguments,
        args,
    } = &e.kind
    {
        if callee.as_str() == crate::vec_ops::SORT_OWNED_ID {
            let [argument] = args.as_slice() else {
                return None;
            };
            if type_arguments.as_slice() != std::slice::from_ref(&descriptor.typed.item.ty)
                || !vector_type(&e.ty, descriptor)
                || e.ownership != O::Own
            {
                return None;
            }
            let K::Place(place) = &argument.kind else {
                return None;
            };
            if argument.ownership != O::Own || argument.ty != e.ty || !place.projections.is_empty()
            {
                return None;
            }
            let Value::Vector(root) = alias(scratch, &place.root, work)?.value else {
                return None;
            };
            return Some(Some(Value::Vector(root)));
        }
    }
    if matches!(e.ty, T::Str | T::SliceU8) {
        None
    } else {
        Some(None)
    }
}
fn register<'a>(
    scratch: &mut Scratch<'a>,
    binding: &'a hir::ResolvedBinding,
    e: &'a E,
    value: Option<Value<'a>>,
    work: &mut usize,
) -> Option<()> {
    let Some(value) = value else {
        return Some(());
    };
    if binding.ty != e.ty || binding.ownership != e.ownership {
        return None;
    }
    for entry in &scratch.aliases[..scratch.len] {
        debit(work, 1)?;
        if entry.is_some_and(|entry| entry.binding.id == binding.id) {
            return None;
        }
    }
    if let Value::Vector(_) = value {
        let moved = match &e.kind {
            K::Place(place) if e.ownership == O::Own => Some(place),
            K::Call { callee, args, .. } if callee.as_str() == crate::vec_ops::SORT_OWNED_ID => {
                let [argument] = args.as_slice() else {
                    return None;
                };
                let K::Place(place) = &argument.kind else {
                    return None;
                };
                Some(place)
            }
            _ => None,
        };
        if let Some(place) = moved {
            debit(work, scratch.len)?;
            let old = scratch.aliases[..scratch.len]
                .iter_mut()
                .find(|entry| entry.is_some_and(|entry| entry.binding.id == place.root))?;
            *old = None;
        }
    }
    *scratch.aliases.get_mut(scratch.len)? = Some(Alias { binding, value });
    scratch.len += 1;
    Some(())
}
fn walk<'a>(
    descriptor: &Descriptor<'a>,
    sources: &[source::Node<'a>],
    scratch: &mut Scratch<'a>,
    root: &'a E,
    work: &mut usize,
    emit: &mut impl FnMut(Fact<'a>, &mut usize) -> Option<()>,
) -> Option<()> {
    if let K::Block { statements, .. } = &root.kind {
        debit(work, statements.len())?;
    }
    let context = traversal::enter(descriptor, sources, scratch, root, None, work)?;
    scratch.frames[0] = Some(frame(root, scratch.len, context));
    let mut depth = 1;
    while depth != 0 {
        debit(work, 1)?;
        let top = scratch.frames[depth - 1].as_mut()?;
        if let Some(child) = next(top) {
            if let K::Block { statements, .. } = &child.kind {
                debit(work, statements.len())?;
            }
            let inherited = top.traversal;
            let context = traversal::enter(descriptor, sources, scratch, child, inherited, work)?;
            *scratch.frames.get_mut(depth)? = Some(frame(child, scratch.len, context));
            depth += 1;
        } else {
            let done = scratch.frames[depth - 1].take()?;
            depth -= 1;
            let value = classify(
                descriptor,
                sources,
                scratch,
                done.expression,
                done.traversal,
                work,
            )?;
            if let Some(Value::View(origin)) = value {
                emit(
                    Fact {
                        expression: done.expression,
                        origin,
                    },
                    work,
                )?;
            }
            if matches!(
                done.expression.kind,
                K::Block { .. } | K::VecConstructor { .. }
            ) {
                scratch.len = done.base;
            }
            if depth != 0 {
                if let Some(binding) = scratch.frames[depth - 1].as_ref()?.binding {
                    register(scratch, binding, done.expression, value, work)?;
                }
            }
        }
    }
    Some(())
}
pub(crate) fn derive<'a>(
    source: &'a ast::Program,
    program: &'a hir::ResolvedProgram,
) -> Option<Mapping<'a>> {
    let descriptor = super::derive(source, program)?;
    let mut work = crate::cache_codec::MAX_NODES;
    let sources = source::inventory(source, &mut work)?;
    if !crate::bounded_output::reserve_active_required(size_of::<Scratch<'_>>()) {
        return None;
    }
    let mut scratch = Scratch {
        frames: [None; DEPTH],
        aliases: [None; LOCALS],
        len: 1,
    };
    scratch.aliases[0] = Some(Alias {
        binding: descriptor.typed.vector,
        value: Value::Vector(&descriptor.typed.vector.id),
    });
    let mut count = 0usize;
    for statement in descriptor.typed.suffix {
        debit(&mut work, 1)?;
        let (binding, value) = match statement {
            S::Let { binding, value, .. } => (Some(binding), value),
            S::Assign { value, .. } => (None, value),
            _ => return None,
        };
        walk(
            &descriptor,
            &sources,
            &mut scratch,
            value,
            &mut work,
            &mut |_, _| {
                count = count.checked_add(1)?;
                Some(())
            },
        )?;
        if let Some(binding) = binding {
            let result = classify(&descriptor, &sources, &scratch, value, None, &mut work)?;
            register(&mut scratch, binding, value, result, &mut work)?;
        }
    }
    walk(
        &descriptor,
        &sources,
        &mut scratch,
        descriptor.typed.tail,
        &mut work,
        &mut |_, _| {
            count = count.checked_add(1)?;
            Some(())
        },
    )?;
    let bytes = count.checked_mul(size_of::<Fact<'_>>())?;
    if !crate::bounded_output::reserve_active_required(bytes) {
        return None;
    }
    let mut facts = Vec::new();
    facts.try_reserve_exact(count).ok()?;
    if facts.capacity() != count {
        return None;
    }
    scratch.len = 1;
    scratch.aliases.fill(None);
    scratch.aliases[0] = Some(Alias {
        binding: descriptor.typed.vector,
        value: Value::Vector(&descriptor.typed.vector.id),
    });
    let mut emit = |fact: Fact<'a>, work: &mut usize| {
        debit(work, facts.len())?;
        if facts
            .iter()
            .any(|prior: &Fact<'_>| prior.expression.id == fact.expression.id)
        {
            return None;
        }
        facts.push(fact);
        Some(())
    };
    for statement in descriptor.typed.suffix {
        debit(&mut work, 1)?;
        let (binding, value) = match statement {
            S::Let { binding, value, .. } => (Some(binding), value),
            S::Assign { value, .. } => (None, value),
            _ => return None,
        };
        walk(
            &descriptor,
            &sources,
            &mut scratch,
            value,
            &mut work,
            &mut emit,
        )?;
        if let Some(binding) = binding {
            let result = classify(&descriptor, &sources, &scratch, value, None, &mut work)?;
            register(&mut scratch, binding, value, result, &mut work)?;
        }
    }
    walk(
        &descriptor,
        &sources,
        &mut scratch,
        descriptor.typed.tail,
        &mut work,
        &mut emit,
    )?;
    if facts.len() != count {
        return None;
    }
    let layout = layout::derive(&descriptor, count)?;
    Some(Mapping {
        descriptor,
        facts,
        layout,
    })
}
