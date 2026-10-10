//! Exact identity lookup with bounded stack and charged prefix inspection.
use super::super::*;

pub(in crate::cleanup_plan::build) fn expression_at<'e>(
    root: &'e ResolvedExpr,
    id: &ExpressionId,
) -> Result<&'e ResolvedExpr, Diagnostic> {
    lookup(root, id, 1_000_000)
}

fn debit(work: &mut usize, limit: usize) -> Result<(), Diagnostic> {
    *work = work
        .checked_add(1)
        .filter(|next| *next <= limit)
        .ok_or_else(|| plan_error("lexical advancement lookup exceeds bounded work"))?;
    Ok(())
}

fn child<'e>(
    expression: &'e ResolvedExpr,
    index: usize,
    work: &mut usize,
    limit: usize,
) -> Result<Option<&'e ResolvedExpr>, Diagnostic> {
    Ok(match &expression.kind {
        ResolvedExprKind::Block { statements, tail } => {
            let mut offset = 0usize;
            for statement in statements {
                debit(work, limit)?;
                let count = statement.child_count();
                let end = offset
                    .checked_add(count)
                    .ok_or_else(|| plan_error("lexical advancement child index overflows"))?;
                if index < end {
                    return Ok(statement.child(index - offset));
                }
                offset = end;
            }
            (index == offset).then_some(tail.as_ref())
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            if index == 0 {
                return Ok(Some(scrutinee));
            }
            let mut offset = 1usize;
            for arm in arms {
                debit(work, limit)?;
                if let Some(guard) = &arm.guard {
                    if index == offset {
                        return Ok(Some(guard));
                    }
                    offset += 1;
                }
                if index == offset {
                    return Ok(Some(&arm.value));
                }
                offset += 1;
            }
            None
        }
        _ => crate::hir::lexical_exit::child(expression, index),
    })
}

fn lookup<'e>(
    root: &'e ResolvedExpr,
    id: &ExpressionId,
    limit: usize,
) -> Result<&'e ResolvedExpr, Diagnostic> {
    const DEPTH: usize = crate::cache_codec::MAX_DEPTH + 1;
    let mut stack: [Option<(&ResolvedExpr, usize)>; DEPTH] = [None; DEPTH];
    stack[0] = Some((root, 0));
    let mut used = 1;
    let mut work = 0;
    let mut found = None;
    while used != 0 {
        let (expression, index) = stack[used - 1].expect("occupied lexical lookup frame");
        debit(&mut work, limit)?;
        if index == 0 && &expression.id == id {
            if found.is_some() {
                return Err(plan_error(
                    "lexical advancement references a duplicate expression identity",
                ));
            }
            found = Some(expression);
        }
        if let Some(child) = child(expression, index, &mut work, limit)? {
            stack[used - 1] = Some((expression, index + 1));
            if used == DEPTH {
                return Err(plan_error(
                    "lexical advancement lookup exceeds bounded depth",
                ));
            }
            stack[used] = Some((child, 0));
            used += 1;
        } else {
            used -= 1;
            stack[used] = None;
        }
    }
    found.ok_or_else(|| plan_error("lexical advancement references an absent expression identity"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> ResolvedExpr {
        let program = crate::hir::resolve(
            &crate::parse(
                "module lookup; @id(\"main\") fn main() -> i64 { let a = 1; let b = 2; 3 }",
                std::path::Path::new("lookup.spx"),
            )
            .unwrap(),
        )
        .unwrap();
        program.functions[0].body.clone()
    }

    #[test]
    fn complete_identity_scan_accepts_exact_work_and_refuses_one_short() {
        let body = body();
        // Three Block children: prefixes 1, 2, 2, 2 plus four cursor
        // visits at the Block and one visit at each scalar = fourteen.
        assert!(lookup(&body, &body.id, 14).is_ok());
        assert!(lookup(&body, &body.id, 13).is_err());
        assert!(lookup(&body, &body.id, 0).is_err());
    }

    #[test]
    fn identity_found_before_later_duplicate_is_not_authority() {
        let mut body = body();
        let id = body.id.clone();
        let ResolvedExprKind::Block { tail, .. } = &mut body.kind else {
            panic!("Block");
        };
        tail.id = id.clone();
        let error = expression_at(&body, &id).unwrap_err();
        assert!(error.message.contains("duplicate expression identity"));
    }
}
