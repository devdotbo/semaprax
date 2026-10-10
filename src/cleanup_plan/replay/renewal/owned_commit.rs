//! An owned RHS debit is a call commit, never an opportunistic history filter.
use super::*;
use crate::cleanup_plan::renewal_history::RenewalHistory;

#[test]
fn owned_rhs_history_debits_only_committed_epochs_and_protects_the_cell() {
    let flag = LivenessFlagId;
    let mut history = RenewalHistory::new(vec![flag(1), flag(2), flag(3)], Some(flag(1)));
    history.stage(|source| match source.0 {
        1 => Some(flag(11)),
        2 => Some(flag(12)),
        _ => None,
    });
    assert_eq!(
        history.history,
        vec![flag(1), flag(2), flag(3)],
        "staging is not consumption"
    );
    // A later argument failure has not committed either staged argument.
    let failure = history.clone();
    assert_eq!(failure.history, vec![flag(1), flag(2), flag(3)]);
    history.commit(&BTreeSet::from([flag(11), flag(12)]));
    assert_eq!(history.history, vec![flag(1), flag(3)]);
    // Neither a stale epoch nor an unrelated live owner is a debit authority.
    history.commit(&BTreeSet::from([flag(2), flag(99)]));
    assert_eq!(history.history, vec![flag(1), flag(3)]);
}

const SOURCE: &str = r#"module test.owned_rhs_commit;
@id("app.row") record Row { @id("app.row.number") number:i64, @id("app.row.text") text:string, }
@id("app.main") fn main()->i64 {
 let mut words=vec_with_capacity<Row>(2usize);
 let later=vec_with_capacity<Row>(1usize);
 let mut index=0;
 while index<2 {
  let item=Row{number:7,text:"ab"};
  words=vec_push<Row>(words,item);
  index=index+1; 0
 }
 if vec_len<Row>(words)==2usize && vec_len<Row>(later)==0usize {7}else{0}
}
"#;
#[test]
fn owned_element_renewal_replays_exact_staging_commit_and_unrelated_history() {
    let checked = crate::check(SOURCE, "owned-rhs-commit.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let roundtrip = crate::check(&canonical, "owned-rhs-roundtrip.spx").unwrap();
    assert_eq!(crate::format::canonical(&roundtrip), canonical);
    let graph = crate::graph::to_json(&checked).unwrap();
    crate::graph::verify_json(&checked, &graph).unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let function = &program.functions[0];
    validate_structure(&program, function).unwrap();
    let at = crate::hir::vec_loop_renewal::bindings_in(&program, function)
        .keys()
        .next()
        .unwrap()
        .clone();
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("main block")
    };
    let later = statements
        .iter()
        .find_map(|statement| match statement {
            ResolvedStatement::Let { binding, .. } if binding.name == "later" => {
                Some(CleanupPlace::whole(StorageId::Value(binding.id.clone())))
            }
            _ => None,
        })
        .unwrap();
    for mode in 0..4 {
        let mut forged = function.clone();
        let mut changed = false;
        for block in &mut forged.cleanup_plan.blocks {
            if mode == 0 {
                block.transitions.retain(|transition| {
                    let remove = matches!(transition, CleanupTransition::CallCommit { call, .. } if call == &at);
                    changed |= remove;
                    !remove
                });
            }
            for transition in &mut block.transitions {
                if let CleanupTransition::CallCommit { call, arguments } = transition {
                    if call == &at && mode == 1 {
                        arguments.pop().unwrap();
                        changed = true;
                    }
                    if call == &at && mode == 2 {
                        // A forged original/stale value epoch cannot replace the exact staged argument.
                        arguments[1].source = later.clone();
                        changed = true;
                    }
                }
            }
        }
        if mode == 3 {
            let exit = forged
                .cleanup_plan
                .exits
                .iter_mut()
                .find(|exit| exit.finalize_in_order.len() >= 2)
                .unwrap();
            exit.finalize_in_order.swap(0, 1);
            changed = true;
        }
        assert!(changed, "hostile control must alter the real Vec call");
        let error = validate_structure(&program, &forged).unwrap_err();
        assert_eq!(error.code, "SPX-H006", "mode {mode}: {error:?}");
    }
}

#[test]
fn failing_later_owned_argument_keeps_the_staged_vector_and_unrelated_owner() {
    let source = SOURCE.replace(
        "let item=Row{number:7,text:\"ab\"};\n  words=vec_push<Row>(words,item);",
        "words=vec_push<Row>(words,Row{number:7,text:string_from_i64(10/(index-index))});",
    );
    assert_ne!(source, SOURCE);
    let checked = crate::check(&source, "owned-rhs-failure.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    validate_structure(&program, function).unwrap();
    // The later constructor-field failure must settle the already-staged first argument,
    // while the untouched later vector retains its own ordinary cleanup epoch.
    assert!(function.cleanup_plan.exits.iter().any(|exit| {
        matches!(exit.continuation, ExitContinuation::ReturnFailure { .. })
            && exit.finalize_in_order.iter().any(|action| {
                matches!(
                    action.source.storage,
                    StorageId::CallArgument {
                        parameter_index: 0,
                        ..
                    }
                )
            })
            && exit
                .finalize_in_order
                .iter()
                .any(|action| matches!(action.source.storage, StorageId::Value(_)))
    }));
    crate::codegen::emit_hir_c(&program).unwrap();
    crate::wasm::emit_resolved_module(&program).unwrap();
}
