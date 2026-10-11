use super::{assignment, assignment_bounded, error};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    lexical_exit::ChildCursor, ExpressionId, ResolvedExpr, ResolvedExprKind, ResolvedProgram,
    ResolvedStatement,
};
include!("old_tests.rs");

fn resolved(source: &str) -> ResolvedProgram {
    let ast = crate::parse(source, "lexical-lookup.spx").unwrap();
    assert_eq!(
        crate::format::canonical(
            &crate::parse(&crate::format::canonical(&ast), "lexical-lookup.spx").unwrap()
        ),
        crate::format::canonical(&ast)
    );
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    program
}
fn assignments(expression: &ResolvedExpr) -> Vec<&ResolvedExpr> {
    let mut found = Vec::new();
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::Assign {
                    field: None, value, ..
                } = statement
                {
                    found.push(value);
                }
            }
        }
        // Independent indexed child enumeration, not the selected cursor.
        let mut index = 0;
        while let Some(child) = crate::hir::lexical_exit::child(expression, index) {
            pending.push(child);
            index += 1;
        }
    }
    found
}

#[test]
fn cursor_assignment_matches_saved_indexed_walk_across_nested_and_generated_blocks() {
    let program = resolved(
        r#"
module test.lookup;
@id("app.main") fn main()->i64 {
    let mut x=0;
    x=1;
    let generated=vec_of<i64>(1,2);
    let result=match x {
        0=>{let mut inner=0;inner=2;inner},
        _=>{let mut other=0;other=3;other},
    };
    x=result;
    x
}
"#,
    );
    let body = &program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap()
        .body;
    let found = assignments(body);
    assert_eq!(found.len(), 4);
    for value in found {
        let expected = old_assignment(body, &value.id).unwrap();
        let current = assignment(body, &value.id).unwrap();
        assert!(std::ptr::eq(expected.0, current.0));
        assert!(std::ptr::eq(expected.1, current.1));
    }
    let absent = ExpressionId::from_owned("foreign-assignment".into());
    assert_eq!(
        assignment(body, &absent).unwrap_err().message,
        old_assignment(body, &absent).unwrap_err().message
    );
}

#[test]
fn cursor_assignment_keeps_exact_identity_duplicate_and_field_refusals() {
    let mut program = resolved(
        r#"
module test.lookup_hostile;
@id("cell") record Cell {@id("cell.n") n:i64,}
@id("app.main") fn main()->i64 {
    let mut x=0;
    x=1;
    x=2;
    let mut cell=Cell {n:0};
    cell.n=3;
    x
}
"#,
    );
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("block")
    };
    let mut ids = Vec::new();
    let mut field = None;
    for statement in statements.iter() {
        if let ResolvedStatement::Assign {
            field: projection,
            value,
            ..
        } = statement
        {
            if projection.is_none() {
                ids.push(value.id.clone());
            } else {
                field = Some(value.id.clone());
            }
        }
    }
    assert_eq!(ids.len(), 2);
    let field = field.unwrap();
    assert!(assignment(&function.body, &field)
        .unwrap_err()
        .message
        .contains("absent"));
    assert!(old_assignment(&function.body, &field)
        .unwrap_err()
        .message
        .contains("absent"));
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        unreachable!()
    };
    for statement in statements {
        if let ResolvedStatement::Assign {
            field: None, value, ..
        } = statement
        {
            if value.id == ids[1] {
                value.id = ids[0].clone();
            }
        }
    }
    assert!(assignment(&function.body, &ids[0])
        .unwrap_err()
        .message
        .contains("duplicated"));
    assert!(old_assignment(&function.body, &ids[0])
        .unwrap_err()
        .message
        .contains("duplicated"));
}

#[test]
fn cursor_wide_lookup_has_linear_work_and_exact_one_short_frame_backing() {
    let mut source =
        String::from("module test.wide_lookup; @id(\"app.main\") fn main()->i64 {let mut x=0;");
    for index in 0..1024 {
        source.push_str(&format!("let padding_{index}={index};"));
    }
    source.push_str("x=42;x}");
    let program = resolved(&source);
    let body = &program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap()
        .body;
    let values = assignments(body);
    assert_eq!(values.len(), 1);
    let id = &values[0].id;
    let mut work = crate::cache_codec::MAX_NODES;
    let value = assignment_bounded(body, id, &mut work).unwrap();
    assert!(std::ptr::eq(value.1, values[0]));
    let charged = crate::cache_codec::MAX_NODES - work;
    // Independent authored census: 1026 one-child statements, 1027
    // expression children, one root and its final next(None) probe.
    let expected = crate::cache_codec::MAX_DEPTH + 2 * 1027 + 1 + 1026;
    assert_eq!(charged, expected);
    assert!(old_assignment(body, id)
        .unwrap_err()
        .message
        .contains("work exhausted"));
    for limit in [expected, expected - 1] {
        let mut remaining = limit;
        let outcome = assignment_bounded(body, id, &mut remaining);
        assert_eq!(outcome.is_ok(), limit == expected);
        if limit == expected {
            assert_eq!(remaining, 0);
        }
    }
    let bytes = std::mem::size_of::<[Option<ChildCursor<'_>>; crate::cache_codec::MAX_DEPTH]>();
    for limit in [bytes, bytes - 1] {
        let (outcome, overflow, charged) =
            crate::bounded_output::with_limit_usage(limit, || assignment(body, id));
        assert_eq!(outcome.is_ok(), limit == bytes);
        assert_eq!(overflow, limit != bytes);
        assert_eq!(charged, if limit == bytes { bytes } else { 0 });
    }
}
