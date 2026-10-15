use super::*;
use crate::test_allocation_observer::measure;

fn fixture() -> crate::hir::ResolvedProgram {
    let source = crate::check(
        r#"module app.loan;
@id("app.row") record Row { @id("app.row.n") n:i64, @id("app.row.text") text:string, }
@id("app.read") fn read(row:borrow Row)->i64 {row.n}
@id("app.main") fn main()->i64 {
 let rows=vec_push<Row>(vec_with_capacity<Row>(1usize),Row{n:7,text:"x"});
 let mut seen=0; for borrow row in rows {seen=seen+read(row);0} seen
}"#,
        "row-prefix-budget.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}
fn region(program: &crate::hir::ResolvedProgram) -> &ResolvedExpr {
    let mut pending: Vec<_> = program
        .functions
        .iter()
        .map(|function| &function.body)
        .collect();
    while let Some(expression) = pending.pop() {
        if super::super::row_region(&program.declarations, expression).is_some() {
            return expression;
        }
        pending.extend(crate::hir::lexical_exit::ChildCursor::new(expression));
    }
    panic!("original row traversal")
}

#[test]
fn actual_comparison_callbacks_match_old_prefix_and_fail_before_unpaid_activity() {
    let program = fixture();
    let region = region(&program);
    let old = super::super::row_region(&program.declarations, region).unwrap();
    // The exact prefix below has3 text comparisons,16 type comparisons,
    //4 place roots +10 distinct-ID pairs,1 binding,1 element and23 visits.
    let mut counts = [0usize; 6];
    let ((new, allocations), _, _, _) = crate::bounded_output::with_limit_usage_refusal(0, || {
        measure(|| {
            row_region(&program.declarations, region, &mut |comparison| {
                let index = match comparison {
                    Comparison::Text(_, _) => 0,
                    Comparison::Type(_, _) => 1,
                    Comparison::Value(_, _) => 2,
                    Comparison::Binding(_, _) => 3,
                    Comparison::Element(_) => 4,
                    Comparison::Visit => 5,
                };
                counts[index] += 1;
                Ok::<_, ()>(())
            })
            .unwrap()
            .unwrap()
        })
    });
    assert!(std::ptr::eq(old.source, new.source));
    assert!(std::ptr::eq(old.owner, new.owner));
    assert!(std::ptr::eq(old.element, new.element));
    assert!(std::ptr::eq(old.length, new.length));
    assert!(std::ptr::eq(old.row_index, new.row_index));
    assert_eq!(allocations.allocated, 0);
    assert!(!allocations.invalid);
    let expected = [3, 16, 14, 1, 1, 23];
    assert_eq!(counts, expected);
    let exact: usize = expected.iter().sum();
    for limit in [0, exact - 1, exact] {
        let mut remaining = limit;
        let mut failed = false;
        let mut consumed = 0;
        let (ok, retry, observed) = {
            let mut callback = |_: Comparison<'_>| {
                if failed || remaining == 0 {
                    failed = true;
                    return Err(());
                }
                remaining -= 1;
                consumed += 1;
                Ok(())
            };
            let (result, observed) =
                measure(|| row_region(&program.declarations, region, &mut callback));
            let retry = if limit < exact {
                row_region(&program.declarations, region, &mut callback).is_err()
            } else {
                false
            };
            (result.is_ok(), retry, observed)
        };
        assert_eq!(ok, limit == exact);
        if limit < exact {
            assert!(failed && retry);
            assert_eq!(consumed, limit);
        }
        assert_eq!(observed.allocated, 0);
        assert!(!observed.invalid);
    }
    // Changing the discovered marker preserves the original None verdict.
    let mut mutated = region.clone();
    let ResolvedExprKind::Block { statements, .. } = &mut mutated.kind else {
        unreachable!()
    };
    let ResolvedStatement::Let { binding, .. } = &mut statements[0] else {
        unreachable!()
    };
    binding.name.push('x');
    assert!(super::super::row_region(&program.declarations, &mutated).is_none());
    assert!(
        row_region(&program.declarations, &mutated, &mut |_| Ok::<_, ()>(()))
            .unwrap()
            .is_none()
    );
}
