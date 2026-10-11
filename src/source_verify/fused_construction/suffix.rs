//! Closed census of every use after the construction loop.
//!
//! The destination owner may only move into an immutable binding, be consumed
//! by `vec_sort_owned`, lend iteration-local row views through `for borrow`,
//! or report `vec_len`. Each move, sort and borrow is recorded as an
//! [`OwnerEvent`] with its exact statement identity. Views never outlive their
//! lexical iteration, rows never bind, and any unknown call, clone, owner
//! mutation, shadowing or use after transfer refuses. This census is source
//! evidence only; it does not validate a runtime lease domain.
use super::shape::{reserved_binding, row_type, Shape};
use super::{OwnerEvent, Refusal, MAX_OWNER_EVENTS};
use crate::ast::{
    BinaryOp, Expr, ExprKind as K, MatchMode, MatchPattern, Statement as S, Type, TypeDeclaration,
    TypeDeclarationKind, UnaryOp,
};

const SLOTS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Ty {
    I64,
    Usize,
    U8,
    Bool,
    Owner,
    Row,
    Str,
    Slice,
    OptionU8,
}

impl Ty {
    fn scalar(self) -> bool {
        matches!(self, Self::I64 | Self::Usize | Self::U8 | Self::Bool)
    }
    fn view(self) -> bool {
        matches!(self, Self::Str | Self::Slice)
    }
}

#[derive(Clone, Copy)]
struct Slot<'p> {
    name: &'p str,
    ty: Ty,
    mutable: bool,
    live: bool,
}

struct Census<'p> {
    record: &'p TypeDeclaration,
    slots: [Option<Slot<'p>>; SLOTS],
    len: usize,
    work: usize,
    events: [Option<OwnerEvent<'p>>; MAX_OWNER_EVENTS],
    recorded: usize,
}

type Step<T> = Option<T>;

impl<'p> Census<'p> {
    fn debit(&mut self) -> Step<()> {
        self.work = self.work.checked_sub(1)?;
        Some(())
    }

    fn live(&self, name: &str) -> Step<Slot<'p>> {
        self.slots[..self.len]
            .iter()
            .flatten()
            .find(|slot| slot.name == name && slot.live)
            .copied()
    }

    fn bind(&mut self, name: &'p str, ty: Ty, mutable: bool) -> Step<()> {
        self.debit()?;
        // A dead name stays reserved: no rebinding can redirect a tracked
        // owner or view to another place.
        if reserved_binding(name)
            || self.slots[..self.len]
                .iter()
                .flatten()
                .any(|slot| slot.name == name)
        {
            return None;
        }
        *self.slots.get_mut(self.len)? = Some(Slot {
            name,
            ty,
            mutable,
            live: true,
        });
        self.len += 1;
        Some(())
    }

    fn record_event(&mut self, event: OwnerEvent<'p>) -> Step<()> {
        *self.events.get_mut(self.recorded)? = Some(event);
        self.recorded += 1;
        Some(())
    }

    /// Invalidate the live owner named by `expression`.
    fn take_owner(&mut self, expression: &Expr) -> Step<()> {
        let K::Var(name) = &expression.kind else {
            return None;
        };
        let slot = self.slots[..self.len]
            .iter_mut()
            .flatten()
            .find(|slot| &slot.name == name && slot.live)?;
        if slot.ty != Ty::Owner {
            return None;
        }
        slot.live = false;
        Some(())
    }

    fn owner_operand(&self, expression: &Expr) -> bool {
        matches!(&expression.kind, K::Var(name)
            if self.live(name).is_some_and(|slot| slot.ty == Ty::Owner))
    }

    fn top(&mut self, statement: &'p S) -> Step<()> {
        self.debit()?;
        match statement {
            S::Let {
                name,
                mutable,
                declared: None,
                value,
                ..
            } if self.owner_operand(value) => {
                if *mutable {
                    return None;
                }
                self.take_owner(value)?;
                self.bind(name, Ty::Owner, false)?;
                self.record_event(OwnerEvent::Move(statement))
            }
            S::Let {
                name,
                mutable,
                declared: None,
                value:
                    Expr {
                        kind:
                            K::Call {
                                name: callee,
                                type_arguments,
                                args,
                            },
                        ..
                    },
                ..
            } if callee == "vec_sort_owned" => {
                let [owner] = args.as_slice() else {
                    return None;
                };
                if *mutable || !row_type(type_arguments, self.record) || !self.owner_operand(owner)
                {
                    return None;
                }
                self.take_owner(owner)?;
                self.bind(name, Ty::Owner, false)?;
                self.record_event(OwnerEvent::Sort(statement))
            }
            S::ForBorrow {
                item,
                index,
                values,
                body,
                ..
            } => {
                let K::Var(owner) = &values.kind else {
                    return None;
                };
                let slot = self.live(owner)?;
                if slot.ty != Ty::Owner || slot.mutable {
                    return None;
                }
                let saved = self.len;
                self.bind(item, Ty::Row, false)?;
                if let Some(index) = index {
                    self.bind(&index.name, Ty::Usize, false)?;
                }
                if self.expression(body, 1)? != Ty::I64 {
                    return None;
                }
                self.len = saved;
                self.record_event(OwnerEvent::Borrow(statement))
            }
            _ => self.nested(statement, 0),
        }
    }

    /// Scalar and iteration-local view statements valid at any depth.
    fn nested(&mut self, statement: &'p S, depth: usize) -> Step<()> {
        self.debit()?;
        match statement {
            S::Let {
                name,
                mutable,
                declared: None,
                value,
                ..
            } => {
                let ty = self.expression(value, depth + 1)?;
                if !(ty.scalar() || (ty.view() && !*mutable)) {
                    return None;
                }
                self.bind(name, ty, *mutable)
            }
            S::Assign {
                name,
                field: None,
                value,
                ..
            } => {
                let slot = self.live(name)?;
                if !slot.mutable || !slot.ty.scalar() {
                    return None;
                }
                (self.expression(value, depth + 1)? == slot.ty).then_some(())
            }
            _ => None,
        }
    }

    fn expression(&mut self, expression: &'p Expr, depth: usize) -> Step<Ty> {
        if depth > crate::cache_codec::MAX_DEPTH {
            return None;
        }
        self.debit()?;
        match &expression.kind {
            K::Int(_) => Some(Ty::I64),
            K::Usize(_) => Some(Ty::Usize),
            K::Uint8(_) => Some(Ty::U8),
            K::Bool(_) => Some(Ty::Bool),
            // A bare owner is a move; only the recorded statement forms and
            // `vec_len` may name it.
            K::Var(name) => Some(self.live(name)?.ty).filter(|ty| *ty != Ty::Owner),
            K::Project { base, field, .. } => {
                if self.expression(base, depth + 1)? != Ty::Row {
                    return None;
                }
                let TypeDeclarationKind::Record { fields } = &self.record.kind else {
                    return None;
                };
                match fields.iter().find(|candidate| &candidate.name == field)?.ty {
                    Type::I64 => Some(Ty::I64),
                    Type::String => Some(Ty::Str),
                    Type::Bytes => Some(Ty::Slice),
                    _ => None,
                }
            }
            K::Unary { op, value } => match (op, self.expression(value, depth + 1)?) {
                (UnaryOp::Not, Ty::Bool) => Some(Ty::Bool),
                (UnaryOp::Neg, Ty::I64) => Some(Ty::I64),
                _ => None,
            },
            K::Binary { op, left, right } => {
                let ty = self.expression(left, depth + 1)?;
                if !ty.scalar() || self.expression(right, depth + 1)? != ty {
                    return None;
                }
                match op {
                    BinaryOp::And | BinaryOp::Or if ty == Ty::Bool => Some(Ty::Bool),
                    BinaryOp::Eq | BinaryOp::Ne => Some(Ty::Bool),
                    BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge if ty != Ty::Bool => {
                        Some(Ty::Bool)
                    }
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul
                        if matches!(ty, Ty::I64 | Ty::Usize) =>
                    {
                        Some(ty)
                    }
                    _ => None,
                }
            }
            K::Block { statements, tail } => {
                let saved = self.len;
                for statement in statements {
                    self.nested(statement, depth + 1)?;
                }
                let ty = self.expression(tail, depth + 1)?;
                self.len = saved;
                ty.scalar().then_some(ty)
            }
            K::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if self.expression(condition, depth + 1)? != Ty::Bool {
                    return None;
                }
                let then_ty = self.expression(then_branch, depth + 1)?;
                let else_ty = self.expression(else_branch, depth + 1)?;
                (then_ty == else_ty && then_ty.scalar()).then_some(then_ty)
            }
            K::Call {
                name,
                type_arguments,
                args,
            } => self.call(name, type_arguments, args, depth),
            K::Match {
                mode: MatchMode::Value,
                scrutinee,
                arms,
            } => {
                if self.expression(scrutinee, depth + 1)? != Ty::OptionU8 || arms.len() != 2 {
                    return None;
                }
                let (mut some, mut none, mut result) = (false, false, None);
                for arm in arms {
                    self.debit()?;
                    let MatchPattern::Variant {
                        type_name,
                        case_name,
                        fields,
                        ..
                    } = &arm.pattern
                    else {
                        return None;
                    };
                    if arm.guard.is_some() || type_name != "Option" {
                        return None;
                    }
                    let saved = self.len;
                    match (case_name.as_str(), fields.as_slice()) {
                        ("Some", [value]) if !some && value.name == "value" => {
                            some = true;
                            self.bind(&value.binding, Ty::U8, false)?;
                        }
                        ("None", []) if !none => none = true,
                        _ => return None,
                    }
                    let ty = self.expression(&arm.value, depth + 1)?;
                    self.len = saved;
                    if !ty.scalar() || result.is_some_and(|previous| previous != ty) {
                        return None;
                    }
                    result = Some(ty);
                }
                result.filter(|_| some && none)
            }
            _ => None,
        }
    }

    fn call(
        &mut self,
        name: &str,
        type_arguments: &[Type],
        args: &'p [Expr],
        depth: usize,
    ) -> Step<Ty> {
        if name == "vec_len" {
            let [owner] = args else {
                return None;
            };
            return (row_type(type_arguments, self.record) && self.owner_operand(owner))
                .then_some(Ty::Usize);
        }
        if !type_arguments.is_empty() {
            return None;
        }
        let (expected, result): (&[Ty], Ty) = match name {
            "i64_from_usize" => (&[Ty::Usize], Ty::I64),
            "str_len_bytes" => (&[Ty::Str], Ty::I64),
            crate::byte_ops::STR_AS_BYTES_NAME => (&[Ty::Str], Ty::Slice),
            crate::byte_ops::LEN_NAME => (&[Ty::Slice], Ty::Usize),
            crate::byte_ops::GET_NAME => (&[Ty::Slice, Ty::Usize], Ty::OptionU8),
            _ => return None,
        };
        if args.len() != expected.len() {
            return None;
        }
        for (arg, ty) in args.iter().zip(expected) {
            if self.expression(arg, depth + 1)? != *ty {
                return None;
            }
        }
        Some(result)
    }
}

/// Census the suffix and tail of a derived shape, returning its owner events.
pub(super) fn census<'p>(
    shape: &Shape<'p>,
) -> Result<[Option<OwnerEvent<'p>>; MAX_OWNER_EVENTS], Refusal> {
    let mut census = Census {
        record: shape.record,
        slots: [None; SLOTS],
        len: 0,
        work: crate::cache_codec::MAX_NODES,
        events: [None; MAX_OWNER_EVENTS],
        recorded: 0,
    };
    let refused = Refusal::Suffix;
    census
        .bind(shape.destination.name, Ty::Owner, true)
        .ok_or(refused)?;
    census
        .bind(shape.induction.name, Ty::I64, true)
        .ok_or(refused)?;
    for statement in shape.suffix {
        census.top(statement).ok_or(refused)?;
    }
    if census.expression(shape.tail, 0) != Some(Ty::I64) {
        return Err(refused);
    }
    Ok(census.events)
}
