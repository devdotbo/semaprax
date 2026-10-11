//! Private source/HIR partition operation proof. No ordinary intrinsic registry.
//! Full validated source/HIR, call-commit/loan replay and a complete selected
//! executor remain prerequisites; this descriptor cannot supply those leases.
use super::domain::Refusal;
use crate::{ast, hir};
use std::fmt::{self, Write};
pub(crate) const NAME: &str = "bytes_partition";
pub(crate) const ID: &str = "core.bytes.partition";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Fault {
    WidthZero,
    ExtentOverflow,
    ExtentMismatch,
    MetadataCapacity,
}
/// Authority failures never become normalized partition outcomes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Preparation {
    Authority(Refusal),
    Fault(Fault),
}
impl From<Fault> for Preparation {
    fn from(value: Fault) -> Self {
        Self::Fault(value)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Geometry {
    pub(crate) count: usize,
    // Semantic usize is target-independent u64. In the empty case its value
    // need not fit an address-space word because no range is formed.
    pub(crate) width: u64,
    pub(crate) extent: usize,
}
/// Semantic preflight order. Symbols have NO normalized numeric code yet.
/// The backing extent must come from the executor's authenticated whole owner.
pub(crate) fn geometry(extent: u64, count: u64, width: u64) -> Result<Geometry, Preparation> {
    if extent > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES {
        return Err(Preparation::Authority(Refusal::Range));
    }
    if width == 0 {
        return Err(Fault::WidthZero.into());
    }
    let total = count
        .checked_mul(width)
        .ok_or(Preparation::Fault(Fault::ExtentOverflow))?;
    if total != extent {
        return Err(Fault::ExtentMismatch.into());
    }
    if count > crate::vec_ops::MAX_CAPACITY {
        return Err(Fault::MetadataCapacity.into());
    }
    Ok(Geometry {
        count: usize::try_from(count).map_err(|_| Preparation::Fault(Fault::MetadataCapacity))?,
        width,
        extent: usize::try_from(extent).map_err(|_| Preparation::Fault(Fault::MetadataCapacity))?,
    })
}
pub(crate) struct Proof<'a> {
    source: &'a ast::Program,
    program: &'a hir::ResolvedProgram,
    authored: &'a ast::Function,
    function: &'a hir::ResolvedFunction,
    pub(crate) producer: &'a hir::ResolvedExpr,
    pub(crate) binding: &'a hir::ResolvedBinding,
    pub(crate) call: &'a hir::ResolvedExpr,
    args: &'a [hir::ResolvedExpr],
    extent: u64,
    count: u64,
    width: u64,
}
impl Proof<'_> {
    /// Exact left-to-right preparation order; one owned operand commits only
    /// after all scalar preflight and real executor metadata reservations.
    pub(crate) fn operands(&self) -> &[hir::ResolvedExpr] {
        self.args
    }
    pub(crate) fn still_held(
        &self,
        source: &ast::Program,
        program: &hir::ResolvedProgram,
        authored: &ast::Function,
        function: &hir::ResolvedFunction,
    ) -> bool {
        std::ptr::eq(self.source, source)
            && std::ptr::eq(self.program, program)
            && std::ptr::eq(self.authored, authored)
            && std::ptr::eq(self.function, function)
    }
    pub(crate) fn prepare(&self, actual_whole_extent: u64) -> Result<Geometry, Preparation> {
        // No caller-provided dimensions or guessed owner length can replace
        // independently recovered ordinary producer extent and operand values.
        if actual_whole_extent != self.extent {
            return Err(Preparation::Authority(Refusal::Identity));
        }
        geometry(actual_whole_extent, self.count, self.width)
    }
}
fn debit(work: &mut usize, amount: usize) -> Result<(), Refusal> {
    match work.checked_sub(amount) {
        Some(n) => {
            *work = n;
            Ok(())
        }
        None => {
            *work = 0;
            Err(Refusal::Exhausted)
        }
    }
}
fn equal(a: &str, b: &str, work: &mut usize) -> Result<bool, Refusal> {
    debit(work, a.len().min(b.len()).saturating_add(1))?;
    Ok(a == b)
}
fn source_call<'a>(
    e: &'a ast::Expr,
    name: &str,
    work: &mut usize,
) -> Result<&'a [ast::Expr], Refusal> {
    let ast::ExprKind::Call {
        name: actual,
        type_arguments,
        args,
    } = &e.kind
    else {
        return Err(Refusal::Identity);
    };
    if !equal(actual, name, work)? || !type_arguments.is_empty() {
        return Err(Refusal::Identity);
    }
    Ok(args)
}
fn typed_call<'a>(
    e: &'a hir::ResolvedExpr,
    id: &str,
    work: &mut usize,
) -> Result<&'a [hir::ResolvedExpr], Refusal> {
    let hir::ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &e.kind
    else {
        return Err(Refusal::Identity);
    };
    if !equal(callee.as_str(), id, work)? || !type_arguments.is_empty() {
        return Err(Refusal::Identity);
    }
    Ok(args)
}
fn usize_value(e: &hir::ResolvedExpr) -> Option<u64> {
    match e.kind {
        hir::ResolvedExprKind::Usize(n)
            if e.ty == hir::ResolvedType::Usize && e.ownership == hir::OwnershipMode::Value =>
        {
            Some(n)
        }
        _ => None,
    }
}
fn source_usize(e: &ast::Expr) -> Option<u64> {
    if let ast::ExprKind::Usize(n) = e.kind {
        Some(n)
    } else {
        None
    }
}
fn vec_bytes(ty: &hir::ResolvedType, work: &mut usize) -> Result<bool, Refusal> {
    let hir::ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    Ok(equal(declaration.as_str(), crate::prelude::VEC_ID, work)?
        && arguments.len() == 1
        && matches!(arguments[0], hir::ResolvedType::Bytes))
}
fn source_vec_bytes(ty: &ast::Type, work: &mut usize) -> Result<bool, Refusal> {
    let ast::Type::Named { name, arguments } = ty else {
        return Ok(false);
    };
    Ok(equal(name, "Vec", work)?
        && arguments.len() == 1
        && matches!(arguments[0], ast::Type::Bytes))
}
/// Scan borrowed current declarations with charged comparisons rather than
/// using an unmetered tree lookup or cloning a synthetic callee identity.
fn declaration(
    program: &hir::ResolvedProgram,
    id: &str,
    origin: hir::IdentityOrigin,
    work: &mut usize,
) -> Result<(), Refusal> {
    for declaration in program.declarations.declarations() {
        debit(work, 1)?;
        if equal(declaration.id.as_str(), id, work)? {
            return if declaration.kind == hir::DeclarationKind::Function
                && declaration.identity_origin == origin
            {
                Ok(())
            } else {
                Err(Refusal::Identity)
            };
        }
    }
    Err(Refusal::Identity)
}
/// Allocation-free exact canonical identity comparison for monomorphic sites.
struct Identity<'a> {
    actual: &'a str,
    next: usize,
    work: &'a mut usize,
}
impl Write for Identity<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        debit(self.work, s.len().saturating_add(1)).map_err(|_| fmt::Error)?;
        let end = self.next.checked_add(s.len()).ok_or(fmt::Error)?;
        if self.actual.as_bytes().get(self.next..end) != Some(s.as_bytes()) {
            return Err(fmt::Error);
        }
        self.next = end;
        Ok(())
    }
}
fn identity(
    actual: &str,
    owner: &str,
    kind: &str,
    path: &str,
    work: &mut usize,
) -> Result<(), Refusal> {
    let mut sink = Identity {
        actual,
        next: 0,
        work,
    };
    write!(
        &mut sink,
        "declaration:{}:{}:{}:{}:{}",
        owner.len(),
        owner,
        kind,
        path.len(),
        path
    )
    .map_err(|_| Refusal::Identity)?;
    if sink.next != actual.len() {
        return Err(Refusal::Identity);
    }
    Ok(())
}
/// First receiving tranche: a closed monomorphic body with one immutable
/// ordinary zeroed whole owner followed immediately by partition. No aliases,
/// views, intervening effects, templates, contracts or unattested child origins.
/// Literal dimensions deliberately leave general computed operands CLOSED.
pub(crate) fn derive<'a>(
    source: &'a ast::Program,
    program: &'a hir::ResolvedProgram,
    authored: &'a ast::Function,
    function: &'a hir::ResolvedFunction,
    work: &mut usize,
) -> Result<Proof<'a>, Refusal> {
    let result = (|| {
        let mut source_present = false;
        for f in &source.functions {
            debit(work, 1)?;
            source_present |= std::ptr::eq(f, authored);
            // Spelling alone cannot resolve a compiler operation when an
            // authored function could capture its name or persistent identity.
            if equal(&f.name, crate::byte_ops::ZEROED_NAME, work)?
                || equal(&f.name, NAME, work)?
                || equal(&f.stable_id, crate::byte_ops::ZEROED_ID, work)?
                || equal(&f.stable_id, ID, work)?
            {
                return Err(Refusal::Identity);
            }
        }
        let mut typed_present = false;
        for f in &program.functions {
            debit(work, 1)?;
            typed_present |= std::ptr::eq(f, function);
        }
        if !source_present
            || !typed_present
            || !equal(&source.module, &program.module, work)?
            || !equal(&authored.stable_id, function.id.as_str(), work)?
            || !equal(&authored.name, &function.name, work)?
            || authored.span != function.span
            || !authored.explicit_id
            || !authored.type_parameters.is_empty()
            || !authored.params.is_empty()
            || !function.params.is_empty()
            || !authored.effects.is_empty()
            || !function.effects.is_empty()
            || authored.yields.is_some()
            || function.yields.is_some()
            || !authored.requires.is_empty()
            || !authored.ensures.is_empty()
            || !function.requires.is_empty()
            || !function.ensures.is_empty()
            || !source.module_uses.is_empty()
            || !source.types.is_empty()
            || !source.interfaces.is_empty()
            || !source.protocols.is_empty()
            || !source.implementations.is_empty()
            || !source_vec_bytes(&authored.return_type, work)?
            || !vec_bytes(&function.return_type, work)?
        {
            return Err(Refusal::Identity);
        }
        declaration(
            program,
            function.id.as_str(),
            hir::IdentityOrigin::Explicit,
            work,
        )?;
        let ast::ExprKind::Block {
            statements: source_statements,
            tail: source_tail,
        } = &authored.body.kind
        else {
            return Err(Refusal::Identity);
        };
        let hir::ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
            return Err(Refusal::Identity);
        };
        debit(
            work,
            source_statements
                .len()
                .checked_add(statements.len())
                .ok_or(Refusal::Exhausted)?,
        )?;
        let [
            ast::Statement::Let {
                syntax: ast::LetSyntax::Authored,
                name,
                mutable: false,
                declared,
                value: original,
                span: source_statement_span,
                ..
            },
        ] = source_statements.as_slice()
        else {
            return Err(Refusal::Identity);
        };
        let [
            hir::ResolvedStatement::Let {
                binding,
                mutable: false,
                value: producer,
                span: typed_statement_span,
                ..
            },
        ] = statements.as_slice()
        else {
            return Err(Refusal::Identity);
        };
        if authored.body.span != function.body.span
            || source_statement_span != typed_statement_span
            || !matches!(declared, None | Some(ast::Type::Bytes))
            || equal(name, crate::byte_ops::ZEROED_NAME, work)?
            || equal(name, NAME, work)?
            || !equal(name, &binding.name, work)?
            || binding.ty != hir::ResolvedType::Bytes
            || binding.ownership != hir::OwnershipMode::Own
            || producer.ty != hir::ResolvedType::Bytes
            || producer.ownership != hir::OwnershipMode::Own
            || original.span != producer.span
            || source_tail.span != tail.span
            || !vec_bytes(&tail.ty, work)?
            || tail.ownership != hir::OwnershipMode::Own
            || !vec_bytes(&function.body.ty, work)?
            || function.body.ownership != hir::OwnershipMode::Own
        {
            return Err(Refusal::Identity);
        }
        let [source_extent] = source_call(original, crate::byte_ops::ZEROED_NAME, work)? else {
            return Err(Refusal::Identity);
        };
        let [typed_extent] = typed_call(producer, crate::byte_ops::ZEROED_ID, work)? else {
            return Err(Refusal::Identity);
        };
        let extent = source_usize(source_extent).ok_or(Refusal::Identity)?;
        if extent > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES
            || usize_value(typed_extent) != Some(extent)
            || source_extent.span != typed_extent.span
        {
            return Err(Refusal::Identity);
        }
        let [source_owner, source_count, source_width] = source_call(source_tail, NAME, work)?
        else {
            return Err(Refusal::Identity);
        };
        let args = typed_call(tail, ID, work)?;
        let [owner, count, width] = args else {
            return Err(Refusal::Identity);
        };
        let ast::ExprKind::Var(source_name) = &source_owner.kind else {
            return Err(Refusal::Identity);
        };
        let hir::ResolvedExprKind::Place(place) = &owner.kind else {
            return Err(Refusal::Identity);
        };
        if !equal(source_name, name, work)?
            || !equal(place.root.as_str(), binding.id.as_str(), work)?
            || !place.projections.is_empty()
            || owner.ty != hir::ResolvedType::Bytes
            || owner.ownership != hir::OwnershipMode::Own
            || source_owner.span != owner.span
            || source_count.span != count.span
            || source_width.span != width.span
        {
            return Err(Refusal::Identity);
        }
        let n = source_usize(source_count).ok_or(Refusal::Identity)?;
        let w = source_usize(source_width).ok_or(Refusal::Identity)?;
        if usize_value(count) != Some(n) || usize_value(width) != Some(w) {
            return Err(Refusal::Identity);
        }
        let id = function.id.as_str();
        for (expr, path) in [
            (&function.body, "body"),
            (producer, "body.s0.value"),
            (typed_extent, "body.s0.value.arg.0"),
            (tail.as_ref(), "body.tail"),
            (owner, "body.tail.arg.0"),
            (count, "body.tail.arg.1"),
            (width, "body.tail.arg.2"),
        ] {
            identity(expr.id.as_str(), id, "expression", path, work)?;
        }
        identity(binding.id.as_str(), id, "value:local", "body.s0", work)?;
        Ok(Proof {
            source,
            program,
            authored,
            function,
            producer,
            binding,
            call: tail,
            args,
            extent,
            count: n,
            width: w,
        })
    })();
    if result.is_err() {
        *work = 0;
    }
    result
}
#[cfg(test)]
mod tests;
