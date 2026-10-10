//! Private drift guard captured at the checked source-to-HIR boundary. This
//! canonical binary projection excludes proof plans: those are replayed later.
//! Unsupported semantic nodes refuse, rather than being omitted from the hash.
use crate::hir::*;
use sha2::{Digest, Sha256};
const DOMAIN: &[u8] = b"semaprax.retained-primitive-body.v1\0";
struct Encoder {
    hash: Sha256,
    work: usize,
}
impl Encoder {
    fn bytes(&mut self, bytes: &[u8]) -> Option<()> {
        self.work = self.work.checked_sub(bytes.len().checked_add(1)?)?;
        self.hash
            .update((u64::try_from(bytes.len()).ok()?).to_le_bytes());
        self.hash.update(bytes);
        Some(())
    }
    fn text(&mut self, text: &str) -> Option<()> {
        self.bytes(text.as_bytes())
    }
    fn number(&mut self, n: u64) -> Option<()> {
        self.bytes(&n.to_le_bytes())
    }
    fn span(&mut self, span: crate::ast::Span) -> Option<()> {
        self.number(u64::try_from(span.start).ok()?)?;
        self.number(u64::try_from(span.end).ok()?)?;
        self.number(u64::try_from(span.line).ok()?)?;
        self.number(u64::try_from(span.column).ok()?)
    }
    fn ty(&mut self, ty: &ResolvedType) -> Option<()> {
        if let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        {
            if !arguments.is_empty() {
                return None;
            }
            self.text("nominal")?;
            self.text(declaration.as_str())?;
            return self.number(0);
        }
        self.text(match ty {
            ResolvedType::I64 => "i64",
            ResolvedType::I32 => "i32",
            ResolvedType::Char => "char",
            ResolvedType::U8 => "u8",
            ResolvedType::Usize => "usize",
            ResolvedType::F32 => "f32",
            ResolvedType::F64 => "f64",
            ResolvedType::Bool => "bool",
            ResolvedType::String => "String",
            ResolvedType::Bytes => "Bytes",
            ResolvedType::Str => "str",
            ResolvedType::SliceU8 => "Slice",
            ResolvedType::Never => "Never",
            ResolvedType::ArrayU8(n) => {
                self.text("ArrayU8")?;
                return self.number(u64::from(*n));
            }
            _ => return None,
        })
    }
    fn ownership(&mut self, mode: OwnershipMode) -> Option<()> {
        self.text(match mode {
            OwnershipMode::Value => "value",
            OwnershipMode::Own => "own",
            OwnershipMode::Borrow => "borrow",
            OwnershipMode::Shared => "shared",
        })
    }
    fn binding(&mut self, binding: &ResolvedBinding) -> Option<()> {
        self.span(binding.span)?;
        self.text(binding.id.as_str())?;
        self.text(&binding.name)?;
        self.ty(&binding.ty)?;
        self.ownership(binding.ownership)
    }
    fn place(&mut self, place: &Place) -> Option<()> {
        self.text(place.root.as_str())?;
        self.number(place.projections.len() as u64)?;
        for projection in &place.projections {
            match projection {
                PlaceProjection::Field(field) => {
                    self.text("field")?;
                    self.text(field.as_str())?;
                }
                PlaceProjection::VariantField { case, field } => {
                    self.text("variant-field")?;
                    self.text(case.as_str())?;
                    self.text(field.as_str())?;
                }
            }
        }
        Some(())
    }
    fn owner(&mut self, owner: &FunctionExecutionId) -> Option<()> {
        match owner {
            FunctionExecutionId::Monomorphic(id) => {
                self.text("mono")?;
                self.text(id.as_str())
            }
            FunctionExecutionId::Generic(_) => None,
        }
    }
    fn pattern(&mut self, pattern: &ResolvedMatchPattern, depth: usize) -> Option<()> {
        if depth > 256 {
            return None;
        }
        match pattern {
            ResolvedMatchPattern::Wildcard => self.text("wildcard"),
            ResolvedMatchPattern::Binding(binding) => {
                self.text("binding")?;
                self.binding(binding)
            }
            ResolvedMatchPattern::Literal(value) => {
                self.text("literal")?;
                match value {
                    PatternValue::Int(n) => {
                        self.text("i64")?;
                        self.bytes(&n.to_le_bytes())
                    }
                    PatternValue::Int32(n) => {
                        self.text("i32")?;
                        self.bytes(&n.to_le_bytes())
                    }
                    PatternValue::Uint8(n) => {
                        self.text("u8")?;
                        self.bytes(&[*n])
                    }
                    PatternValue::Usize(n) => {
                        self.text("usize")?;
                        self.number(*n)
                    }
                    PatternValue::Char(n) => {
                        self.text("char")?;
                        self.bytes(&n.to_le_bytes())
                    }
                    PatternValue::Bool(n) => {
                        self.text("bool")?;
                        self.bytes(&[u8::from(*n)])
                    }
                }
            }
            ResolvedMatchPattern::Or(alternatives) => {
                self.text("or")?;
                self.number(alternatives.len() as u64)?;
                for p in alternatives {
                    self.pattern(p, depth + 1)?;
                }
                Some(())
            }
            ResolvedMatchPattern::Variant { .. } | ResolvedMatchPattern::Record { .. } => None,
        }
    }
    fn expression(&mut self, expression: &ResolvedExpr, depth: usize) -> Option<()> {
        if depth > 256 {
            return None;
        }
        self.span(expression.span)?;
        self.text(expression.id.as_str())?;
        self.ty(&expression.ty)?;
        self.ownership(expression.ownership)?;
        use ResolvedExprKind as E;
        match &expression.kind {
            E::Int(n) => {
                self.text("int")?;
                self.bytes(&n.to_le_bytes())
            }
            E::Int32(n) => {
                self.text("int32")?;
                self.bytes(&n.to_le_bytes())
            }
            E::Char(n) => {
                self.text("char")?;
                self.bytes(&n.to_le_bytes())
            }
            E::Uint8(n) => {
                self.text("u8")?;
                self.bytes(&[*n])
            }
            E::Usize(n) => {
                self.text("usize")?;
                self.number(*n)
            }
            E::Float32(n) => {
                self.text("f32bits")?;
                self.bytes(&n.to_le_bytes())
            }
            E::Float64(n) => {
                self.text("f64bits")?;
                self.number(*n)
            }
            E::Bool(n) => {
                self.text("bool")?;
                self.bytes(&[u8::from(*n)])
            }
            E::ArrayU8(bytes) => {
                self.text("array")?;
                self.bytes(bytes)
            }
            E::RepeatArrayU8 { value, count } => {
                self.text("repeat-array")?;
                self.bytes(&[*value])?;
                self.number(u64::from(*count))
            }
            E::String(text) => {
                self.text("string")?;
                self.text(text)
            }
            E::BorrowedUtf8Literal(text) => {
                if !text.is_backed() {
                    return None;
                }
                self.text("static-str")?;
                self.text(text.as_str())
            }
            E::Place(place) => {
                self.text("place")?;
                self.place(place)
            }
            E::BorrowPlace { operation, place } => {
                self.text("borrow-place")?;
                self.text(operation.as_str())?;
                self.place(place)
            }
            E::ByteRange {
                operation,
                source,
                start,
                end,
            } => {
                self.text("byte-range")?;
                self.text(operation.as_str())?;
                self.expression(source, depth + 1)?;
                self.expression(start, depth + 1)?;
                self.expression(end, depth + 1)
            }
            E::Call {
                callee,
                type_arguments,
                instance,
                args,
            } => {
                if !type_arguments.is_empty() || instance.is_some() {
                    return None;
                }
                self.text("call")?;
                self.text(callee.as_str())?;
                self.number(args.len() as u64)?;
                for arg in args {
                    self.expression(arg, depth + 1)?;
                }
                Some(())
            }
            E::Unary { op, value } => {
                self.text("unary")?;
                self.text(match op {
                    crate::ast::UnaryOp::Neg => "neg",
                    crate::ast::UnaryOp::Not => "not",
                })?;
                self.expression(value, depth + 1)
            }
            E::Binary { op, left, right } => {
                self.text("binary")?;
                self.text(op.text())?;
                self.expression(left, depth + 1)?;
                self.expression(right, depth + 1)
            }
            E::Block { statements, tail } => {
                self.text("block")?;
                self.number(statements.len() as u64)?;
                for statement in statements {
                    self.statement(statement, depth + 1)?;
                }
                self.expression(tail, depth + 1)
            }
            E::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.text("if")?;
                self.expression(condition, depth + 1)?;
                self.expression(then_branch, depth + 1)?;
                self.expression(else_branch, depth + 1)
            }
            E::Match {
                mode,
                scrutinee,
                arms,
            } => {
                self.text("match")?;
                self.text(match mode {
                    ResolvedMatchMode::Value => "value",
                    ResolvedMatchMode::Own => "own",
                    ResolvedMatchMode::Borrow => "borrow",
                })?;
                self.expression(scrutinee, depth + 1)?;
                self.number(arms.len() as u64)?;
                for arm in arms {
                    self.span(arm.span)?;
                    self.pattern(&arm.pattern, 0)?;
                    self.bytes(&[u8::from(arm.guard.is_some())])?;
                    if let Some(guard) = &arm.guard {
                        self.expression(guard, depth + 1)?;
                    }
                    self.expression(&arm.value, depth + 1)?;
                }
                Some(())
            }
            E::LexicalExit {
                kind,
                target,
                value,
            } => {
                self.text("exit")?;
                self.text(match kind {
                    crate::ast::LexicalExitKind::Break => "break",
                    crate::ast::LexicalExitKind::Continue => "continue",
                    crate::ast::LexicalExitKind::Return => "return",
                })?;
                match target {
                    ControlTarget::Function(owner) => {
                        self.text("function")?;
                        self.owner(owner)?;
                    }
                    ControlTarget::Loop { owner, site } => {
                        self.text("loop")?;
                        self.owner(owner)?;
                        self.text(site.as_str())?;
                    }
                }
                self.bytes(&[u8::from(value.is_some())])?;
                if let Some(value) = value {
                    self.expression(value, depth + 1)?;
                }
                Some(())
            }
            E::ConstructRecord { record, fields } => {
                self.text("construct-record")?;
                self.text(record.as_str())?;
                self.number(fields.len() as u64)?;
                for field in fields {
                    self.text(field.field.as_str())?;
                    self.expression(&field.value, depth + 1)?;
                }
                Some(())
            }
            E::Project { base, field } => {
                self.text("project")?;
                self.text(field.as_str())?;
                self.expression(base, depth + 1)
            }
            E::Closure { .. }
            | E::FunctionReference { .. }
            | E::Invoke { .. }
            | E::LiteralFormat { .. }
            | E::VecFieldRead { .. }
            | E::NativeRustImportCall(_)
            | E::HostCommandCall(_)
            | E::ConstructVariant { .. }
            | E::Try { .. }
            | E::TryOption { .. }
            | E::UpdateRecord { .. }
            | E::Upcast { .. }
            | E::Yield { .. } => None,
        }
    }
    fn statement(&mut self, statement: &ResolvedStatement, depth: usize) -> Option<()> {
        if depth > 256 {
            return None;
        }
        self.span(match statement {
            ResolvedStatement::Let { span, .. }
            | ResolvedStatement::Assign { span, .. }
            | ResolvedStatement::While { span, .. }
            | ResolvedStatement::LexicalWhile { span, .. }
            | ResolvedStatement::Unsafe { span, .. } => *span,
        })?;
        match statement {
            ResolvedStatement::Let {
                binding,
                mutable,
                value,
                ..
            } => {
                self.text("let")?;
                self.binding(binding)?;
                self.bytes(&[u8::from(*mutable)])?;
                self.expression(value, depth)
            }
            ResolvedStatement::Assign {
                binding,
                field,
                value,
                ..
            } => {
                self.text("assign")?;
                self.binding(binding)?;
                self.bytes(&[u8::from(field.is_some())])?;
                if let Some(field) = field {
                    self.text(field.as_str())?;
                }
                self.expression(value, depth)
            }
            ResolvedStatement::While {
                condition, body, ..
            } => {
                self.text("while")?;
                self.expression(condition, depth)?;
                self.expression(body, depth)
            }
            ResolvedStatement::LexicalWhile {
                condition,
                body,
                latch,
                ..
            } => {
                if *latch != LoopLatchKind::Condition {
                    return None;
                }
                self.text("lexical-while-condition")?;
                self.expression(condition, depth)?;
                self.expression(body, depth)
            }
            ResolvedStatement::Unsafe { .. } => None,
        }
    }
}
pub(super) fn typed(function: &ResolvedFunction, source_revision: &[u8; 32]) -> Option<[u8; 32]> {
    let mut encoder = Encoder {
        hash: Sha256::new(),
        work: 1_048_576,
    };
    encoder.bytes(DOMAIN)?;
    encoder.bytes(source_revision)?;
    encoder.span(function.span)?;
    encoder.text(function.id.as_str())?;
    encoder.text(&function.name)?;
    encoder.ty(&function.return_type)?;
    encoder.text(function.result_id.as_str())?;
    encoder.number(function.params.len() as u64)?;
    for param in &function.params {
        encoder.span(param.span)?;
        encoder.text(param.id.as_str())?;
        encoder.text(&param.name)?;
        encoder.ty(&param.ty)?;
        encoder.ownership(param.ownership)?;
    }
    if !function.effects.is_empty() || function.yields.is_some() {
        return None;
    }
    encoder.text("requires")?;
    encoder.number(function.requires.len() as u64)?;
    for expression in &function.requires {
        encoder.expression(expression, 0)?;
    }
    encoder.text("body")?;
    encoder.expression(&function.body, 0)?;
    encoder.text("ensures")?;
    encoder.number(function.ensures.len() as u64)?;
    for expression in &function.ensures {
        encoder.expression(expression, 0)?;
    }
    Some(encoder.hash.finalize().into())
}
pub(super) fn source(path: &str, module: &str, source: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"semaprax.retained-primitive-source.v1\0");
    for bytes in [path.as_bytes(), module.as_bytes(), source.as_bytes()] {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    hash.finalize().into()
}

pub(super) fn scratch_bytes() -> usize {
    // Fixed SHA state; no formatter or materialized source is created.
    // Expression recursion is independently bounded to 256 and constructs no heap.
    std::mem::size_of::<Encoder>() + std::mem::size_of::<Sha256>()
}
