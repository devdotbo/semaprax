//! An explicit current v84 composition frontier; tags and detached parts lend
//! no authority to skip frozen nested-loan restrictions.
use super::{Diagnostic, ResolvedProgram};
use crate::hir::{ResolvedExprKind as K, ResolvedStatement as S};

pub(in crate::graph) fn authenticate(program: &ResolvedProgram) -> Result<bool, Diagnostic> {
    if !crate::hir::copy_discard_program_uses(program) {
        return Ok(false);
    }
    let mut found = false;
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
    {
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            // Each discard must be inside the exact current finite-record body.
            // Finite induction sizes demand; the full validators below retain
            // ownership, loan, lexical target and canonical cleanup authority.
            let mut frames = [None; 514];
            frames[0] = Some((crate::hir::lexical_exit::ChildCursor::new(root), false));
            let mut length = 1;
            let mut work = 0usize;
            while length != 0 {
                let (cursor, covered) = frames[length - 1].as_mut().unwrap();
                if !cursor.is_started() {
                    if let K::Block { statements, .. } | K::VecConstructor { statements, .. } =
                        &cursor.expression().kind
                    {
                        for statement in statements {
                            if matches!(statement, S::CopyDiscard { .. }) {
                                if !*covered {
                                    return Ok(false);
                                }
                                found = true;
                            }
                        }
                    }
                }
                if let Some(child) = cursor.next() {
                    work += 1;
                    if work > 65_536 || length == 514 {
                        return Ok(false);
                    }
                    let mut child_covered = *covered;
                    if let Some((statement, 1)) = cursor.last_statement_child() {
                        if let S::While {
                            condition, body, ..
                        }
                        | S::LexicalWhile {
                            condition, body, ..
                        } = statement
                        {
                            child_covered |=
                                crate::finite_owned_record_loop::resolved(program, condition, body)
                                    .is_some();
                        }
                    }
                    frames[length] = Some((
                        crate::hir::lexical_exit::ChildCursor::new(child),
                        child_covered,
                    ));
                    length += 1;
                } else {
                    frames[length - 1] = None;
                    length -= 1;
                }
            }
        }
    }
    if !found {
        return Ok(false);
    }
    crate::hir::validate(program)?;
    crate::loan_plan::validate_program(program)?;
    crate::hir::analyze_byte_data_capacity(program)?;
    crate::cleanup_plan::validate_program(program)?;
    Ok(true)
}
