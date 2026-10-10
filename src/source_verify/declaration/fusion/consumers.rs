//! Closed source-only use census. This does not validate a runtime lease domain.
use super::*;
use crate::ast::{MatchMode, MatchPattern, UnaryOp};
const SLOTS: usize = 64;
#[derive(Clone, Copy, Eq, PartialEq)]
enum Ty {
    I64,
    Usize,
    U8,
    Bool,
    Vector,
    Row,
    Str,
    Slice,
    OptionU8,
}
impl Ty {
    fn scalar(self) -> bool {
        matches!(self, Self::I64 | Self::Usize | Self::U8 | Self::Bool)
    }
}
#[derive(Clone, Copy)]
struct Binding<'a> {
    name: &'a str,
    ty: Ty,
    mutable: bool,
    live: bool,
}
struct Census<'a> {
    record: &'a TypeDeclaration,
    bindings: [Option<Binding<'a>>; SLOTS],
    len: usize,
    work: usize,
}
impl<'a> Census<'a> {
    fn debit(&mut self) -> Option<()> {
        self.work = self.work.checked_sub(1)?;
        Some(())
    }
    fn lookup(&self, name: &str) -> Option<Binding<'a>> {
        self.bindings[..self.len]
            .iter()
            .flatten()
            .find(|b| b.name == name && b.live)
            .copied()
    }
    fn bind(&mut self, name: &'a str, ty: Ty, mutable: bool) -> Option<()> {
        self.debit()?;
        // No shadowing can redirect a tracked owner or borrow to another place.
        if self.bindings[..self.len]
            .iter()
            .flatten()
            .any(|b| b.name == name)
        {
            return None;
        }
        *self.bindings.get_mut(self.len)? = Some(Binding {
            name,
            ty,
            mutable,
            live: true,
        });
        self.len += 1;
        Some(())
    }
    fn take_vector(&mut self, expression: &Expr) -> Option<()> {
        let K::Var(name) = &expression.kind else {
            return None;
        };
        let binding = self.bindings[..self.len]
            .iter_mut()
            .flatten()
            .find(|b| b.name == name && b.live)?;
        if binding.ty != Ty::Vector {
            return None;
        }
        binding.live = false;
        Some(())
    }
    fn statements(&mut self, statements: &'a [S], depth: usize, top: bool) -> Option<()> {
        if depth > crate::cache_codec::MAX_DEPTH {
            return None;
        }
        for statement in statements {
            self.debit()?;
            match statement {
                S::Let {
                    name,
                    mutable,
                    declared: None,
                    value,
                    ..
                } => {
                    let ty = if top
                        && matches!(&value.kind, K::Var(n) if self.lookup(n).is_some_and(|b| b.ty == Ty::Vector))
                    {
                        self.take_vector(value)?;
                        Ty::Vector
                    } else if top {
                        if let Some((types, args)) = call(value, "vec_sort_owned") {
                            let [owner] = args else {
                                return None;
                            };
                            if !row_type(types, self.record) {
                                return None;
                            }
                            self.take_vector(owner)?;
                            Ty::Vector
                        } else {
                            self.expression(value, depth + 1)?
                        }
                    } else {
                        self.expression(value, depth + 1)?
                    };
                    if matches!(ty, Ty::Row | Ty::OptionU8) || (*mutable && !ty.scalar()) {
                        return None;
                    }
                    self.bind(name, ty, *mutable)?;
                }
                S::Assign {
                    name,
                    field: None,
                    value,
                    ..
                } => {
                    let binding = self.lookup(name)?;
                    if !binding.mutable
                        || !binding.ty.scalar()
                        || self.expression(value, depth + 1)? != binding.ty
                    {
                        return None;
                    }
                }
                S::ForBorrow {
                    item,
                    index,
                    values,
                    body,
                    ..
                } if top => {
                    let K::Var(owner) = &values.kind else {
                        return None;
                    };
                    let binding = self.lookup(owner)?;
                    if binding.ty != Ty::Vector || binding.mutable {
                        return None;
                    }
                    let saved = self.len;
                    self.bind(item, Ty::Row, false)?;
                    if let Some(index) = index {
                        self.bind(&index.name, Ty::Usize, false)?;
                    }
                    if self.expression(body, depth + 1)? != Ty::I64 {
                        return None;
                    }
                    self.len = saved;
                }
                _ => return None,
            }
        }
        Some(())
    }
    fn expression(&mut self, expression: &'a Expr, depth: usize) -> Option<Ty> {
        if depth > crate::cache_codec::MAX_DEPTH {
            return None;
        }
        self.debit()?;
        match &expression.kind {
            K::Int(_) => Some(Ty::I64),
            K::Usize(_) => Some(Ty::Usize),
            K::Uint8(_) => Some(Ty::U8),
            K::Bool(_) => Some(Ty::Bool),
            K::Var(name) => Some(self.lookup(name)?.ty),
            K::Project { base, field, .. } => {
                if self.expression(base, depth + 1)? != Ty::Row {
                    return None;
                }
                let TypeDeclarationKind::Record { fields } = &self.record.kind else {
                    return None;
                };
                match &fields.iter().find(|f| &f.name == field)?.ty {
                    Type::I64 => Some(Ty::I64),
                    Type::String => Some(Ty::Str),
                    Type::Bytes => Some(Ty::Slice),
                    _ => None,
                }
            }
            K::Unary { op, value } => {
                let ty = self.expression(value, depth + 1)?;
                match (op, ty) {
                    (UnaryOp::Not, Ty::Bool) | (UnaryOp::Neg, Ty::I64) => Some(ty),
                    _ => None,
                }
            }
            K::Binary { op, left, right } => {
                let ty = self.expression(left, depth + 1)?;
                if self.expression(right, depth + 1)? != ty || !ty.scalar() {
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
                self.statements(statements, depth + 1, false)?;
                let result = self.expression(tail, depth + 1)?;
                self.len = saved;
                result.scalar().then_some(result)
            }
            K::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if self.expression(condition, depth + 1)? != Ty::Bool {
                    return None;
                }
                let left = self.expression(then_branch, depth + 1)?;
                let right = self.expression(else_branch, depth + 1)?;
                (left == right && left.scalar()).then_some(left)
            }
            K::Call {
                name,
                type_arguments,
                args,
            } => {
                if name == "vec_len" {
                    if !row_type(type_arguments, self.record)
                        || args.len() != 1
                        || self.expression(&args[0], depth + 1)? != Ty::Vector
                    {
                        return None;
                    }
                    return Some(Ty::Usize);
                }
                if !type_arguments.is_empty() {
                    return None;
                }
                let expected: &[Ty] = match name.as_str() {
                    "i64_from_usize" => &[Ty::Usize],
                    "str_len_bytes" | "str_as_bytes" => &[Ty::Str],
                    "byte_len" => &[Ty::Slice],
                    "byte_get" => &[Ty::Slice, Ty::Usize],
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
                match name.as_str() {
                    "i64_from_usize" | "str_len_bytes" => Some(Ty::I64),
                    "str_as_bytes" => Some(Ty::Slice),
                    "byte_len" => Some(Ty::Usize),
                    "byte_get" => Some(Ty::OptionU8),
                    _ => None,
                }
            }
            K::Match {
                mode: MatchMode::Value,
                scrutinee,
                arms,
            } => {
                if self.expression(scrutinee, depth + 1)? != Ty::OptionU8 || arms.len() != 2 {
                    return None;
                }
                let mut some = false;
                let mut none = false;
                let mut result = None;
                for arm in arms {
                    self.debit()?;
                    if arm.guard.is_some() {
                        return None;
                    }
                    let MatchPattern::Variant {
                        type_name,
                        case_name,
                        fields,
                        ..
                    } = &arm.pattern
                    else {
                        return None;
                    };
                    if type_name != "Option" {
                        return None;
                    }
                    let saved = self.len;
                    match case_name.as_str() {
                        "Some" if !some && fields.len() == 1 && fields[0].name == "value" => {
                            some = true;
                            self.bind(&fields[0].binding, Ty::U8, false)?;
                        }
                        "None" if !none && fields.is_empty() => none = true,
                        _ => return None,
                    }
                    let ty = self.expression(&arm.value, depth + 1)?;
                    self.len = saved;
                    if !ty.scalar() || result.is_some_and(|old| old != ty) {
                        return None;
                    }
                    result = Some(ty);
                }
                if some && none {
                    result
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
pub(super) fn validate<'a>(
    record: &'a TypeDeclaration,
    vector: &'a str,
    induction: &'a str,
    statements: &'a [S],
    tail: &'a Expr,
) -> Option<()> {
    if !crate::bounded_output::reserve_active_preserving(std::mem::size_of::<Census<'_>>(), 0) {
        return None;
    }
    let mut census = Census {
        record,
        bindings: [None; SLOTS],
        len: 0,
        work: crate::cache_codec::MAX_NODES,
    };
    census.bind(vector, Ty::Vector, true)?;
    census.bind(induction, Ty::I64, true)?;
    census.statements(statements, 0, true)?;
    (census.expression(tail, 0)? == Ty::I64).then_some(())
}
