//! Typed source constructor expansion; every push remains an ordinary call.
use super::Parser;
use crate::ast::{Expr, ExprKind, LetSyntax, Span, Statement, Type};
use crate::diagnostic::Diagnostic;

impl Parser {
    pub(super) fn vec_constructor(
        &mut self,
        mut types: Vec<Type>,
        items: Vec<Expr>,
        span: Span,
    ) -> Result<Expr, Diagnostic> {
        if types.len() != 1 {
            return Err(self.error_here(
                "SPX-P220",
                "vec_of requires exactly one explicit element type",
            ));
        }
        let count = items.len();
        if count as u64 > crate::vec_ops::MAX_CAPACITY {
            return Err(
                self.error_here("SPX-P220", "vec_of exceeds the existing Vec capacity bound")
            );
        }
        let element = types.pop().expect("one explicit type");
        // Charge all replacement backing before constructing any generated node.
        // Each step retains its own Type clone, call argument carrier, binding
        // name and expression nodes. Original items move, never clone.
        if !crate::bounded_output::reserve_active_required(
            129 * std::mem::size_of::<Option<(&Type, usize)>>(),
        ) {
            return Err(self.error_here(
                "SPX-P220",
                "vec_of type forecast exceeds the active source budget",
            ));
        }
        let type_bytes =
            crate::ast::vec_constructor::type_backing_bytes(&element).ok_or_else(|| {
                self.error_here("SPX-P220", "vec_of type expansion exceeds bounded depth")
            })?;
        let per_step = std::mem::size_of::<Statement>()
            + 2 * std::mem::size_of::<Expr>()
            + std::mem::size_of::<Type>()
            + 3 * (3 + usize::BITS as usize)
            + "vec_with_capacity".len()
            + type_bytes;
        let bytes = count
            .checked_add(1)
            .and_then(|steps| steps.checked_mul(per_step))
            .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<Expr>()));
        if !bytes.is_some_and(crate::bounded_output::reserve_active_required) {
            return Err(self.error_here(
                "SPX-P220",
                "vec_of expansion exceeds the active source budget",
            ));
        }
        let mut statements = Vec::with_capacity(count + 1);
        let mut previous = self.discard_name(); // Complete token inventory, not parser-local freshness.
        let call = Expr {
            kind: ExprKind::Call {
                name: "vec_with_capacity".to_owned(),
                type_arguments: vec![element.clone()],
                args: vec![Expr {
                    kind: ExprKind::Usize(count as u64),
                    span,
                }],
            },
            span,
        };
        statements.push(binding(previous.clone(), call, span));
        for item in items {
            let name = self.discard_name();
            let call = Expr {
                kind: ExprKind::Call {
                    name: "vec_push".to_owned(),
                    type_arguments: vec![element.clone()],
                    args: vec![
                        Expr {
                            kind: ExprKind::Var(previous),
                            span,
                        },
                        item,
                    ],
                },
                span,
            };
            statements.push(binding(name.clone(), call, span));
            previous = name;
        }
        let value = Expr {
            kind: ExprKind::Block {
                statements,
                tail: Box::new(Expr {
                    kind: ExprKind::Var(previous),
                    span,
                }),
            },
            span,
        };
        Ok(Expr {
            kind: ExprKind::VecConstructor {
                element,
                count: count as u32,
                value: Box::new(value),
            },
            span,
        })
    }
}
fn binding(name: String, value: Expr, span: Span) -> Statement {
    Statement::Let {
        syntax: LetSyntax::Authored,
        name,
        name_span: span,
        mutable: false,
        declared: None,
        value,
        span,
    }
}
