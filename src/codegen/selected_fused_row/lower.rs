//! Closed-domain lowering of one selected construction function.
//!
//! The construction prefix is bound to the selection's exact borrowed sites.
//! The suffix lowers only the census's closed use domain; every other
//! expression, call or carrier crossing refuses before any text is returned.
//! String and Bytes views take their representation from the producer-bound
//! carrier fact for that exact expression, never from `ResolvedType` alone.
//!
//! Ownership order is the attached canonical cleanup plan's: liveness flags
//! follow its transitions at their exact expressions, every ordinary failure
//! jumps to the plan's own exit for that status source, and each exit's
//! finalizers are emitted verbatim. Private carriers (staged rows, row Vec
//! headers, leased views) live in the admitted heap frame, never the C stack.
//! Inside the function no accounting can refuse: domain authority failures are
//! invariant fail-stops, and only plan-backed ordinary statuses leave early.
use super::{cleanup::PlanProof, error, selection::Sites};
use crate::construction_fusion::native_provenance::Carrier;
use crate::diagnostic::Diagnostic;
use crate::hir::{
    OwnershipMode as O, ResolvedBinding, ResolvedExpr as E, ResolvedExprKind as K,
    ResolvedStatement as S, ResolvedType as T, ValueId,
};
use std::fmt::Write as _;
mod expr;
mod plan;
mod prefix;
use plan::Physical;

/// Selected row field roles in record declaration order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Role {
    Scalar,
    Text,
    Payload,
}
impl Role {
    pub(super) fn c(self) -> &'static str {
        match self {
            Self::Scalar => "SPX_FUSED_SCALAR",
            Self::Text => "SPX_FUSED_STRING",
            Self::Payload => "SPX_FUSED_PAYLOAD",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    I64,
    Usize,
    Bool,
    U8,
    Rows,
    Staged,
    Text,
    Leased,
}
impl Kind {
    fn c(self) -> &'static str {
        match self {
            Self::I64 => "int64_t",
            Self::Usize => "uint64_t",
            Self::Bool => "bool",
            Self::U8 => "uint8_t",
            Self::Rows => "spx_fused_rows",
            Self::Staged => "spx_fused_staged",
            Self::Text => "spx_fused_text",
            Self::Leased => "spx_fused_view",
        }
    }
    fn zero(self) -> &'static str {
        match self {
            Self::I64 => "INT64_C(0)",
            Self::Usize => "UINT64_C(0)",
            Self::Bool => "false",
            Self::U8 => "UINT8_C(0)",
            _ => "{0}",
        }
    }
    /// Private carriers live in the admitted heap frame.
    fn framed(self) -> bool {
        matches!(self, Self::Rows | Self::Staged | Self::Leased)
    }
    fn scalar(ty: &T) -> Option<Self> {
        match ty {
            T::I64 => Some(Self::I64),
            T::Usize => Some(Self::Usize),
            T::Bool => Some(Self::Bool),
            T::U8 => Some(Self::U8),
            _ => None,
        }
    }
}

struct Local {
    id: Option<ValueId>,
    name: String,
    kind: Kind,
}

/// A lowered value. Scalars are C expressions over already evaluated
/// operands; views name a declared local; a temporary leased view must be
/// ended by its single consumer.
enum Value {
    Scalar(String, Kind),
    Text(String),
    Leased { name: String, temporary: bool },
    Rows(String),
}

pub(super) struct Lowered {
    pub(super) function: String,
    pub(super) order: [Role; 3],
    /// Heap frame members after the domain header: (field, C type).
    pub(super) frame: Vec<(String, &'static str)>,
    pub(super) placement: [usize; 3],
}

pub(super) const FUNCTION: &str = "spx_fused_selected_v1";
/// A domain authority failure inside the selected function is a compiler or
/// runtime invariant breach, never a recoverable outcome.
const AUTHORITY: &str = "spx_runtime_invariant_failure(\"selected fused-row domain authority\");";
/// Lowering recursion bound; the closed census already bounds HIR depth.
const DEPTH: usize = 96;

struct Lower<'s, 'a> {
    sites: &'s Sites<'a>,
    cleanup: &'s PlanProof<'a>,
    order: [Role; 3],
    decls: String,
    body: String,
    locals: Vec<Local>,
    scopes: Vec<Vec<String>>,
    temps: usize,
    indent: usize,
    depth: usize,
    registry: Vec<(crate::cleanup_plan::StorageId, Physical)>,
    applied: Vec<crate::hir::ExpressionId>,
    failures_used: Vec<u32>,
    placed: Vec<u32>,
}

pub(super) fn lower(sites: &Sites<'_>, cleanup: &PlanProof<'_>) -> Result<Lowered, Diagnostic> {
    let order = roles(sites)?;
    let mut lower = Lower {
        sites,
        cleanup,
        order,
        decls: String::new(),
        body: String::new(),
        locals: Vec::new(),
        scopes: vec![Vec::new()],
        temps: 0,
        indent: 1,
        depth: 0,
        registry: Vec::new(),
        applied: Vec::new(),
        failures_used: Vec::new(),
        placed: Vec::new(),
    };
    lower.prefix()?;
    for statement in sites.suffix {
        lower.statement(statement)?;
    }
    let value = lower.expression(sites.tail)?;
    let Value::Scalar(result, Kind::I64) = value else {
        return Err(error("selected function tail is not an i64 result"));
    };
    let result = lower.materialize(result, Kind::I64);
    lower.close_scope()?;
    lower.place_region(&sites.function.body)?;
    lower.apply(&sites.function.body.id)?;
    lower.line("/* Canonical success exit; non-result cleanup precedes publication. */");
    let success = lower.finalizers(cleanup.success())?;
    for line in success.lines() {
        lower.line(line.trim_start());
    }
    let dead = lower.all_dead();
    for line in dead.lines() {
        lower.line(line.trim_start());
    }
    lower.line(&format!(
        "if (spx_domain->state != SPX_FUSED_SETTLED) {AUTHORITY}"
    ));
    lower.line(&format!("*spx_result = {result};"));
    lower.line("goto spx_return;");
    lower.failure_exits()?;
    lower.complete()?;
    let mut function = String::new();
    writeln!(
        function,
        "static __attribute__((unused)) spx_status_token {FUNCTION}(\n    struct spx_context *spx_ctx, spx_fused_frame *spx_frame, int64_t *spx_result\n) {{\n    spx_status_token spx_status = SPX_STATUS_SUCCESS;\n    spx_fused_domain *spx_domain = &spx_frame->domain;"
    )
    .expect("writing to a string cannot fail");
    function.push_str(&lower.flag_declarations());
    function.push_str(&lower.decls);
    // Ordinary call-depth guard; its entry boundary finalizes nothing.
    function.push_str(
        "    if (spx_ctx->call_depth >= SPX_MAX_CALL_DEPTH) {\n        spx_fused_abort(spx_domain);\n        return spx_rt_call_depth_failure(spx_ctx);\n    }\n    ++spx_ctx->call_depth;\n",
    );
    function.push_str(&lower.body);
    function.push_str(
        "spx_return:\n    if (spx_ctx->call_depth == UINT32_C(0)) spx_runtime_invariant_failure(\"call depth underflow\");\n    --spx_ctx->call_depth;\n    return spx_status;\n}\n",
    );
    let frame = lower
        .locals
        .iter()
        .filter(|local| local.kind.framed() && local.id.is_none())
        .map(|local| {
            (
                local.name.trim_start_matches("spx_frame->").to_owned(),
                local.kind.c(),
            )
        })
        .collect();
    let count = |kind| lower.names(kind).len();
    Ok(Lowered {
        function,
        order,
        frame,
        placement: [count(Kind::Staged), count(Kind::Rows), count(Kind::Leased)],
    })
}

/// Declaration order of the selected record's three field roles.
fn roles(sites: &Sites<'_>) -> Result<[Role; 3], Diagnostic> {
    let crate::hir::ResolvedTypeDeclarationKind::Record { fields } = &sites.record.kind else {
        return Err(error("selected record declaration is not a record"));
    };
    let mut order = [Role::Scalar; 3];
    let [a, b, c] = fields.as_slice() else {
        return Err(error("selected record does not have exactly three fields"));
    };
    for (slot, field) in order.iter_mut().zip([a, b, c]) {
        *slot = match field.ty {
            T::I64 => Role::Scalar,
            T::String => Role::Text,
            T::Bytes => Role::Payload,
            _ => {
                return Err(error(
                    "selected record field is outside the closed row shape",
                ))
            }
        };
    }
    for role in [Role::Scalar, Role::Text, Role::Payload] {
        if order.iter().filter(|r| **r == role).count() != 1 {
            return Err(error(
                "selected record roles are not one scalar, String and Bytes",
            ));
        }
    }
    Ok(order)
}

impl Lower<'_, '_> {
    fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.body.push_str("    ");
        }
        self.body.push_str(text);
        self.body.push('\n');
    }
    fn open(&mut self, text: &str) {
        self.line(text);
        self.indent += 1;
    }
    fn close(&mut self, text: &str) {
        self.indent -= 1;
        self.line(text);
    }
    /// Scalars, ordinary text views and flags are ordinary C locals; private
    /// carriers are fields of the admitted heap frame.
    fn declare_name(&mut self, kind: Kind) -> String {
        let field = format!("spx_l{}", self.temps);
        self.temps += 1;
        let name = if kind.framed() {
            format!("spx_frame->{field}")
        } else {
            writeln!(
                self.decls,
                "    __attribute__((unused)) {} {field} = {};",
                kind.c(),
                kind.zero()
            )
            .expect("writing to a string cannot fail");
            field
        };
        self.locals.push(Local {
            id: None,
            name: name.clone(),
            kind,
        });
        name
    }
    fn temp(&mut self, kind: Kind) -> String {
        self.declare_name(kind)
    }
    fn bind(&mut self, binding: &ResolvedBinding, kind: Kind) -> Result<String, Diagnostic> {
        if self.bound(&binding.id) {
            return Err(error("selected binding identity is declared twice"));
        }
        let name = self.declare_name(kind);
        self.locals.push(Local {
            id: Some(binding.id.clone()),
            name: name.clone(),
            kind,
        });
        Ok(name)
    }
    /// An immutable alias of an existing view shares its loan; it never
    /// begins a second loan and never ends the original.
    fn alias(
        &mut self,
        binding: &ResolvedBinding,
        name: String,
        kind: Kind,
    ) -> Result<(), Diagnostic> {
        if self.bound(&binding.id) {
            return Err(error("selected binding identity is declared twice"));
        }
        self.locals.push(Local {
            id: Some(binding.id.clone()),
            name,
            kind,
        });
        Ok(())
    }
    fn bound(&self, id: &ValueId) -> bool {
        self.locals
            .iter()
            .any(|local| local.id.as_ref() == Some(id))
    }
    fn lookup(&self, id: &ValueId) -> Result<(&str, Kind), Diagnostic> {
        self.locals
            .iter()
            .rev()
            .find(|local| local.id.as_ref() == Some(id))
            .map(|local| (local.name.as_str(), local.kind))
            .ok_or_else(|| error("selected place reads an unbound or foreign value"))
    }
    /// Distinct declared carriers of one kind, in declaration order.
    fn names(&self, kind: Kind) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for local in &self.locals {
            if local.kind == kind && !names.contains(&local.name) {
                names.push(local.name.clone());
            }
        }
        names
    }
    fn materialize(&mut self, code: String, kind: Kind) -> String {
        if code
            .strip_prefix("spx_l")
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
        {
            return code;
        }
        let name = self.temp(kind);
        self.line(&format!("{name} = {code};"));
        name
    }
    fn is_vector(&self, ty: &T) -> bool {
        *ty == self.sites.vector.ty
    }
    /// The exact producer-bound carrier fact for a view-valued expression.
    fn carrier(&self, e: &E) -> Result<Carrier, Diagnostic> {
        self.sites
            .carrier(e)
            .ok_or_else(|| error("view expression has no producer-bound carrier fact"))
    }
    /// Lexical loan expiry: end this scope's leased views in reverse order.
    fn close_scope(&mut self) -> Result<(), Diagnostic> {
        let views = self
            .scopes
            .pop()
            .ok_or_else(|| error("selected lexical scope underflow"))?;
        for name in views.iter().rev() {
            self.line(&format!(
                "if (!spx_fused_borrow_end(spx_domain, &{name})) {AUTHORITY}"
            ));
        }
        Ok(())
    }
    fn statement(&mut self, statement: &S) -> Result<(), Diagnostic> {
        match statement {
            S::Let { binding, value, .. } => self.let_statement(binding, value),
            S::Assign {
                binding,
                field: None,
                value,
                ..
            } => {
                let (name, kind) = self.lookup(&binding.id)?;
                let name = name.to_owned();
                if Kind::scalar(&binding.ty) != Some(kind) {
                    return Err(error("selected assignment target is not a scalar binding"));
                }
                let value = self.scalar(value, kind)?;
                self.line(&format!("{name} = {value};"));
                Ok(())
            }
            S::While {
                condition, body, ..
            }
            | S::LexicalWhile {
                condition, body, ..
            } => {
                self.open("for (;;) {");
                let condition = self.scalar(condition, Kind::Bool)?;
                self.line(&format!("if (!({condition})) break;"));
                let _ = self.expression(body)?;
                self.close("}");
                Ok(())
            }
            _ => Err(error(
                "statement is outside the closed selected suffix domain",
            )),
        }
    }
    fn let_statement(&mut self, binding: &ResolvedBinding, value: &E) -> Result<(), Diagnostic> {
        if binding.ty != value.ty || binding.ownership != value.ownership {
            return Err(error(
                "selected binding type or ownership drifted from its value",
            ));
        }
        if self.is_vector(&binding.ty) {
            return self.rows_transfer(binding, value);
        }
        if let Some(kind) = Kind::scalar(&binding.ty) {
            let code = self.scalar(value, kind)?;
            let name = self.bind(binding, kind)?;
            self.line(&format!("{name} = {code};"));
            return Ok(());
        }
        match self.expression(value)? {
            Value::Text(code) => {
                let name = self.bind(binding, Kind::Text)?;
                self.line(&format!("{name} = {code};"));
                Ok(())
            }
            Value::Leased {
                name,
                temporary: true,
            } => {
                // The binding adopts the loan; its lexical scope ends it.
                self.alias(binding, name.clone(), Kind::Leased)?;
                self.scopes
                    .last_mut()
                    .ok_or_else(|| error("selected lexical scope underflow"))?
                    .push(name);
                Ok(())
            }
            Value::Leased {
                name,
                temporary: false,
            } => self.alias(binding, name, Kind::Leased),
            _ => Err(error("selected binding value is outside the closed domain")),
        }
    }
    /// Immutable Vec transfer or consuming sort; both retire the prior header.
    /// Canonical transitions apply after the physical transfer they describe.
    fn rows_transfer(&mut self, binding: &ResolvedBinding, value: &E) -> Result<(), Diagnostic> {
        use crate::cleanup_plan::StorageId;
        if binding.ownership != O::Own || value.ownership != O::Own {
            return Err(error("selected row Vec transfer is not an owned move"));
        }
        let (argument, source) = match &value.kind {
            K::Place(place) if place.projections.is_empty() => (None, &place.root),
            K::Call {
                callee,
                instance: None,
                type_arguments,
                args,
            } if callee.as_str() == crate::vec_ops::SORT_OWNED_ID
                && type_arguments.as_slice() == std::slice::from_ref(&self.sites.item.ty) =>
            {
                let [argument] = args.as_slice() else {
                    return Err(error("selected sort has the wrong arity"));
                };
                match &argument.kind {
                    K::Place(place)
                        if place.projections.is_empty()
                            && argument.ownership == O::Own
                            && self.is_vector(&argument.ty) =>
                    {
                        (Some(argument), &place.root)
                    }
                    _ => return Err(error("selected sort does not consume a named row Vec")),
                }
            }
            _ => return Err(error("selected row Vec value is outside the closed domain")),
        };
        let (from, kind) = self.lookup(source)?;
        if kind != Kind::Rows {
            return Err(error("selected row Vec transfer reads a non-Vec binding"));
        }
        let from = from.to_owned();
        let to = self.bind(binding, Kind::Rows)?;
        self.register(
            StorageId::Value(binding.id.clone()),
            Physical::Rows(to.clone()),
        )?;
        self.register(
            StorageId::Temporary(value.id.clone()),
            Physical::Rows(to.clone()),
        )?;
        if let Some(argument) = argument {
            self.register(
                StorageId::CallArgument {
                    call: value.id.clone(),
                    parameter_index: 0,
                    value_expression: argument.id.clone(),
                },
                Physical::Rows(from.clone()),
            )?;
            self.apply(&argument.id)?;
            self.line(&format!(
                "if (!spx_fused_rows_sort(spx_domain, &{from}, &{to}, spx_fused_order, spx_string_length_v10)) {AUTHORITY}"
            ));
        } else {
            self.line(&format!(
                "if (!spx_fused_rows_move(spx_domain, &{from}, &{to})) {AUTHORITY}"
            ));
        }
        self.apply(&value.id)
    }
    fn scalar(&mut self, e: &E, kind: Kind) -> Result<String, Diagnostic> {
        match self.expression(e)? {
            Value::Scalar(code, actual) if actual == kind => Ok(code),
            _ => Err(error(
                "selected expression does not produce the required scalar",
            )),
        }
    }
    /// Each used plan failure exit: end loans, administrative abort, the exit's
    /// verbatim finalizers, pin release, then the selected status unchanged.
    fn failure_exits(&mut self) -> Result<(), Diagnostic> {
        let cleanup = self.cleanup;
        let exits: Vec<_> = cleanup
            .failures()
            .filter(|exit| self.failures_used.contains(&exit.id.0))
            .collect();
        for exit in exits {
            self.indent = 0;
            self.line(&format!("spx_exit_{}:", exit.id.0));
            self.indent = 1;
            for view in self.names(Kind::Leased).iter().rev() {
                self.line(&format!(
                    "if ({view}.nonce && !spx_fused_borrow_end(spx_domain, &{view})) {AUTHORITY}"
                ));
            }
            self.line("spx_fused_abort(spx_domain);");
            let code = self.finalizers(exit)?;
            for line in code.lines() {
                self.line(line.trim_start());
            }
            self.line(&format!(
                "if (!spx_fused_settle_abort(spx_domain)) {AUTHORITY}"
            ));
            let dead = self.all_dead();
            for line in dead.lines() {
                self.line(line.trim_start());
            }
            self.line("goto spx_return;");
        }
        Ok(())
    }
}
