use super::{
    agent_contract_expr_json, quote_json, type_json, Diagnostic, ResolvedExpr, ResolvedExprKind,
    ResolvedStatement,
};
use crate::bounded_output::BudgetedJoin as _;
pub(super) fn json(expression: &ResolvedExpr) -> Result<String, Diagnostic> {
    let (statements, tail, kind) = match &expression.kind {
        ResolvedExprKind::Block { statements, tail } => (
            statements,
            tail,
            std::borrow::Cow::Borrowed("\"kind\":\"block\""),
        ),
        ResolvedExprKind::VecConstructor {
            element,
            count,
            statements,
            tail,
        } => {
            crate::hir::vec_constructor::authenticate(expression)?;
            (
                statements,
                tail,
                std::borrow::Cow::Owned(crate::bounded_output::budgeted_format(format_args!(
                    "\"kind\":\"vec_constructor\",\"element\":{},\"count\":{count}",
                    type_json(element)
                ))),
            )
        }
        _ => unreachable!("block presentation"),
    };
    Ok(format!(
        "{{{kind},\"statements\":[{}],\"tail\":{}}}",
        statements
            .iter()
            .map(|statement| {
                Ok(match statement {
                    ResolvedStatement::Let { binding, value, .. } => format!(
                        "{{\"kind\":\"let\",\"binding\":{},\"value\":{}}}",
                        quote_json(binding.id.as_str()),
                        agent_contract_expr_json(value)?
                    ),
                    ResolvedStatement::Assign {
                        binding,
                        field,
                        value,
                        ..
                    } => format!(
                        "{{\"kind\":\"assign\",\"target\":{},\"value\":{}{}}}",
                        quote_json(binding.id.as_str()),
                        agent_contract_expr_json(value)?,
                        field
                            .as_ref()
                            .map(|field| format!(",\"field\":{}", quote_json(field.as_str())))
                            .unwrap_or_default()
                    ),
                    ResolvedStatement::Unsafe { audit, body, .. } => format!(
                        "{{\"kind\":\"unsafe\",\"audit\":{},\"body\":{}}}",
                        quote_json(audit),
                        agent_contract_expr_json(body)?
                    ),
                    ResolvedStatement::LexicalWhile { .. } => {
                        return Err(Diagnostic::io(
                            "SPX-H006",
                            "lexical loops are not contract expressions",
                        ))
                    }
                    ResolvedStatement::While {
                        condition, body, ..
                    } => format!(
                        "{{\"kind\":\"while\",\"condition\":{},\"body\":{}}}",
                        agent_contract_expr_json(condition)?,
                        agent_contract_expr_json(body)?
                    ),
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?
            .budgeted_join(","),
        agent_contract_expr_json(tail)?
    ))
}
