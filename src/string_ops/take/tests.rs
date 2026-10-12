use super::*;
use crate::cleanup_plan::{CleanupTransition, StatusLane, StorageId};

const SOURCE: &str = r#"module take.owner;
@id("take") fn take(text:string)->string {string_take(text)}
@id("copy") fn copy(text:string)->string {text}
@id("main") fn main()->i64 {string_len(take("é\u{0}"))}
"#;

#[test]
fn exact_named_transfer_has_one_commit_no_clone_or_failure_and_keeps_default_reads() {
    assert!(!crate::string_ops::ALL.contains(&crate::string_ops::StringOp::Take));
    assert!(crate::string_ops::StringOp::Take.is_wasm_refused());
    let ast = crate::check(SOURCE, "string-take.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let again = crate::check(&canonical, "string-take.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&again));
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    assert_eq!(
        crate::wasm::emit_resolved_module_with_scalar_exports(&program, &["main".to_owned()])
            .unwrap_err()
            .code,
        "SPX-W115"
    );
    crate::cleanup_plan::validate_program(&program).unwrap();
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&ast, &graph).unwrap();
    assert!(graph.contains(ID));
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "take")
        .unwrap();
    let mut root = &function.body;
    while let ResolvedExprKind::Block { tail, .. } = &root.kind {
        root = tail;
    }
    let argument = operand(root).expect("actual named intrinsic");
    let ResolvedExprKind::Place(place) = &argument.kind else {
        unreachable!()
    };
    assert_eq!(
        crate::literal_format::moving_string_operands(function).len(),
        1
    );
    assert!(!function
        .cleanup_plan
        .status_sources
        .iter()
        .any(|s| s.id.expression == root.id && s.id.lane == StatusLane::OwnerAdmission));
    assert!(!function
        .cleanup_plan
        .status_sources
        .iter()
        .any(|s| s.id.expression == root.id && s.id.lane == StatusLane::OperationFailure));
    assert_eq!(function.cleanup_plan.blocks.iter().flat_map(|b|&b.transitions).filter(|t|
        matches!(t,CleanupTransition::CallCommit {call,arguments} if call==&root.id && arguments.len()==1)).count(),1);
    assert_eq!(function.cleanup_plan.blocks.iter().flat_map(|b|&b.transitions).filter(|t|
        matches!(t,CleanupTransition::Transfer {at,source,destination} if at==&argument.id
            && source.storage==StorageId::Value(place.root.clone()) && source.projections.is_empty()
            && matches!(&destination.storage,StorageId::CallArgument {call,parameter_index:0,value_expression} if call==&root.id && value_expression==&argument.id))).count(),1);
    assert!(!function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|b| &b.transitions)
        .any(|t| matches!(t,CleanupTransition::Initialize {at,..} if at==&argument.id)));
    let copy = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "copy")
        .unwrap();
    assert!(crate::literal_format::moving_string_operands(copy).is_empty());
    assert!(copy.cleanup_plan.blocks.iter().flat_map(|b|&b.transitions).any(|t|
        matches!(t,CleanupTransition::Initialize {destination,..} if matches!(destination.storage,StorageId::Temporary(_)))));
    for mutation in 0..5 {
        let mut forged = program.clone();
        let f = forged
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "take")
            .unwrap();
        let ResolvedExprKind::Block {
            tail: expression, ..
        } = &mut f.body.kind
        else {
            panic!("checked function block")
        };
        let ResolvedExprKind::Call {
            args,
            type_arguments,
            ..
        } = &mut expression.kind
        else {
            panic!("take call")
        };
        match mutation {
            0 => args[0].ownership = OwnershipMode::Borrow,
            1 => args[0].ty = ResolvedType::I64,
            2 => {
                let ResolvedExprKind::Place(p) = &mut args[0].kind else {
                    panic!("place")
                };
                p.projections.push(crate::hir::PlaceProjection::Field(
                    crate::hir::DeclarationId::new("forged"),
                ));
            }
            3 => type_arguments.push(ResolvedType::String),
            _ => {
                args[0].kind = ResolvedExprKind::String("forged".into());
            }
        }
        assert_eq!(crate::hir::validate(&forged).unwrap_err().code, "SPX-H006");
    }
}

#[test]
fn named_only_source_refuses_contracts_computed_borrowed_and_live_loan_transfers() {
    for (source, code) in [
        ("module bad; @id(\"main\") fn main()->i64 {string_len(string_take(\"literal\"))}", "SPX-O105"),
        ("module bad; @id(\"main\") fn main()->i64 {let text=\"x\";let moved=string_take(text);string_len(text)}", "SPX-O101"),
        ("module bad; @id(\"main\") fn main()->i64 {let text=\"x\";let view=string_as_str(text);let moved=string_take(text);str_len_bytes(view)}", "SPX-T265"),
        ("module bad; @id(\"helper\") fn helper(text:string)->i64 requires string_len(string_take(text))>0 {0} @id(\"main\") fn main()->i64 {0}", "SPX-O105"),
        ("module bad; @id(\"helper\") fn helper(text:borrow string)->string {string_take(text)} @id(\"main\") fn main()->i64 {0}", "SPX-O102"),
    ] {
        let errors=crate::check(source,"string-take-negative.spx").unwrap_err();
        assert!(errors.iter().any(|error|error.code==code),"{source}: {errors:?}");
    }
}

#[test]
fn added_transfer_inventory_pays_its_node_exact_short_sticky_and_drop() {
    use crate::test_allocation_observer::measure;
    use std::mem::{align_of, size_of};
    let ast = crate::check(SOURCE, "string-take-inventory.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "take")
        .unwrap();
    // Saved pre-transfer discovery: same traversal and old concat/format set,
    // but no new Take insertion. This is an allocation baseline, not a payer.
    let (_, old) = measure(|| {
        let mut set = crate::string_ops::same_owner_concat_operands(function);
        crate::hir::function_value::walk(function, |expression| {
            if let ResolvedExprKind::LiteralFormat { args, .. } = &expression.kind {
                set.extend(
                    args.iter()
                        .filter(|a| {
                            a.ty == ResolvedType::String
                                && a.ownership == OwnershipMode::Own
                                && matches!(a.kind, ResolvedExprKind::Place(_))
                        })
                        .map(|a| a.id.clone()),
                );
            }
        });
        assert!(set.is_empty());
        drop(set);
    });
    // Independent literal B=6 internal-node envelope: header, count, height,
    // eleven shared-ID keys, and twelve links. ID clone payload is shared.
    let alignment = align_of::<crate::hir::ExpressionId>().max(align_of::<usize>());
    let round = |n: usize| n.div_ceil(alignment) * alignment;
    let required: usize = [
        size_of::<usize>(),
        2,
        2,
        11 * size_of::<crate::hir::ExpressionId>(),
        0,
        12 * size_of::<usize>(),
    ]
    .into_iter()
    .map(round)
    .sum();
    let (((), observed), overflowed, used) =
        crate::bounded_output::with_limit_usage(required, || {
            // Fixed ledger context exists before observation; every inventory and
            // traversal allocation plus its Drop remains inside the observer.
            measure(|| {
                let set = crate::literal_format::checked_moving_string_operands(function).unwrap();
                assert_eq!(set.len(), 1);
                let mut root = &function.body;
                while let ResolvedExprKind::Block { tail, .. } = &root.kind {
                    root = tail;
                }
                assert!(set.contains(&operand(root).unwrap().id));
                drop(set);
            })
        });
    assert!(!overflowed);
    assert_eq!(used, required);
    assert!(!old.invalid && !observed.invalid);
    assert_eq!(old.live, 0);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    assert_eq!(observed.allocations, old.allocations + 1);
    assert!(observed.allocated > old.allocated);
    assert!(observed.allocated - old.allocated <= required);
    let (_, overflowed, used, first) =
        crate::bounded_output::with_limit_usage_refusal(required - 1, || {
            let error =
                crate::literal_format::checked_moving_string_operands(function).unwrap_err();
            assert_eq!(error.code, "SPX-H006");
            assert!(error.message.is_empty());
            assert!(!crate::bounded_output::reserve_active_required(0));
            assert!(crate::literal_format::checked_moving_string_operands(function).is_err());
        });
    assert!(overflowed);
    assert_eq!(used, 0);
    let first = first.unwrap();
    assert_eq!(first.requested, required);
    assert_eq!(first.remaining, required - 1);
}
