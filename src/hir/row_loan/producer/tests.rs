use super::*;
use crate::bounded_output::{active_refused, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;

#[test]
fn descriptor_storage_and_current_identity_bytes_have_literal_exact_short_and_drop_bounds() {
    let span = Span::default();
    let execution = FunctionExecutionId::Monomorphic(DeclarationId::new("app.main"));
    let element = ResolvedType::Nominal {
        declaration: DeclarationId::new("app.row"),
        arguments: Vec::new(),
    };
    let binding = |name: &str, ty, ownership| ResolvedBinding {
        id: ValueId::new(name.to_owned()),
        name: name.to_owned(),
        ty,
        ownership,
        span,
    };
    let row = resolve_for::record::Row {
        source: binding(
            "rows",
            crate::vec_ops::resolved_vec(element.clone()),
            OwnershipMode::Own,
        ),
        index: binding("index", ResolvedType::Usize, OwnershipMode::Value),
        element,
        region: ExpressionId::new(&execution, "body"),
        bound: ValueId::new("bound".to_owned()),
    };
    let path = "body.arg.0";
    let identities = [
        "declaration:8:app.main:expression:10:body.arg.0",
        "declaration:8:app.main:expression:16:body.arg.0.arg.0",
        "declaration:8:app.main:expression:16:body.arg.0.arg.1",
    ];
    // Independent actual inventory: Box descriptor, two child carriers, one
    // Vec type argument, three Row ID clones, one Vec ID and 3 IDs. The
    // sentinel is a fixed compiler identity and owns no bytes.
    let storage = Layout::new::<RowLoan>().size()
        + 2 * std::mem::size_of::<ResolvedExpr>()
        + std::mem::size_of::<ResolvedType>()
        + 3 * "app.row".len()
        + "core.vec".len()
        + identities.iter().map(|id| id.len()).sum::<usize>();
    assert_eq!(required(&execution, &row, path), Some(storage));
    // The shared type-clone debit still prices the four declaration clones
    // (it is also the validator's); shared backings make them allocation-free.
    let shared_clones = 3 * "app.row".len() + "core.vec".len();
    let exact = storage + 3 * ExpressionId::OWNED_ALLOCATION_CARRIER_BYTES;
    // Independent owned-text oracle for the sentinel's value, built unobserved.
    let sentinel_oracle = DeclarationId::new(UNBOUND_CALLEE.to_owned());
    let (((), observed), refused, charged, receipt) = with_limit_usage_refusal(exact, || {
        measure(|| {
            let node = materialize(&execution, span, path, &row).unwrap();
            assert_eq!(node.id.as_str(), identities[0]);
            let ResolvedExprKind::RowLoan { loan, args } = &node.kind else {
                panic!("descriptor")
            };
            assert!(!loan.is_bound());
            assert!(loan.callee.is_compiler_static());
            assert_eq!(loan.callee.owned_allocation_bytes(), 0);
            assert_eq!(loan.callee, sentinel_oracle);
            assert_eq!(loan.parameter, row.bound);
            assert_eq!(args.len(), 2);
            assert_eq!(args[0].id.as_str(), identities[1]);
            assert_eq!(args[1].id.as_str(), identities[2]);
            drop(node);
        })
    });
    assert!(!refused);
    assert_eq!(charged, exact);
    assert_eq!(receipt, None);
    assert!(!observed.invalid);
    assert_eq!(observed.allocated, exact - shared_clones);
    assert_eq!(observed.live, 0);
    for limit in [storage - 1, exact - 1] {
        let (((), observed), refused, charged, receipt) = with_limit_usage_refusal(limit, || {
            measure(|| {
                assert!(materialize(&execution, span, path, &row).is_none());
                assert!(active_refused());
                assert!(materialize(&execution, span, path, &row).is_none());
            })
        });
        assert!(refused && receipt.is_some());
        assert!(charged <= limit);
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        if limit == storage - 1 {
            assert_eq!(observed.allocated, 0);
        }
    }
}

#[test]
fn bound_parameter_uses_old_canonical_identity_with_paid_carrier_and_short_refusal() {
    let callee = DeclarationId::new("app.reader");
    let old = ValueId::parameter(&FunctionExecutionId::Monomorphic(callee.clone()), 12);
    const TEXT: &str = "declaration:10:app.reader:value:param:2:12";
    assert_eq!(old.as_str(), TEXT);
    let exact = 2 * TEXT.len() + 10 + ExpressionId::SHARED_ALLOCATION_CARRIER_BYTES;
    let (((), observed), overflow, used, refusal) = with_limit_usage_refusal(exact, || {
        measure(|| {
            let (name, parameter) = bind(&callee, 12).unwrap();
            assert_eq!(name, callee);
            assert_eq!(parameter, old);
            drop((name, parameter));
        })
    });
    assert!(!overflow);
    assert_eq!(used, exact);
    assert_eq!(refusal, None);
    assert!(observed.allocated <= exact);
    assert_eq!(observed.live, 0);
    let (((), observed), overflow, used, refusal) = with_limit_usage_refusal(exact - 1, || {
        measure(|| {
            assert!(bind(&callee, 12).is_none());
            assert!(bind(&callee, 12).is_none());
        })
    });
    assert!(overflow);
    assert_eq!(used, 0);
    assert_eq!(refusal.unwrap().requested, exact);
    assert_eq!(observed.allocated, 0);
    assert_eq!(observed.live, 0);
}
