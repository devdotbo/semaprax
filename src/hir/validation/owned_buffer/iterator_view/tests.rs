use super::*;
const SOURCE: &str = r#"module iterator.inline_read;
@id("app.main") fn main()->i64 ensures result==2 {
 let values=vec_push<Bytes>(vec_with_capacity<Bytes>(1usize),bytes_zeroed(1usize));
 let mut count=0;
 for own item in vec_into_iter<Bytes>(values) {
  let scratch=bytes_zeroed(2usize);
  if byte_len(bytes_as_slice(item))==1usize {count=count+1;};
  if byte_len(bytes_as_slice(scratch))==2usize {count=count+1;};
  0
 }
 return count;
}
"#;
fn resolved() -> crate::hir::ResolvedProgram {
    let source = crate::check(SOURCE, "iterator-inline-read.spx").unwrap();
    let canonical = crate::format::canonical(&source);
    let roundtrip = crate::check(&canonical, "iterator-inline-read.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&roundtrip));
    crate::hir::resolve(&source).unwrap()
}
#[test]
fn exact_iterator_item_and_finite_scratch_inline_reads_revalidate() {
    let program = resolved();
    crate::hir::validate(&program).unwrap();
    let summary = crate::hir::analyze_byte_data_capacity(&program).unwrap();
    let main = summary.function("app.main").unwrap();
    assert_eq!(
        (main.bytes_copy_sites, main.owned_byte_payload_bytes),
        (2, 3)
    );
}
#[test]
fn inline_view_requires_current_exact_owner_operation_and_borrow_type() {
    let program = resolved();
    let main = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let (condition, body) = crate::finite_byte_scratch::loops(main).next().unwrap();
    let protocol = crate::hir::iterator_loop::recognize(condition, body).unwrap();
    let proof = crate::finite_byte_scratch::resolved(&program, condition, body).unwrap();
    let item = protocol.owned_item.unwrap();
    let mut pending = vec![protocol.authored_body];
    let mut views = Vec::new();
    while let Some(expression) = pending.pop() {
        if matches!(&expression.kind,ResolvedExprKind::BorrowPlace {operation,..}
            if operation.as_str()==crate::byte_ops::BYTES_AS_SLICE_ID)
        {
            views.push(expression);
        }
        crate::hir::byte_capacity::push_resolved_expression_children_in_authored_order(
            expression,
            &mut pending,
        );
    }
    assert_eq!(views.len(), 2);
    for view in views {
        assert!(iterator_byte_view_is_admitted(
            view,
            Some(item),
            Some(&proof)
        ));
        assert!(!iterator_byte_view_is_admitted(view, None, None));
        let mut forged = view.clone();
        forged.ty = ResolvedType::Str;
        assert!(!iterator_byte_view_is_admitted(
            &forged,
            Some(item),
            Some(&proof)
        ));
        forged = view.clone();
        forged.ownership = OwnershipMode::Own;
        assert!(!iterator_byte_view_is_admitted(
            &forged,
            Some(item),
            Some(&proof)
        ));
        forged = view.clone();
        let ResolvedExprKind::BorrowPlace { operation, .. } = &mut forged.kind else {
            unreachable!()
        };
        *operation = DeclarationId::new(crate::byte_ops::STR_AS_BYTES_ID);
        assert!(!iterator_byte_view_is_admitted(
            &forged,
            Some(item),
            Some(&proof)
        ));
        forged = view.clone();
        let ResolvedExprKind::BorrowPlace { place, .. } = &mut forged.kind else {
            unreachable!()
        };
        place.root = main.result_id.clone();
        assert!(!iterator_byte_view_is_admitted(
            &forged,
            Some(item),
            Some(&proof)
        ));
        forged = view.clone();
        let ResolvedExprKind::BorrowPlace { place, .. } = &mut forged.kind else {
            unreachable!()
        };
        place
            .projections
            .push(crate::hir::PlaceProjection::Field(DeclarationId::new(
                "hostile.field",
            )));
        assert!(!iterator_byte_view_is_admitted(
            &forged,
            Some(item),
            Some(&proof)
        ));
    }
}
