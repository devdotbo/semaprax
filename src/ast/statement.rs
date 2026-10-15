//! Source statement carriers and their evaluated children.
use super::{Expr, FieldTarget, LetSyntax, Span, Type};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Statement {
    Let {
        /// Authored syntax provenance; it grants no semantic authority.
        syntax: LetSyntax,
        name: String,
        name_span: Span,
        /// Explicit Mutation v1: `true` when the source declared `let mut`.
        mutable: bool,
        /// Class Inheritance v1: optional declared type annotation
        /// `let name: T = value;`. `None` when the source omitted `: T`.
        declared: Option<Type>,
        value: Expr,
        span: Span,
    },
    /// Explicit Mutation v1: `<binding> = <expr>;` over a simple local
    /// binding. Statement-only; there is no assignment expression.
    /// Field Mutation v1 extends the target with one direct scalar field:
    /// `<binding>.<field> = <expr>;`.
    Assign {
        name: String,
        name_span: Span,
        field: Option<FieldTarget>,
        value: Expr,
        span: Span,
    },
    /// Unsafe Boundary Mechanics v1: `@audit("...") unsafe { ... }`. The body
    /// is an ordinary safe block expression; no raw pointers or memory
    /// operations exist inside. The audit summary is recorded verbatim.
    Unsafe {
        audit: String,
        audit_span: Span,
        body: Box<Expr>,
        span: Span,
    },
    /// Bounded While-Loops v1: `while <condition> { <body> }`. The condition
    /// must be exactly `bool` and the body is an ordinary block whose value is
    /// discarded; the statement itself produces no value.
    While {
        condition: Box<Expr>,
        body: Box<Expr>,
        span: Span,
    },
    /// Bounded Vec traversal: `for item in values { body }`.
    For {
        item: String,
        item_span: Span,
        values: Box<Expr>,
        body: Box<Expr>,
        span: Span,
    },
    /// Authenticated, iteration-local record view: `for borrow row at index in rows`.
    /// `index` is optional. Neither the row nor its owned fields materialize owners.
    ForBorrow {
        item: String,
        item_span: Span,
        index: Option<TraversalIndex>,
        values: Box<Expr>,
        body: Box<Expr>,
        span: Span,
    },
    /// Consuming iterator traversal: `for own item in iterator { body }`.
    ///
    /// This stays distinct from [`Statement::For`] so the older Vec traversal
    /// retains its exact syntax and cache codec representation.
    ForOwn {
        item: String,
        item_span: Span,
        values: Box<Expr>,
        body: Box<Expr>,
        span: Span,
    },
}

/// Immutable source spelling for the current traversal index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraversalIndex {
    pub name: String,
    pub span: Span,
}

impl Statement {
    /// The statement's evaluated expression: the initializer of a `let`, the
    /// assigned value of an assignment, or the ordinary block body of an
    /// unsafe boundary statement. While statements carry two evaluated
    /// expressions and must be traversed with [`Statement::child`] instead.
    pub fn value(&self) -> &Expr {
        match self {
            Self::Let { value, .. } | Self::Assign { value, .. } => value,
            Self::Unsafe { body, .. } => body,
            Self::While { .. }
            | Self::For { .. }
            | Self::ForBorrow { .. }
            | Self::ForOwn { .. } => {
                panic!("loop statements expose source/condition and body children")
            }
        }
    }

    /// Mutable access to the statement's evaluated expression.
    pub fn value_mut(&mut self) -> &mut Expr {
        match self {
            Self::Let { value, .. } | Self::Assign { value, .. } => value,
            Self::Unsafe { body, .. } => body,
            Self::While { .. }
            | Self::For { .. }
            | Self::ForBorrow { .. }
            | Self::ForOwn { .. } => {
                panic!("loop statements expose source/condition and body children")
            }
        }
    }

    /// The statement's source-level binding name. Only `let` and assignment
    /// statements bind or target a name.
    pub fn name(&self) -> &str {
        match self {
            Self::Let { name, .. } | Self::Assign { name, .. } => name,
            Self::Unsafe { .. }
            | Self::While { .. }
            | Self::For { .. }
            | Self::ForBorrow { .. }
            | Self::ForOwn { .. } => {
                panic!("only let and assignment statements declare a binding")
            }
        }
    }

    /// The verbatim audit summary of an unsafe boundary statement.
    pub fn audit(&self) -> Option<&str> {
        match self {
            Self::Unsafe { audit, .. } => Some(audit),
            _ => None,
        }
    }

    /// `true` for assignment statements.
    pub fn is_assign(&self) -> bool {
        matches!(self, Self::Assign { .. })
    }

    /// Number of directly nested evaluated expressions. `let`, assignment,
    /// and unsafe statements contribute one; while statements contribute its
    /// condition then its body, in evaluation order.
    pub fn child_count(&self) -> usize {
        match self {
            Self::Let { .. } | Self::Assign { .. } | Self::Unsafe { .. } => 1,
            Self::While { .. }
            | Self::For { .. }
            | Self::ForBorrow { .. }
            | Self::ForOwn { .. } => 2,
        }
    }

    /// One directly nested evaluated expression in left-to-right order.
    pub fn child(&self, index: usize) -> Option<&Expr> {
        match self {
            Self::Let { value, .. } | Self::Assign { value, .. } => (index == 0).then_some(value),
            Self::Unsafe { body, .. } => (index == 0).then_some(body.as_ref()),
            Self::While {
                condition, body, ..
            } => [condition.as_ref(), body.as_ref()].get(index).copied(),
            Self::For { values, body, .. }
            | Self::ForBorrow { values, body, .. }
            | Self::ForOwn { values, body, .. } => {
                [values.as_ref(), body.as_ref()].get(index).copied()
            }
        }
    }
}
