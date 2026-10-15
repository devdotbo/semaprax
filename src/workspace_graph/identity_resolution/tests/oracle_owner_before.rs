use super::*;

pub(super) fn one_mib_ledger_preserves_first_cursor_refusal_and_sticky_retries() {
    let program = crate::check(SOURCE, "identity-workspace.spx").unwrap();
    let input = Input::Owned(program.clone());
    let borrowed = Input::Borrowed(SyntheticProgramView::new(&program, Vec::new()).unwrap());
    let (oracle, legacy_refused, _) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            legacy_receiver::resolve_module(None, &program.path, &input, None).unwrap()
        });
    assert!(!legacy_refused);
    let expected_reused = oracle.2;
    let expected_static = oracle.3;
    let expected_costs = &oracle.1;
    with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
        resolve_module(None, &program.path, &input, None).unwrap()
    });
    // Keep the original literal ledger, all three invocations and their live
    // receiving results. A reduced real request may move the refusal frontier.
    let (((), observed), failed, used, receipt) = with_limit_usage_refusal(1 << 20, || {
        measure(|| {
            let run = |current: &Input<'_>| {
                let result = resolve_module(None, &program.path, current, None);
                match &result {
                    Ok(parts) => {
                        assert_eq!(parts.0, oracle.0);
                        assert_eq!(&parts.1, expected_costs);
                        assert_eq!(parts.2, expected_reused);
                        assert_eq!(parts.3, expected_static);
                        assert!(!crate::bounded_output::active_refused());
                    }
                    Err(_) => {
                        assert!(crate::bounded_output::active_refused());
                        let remaining = crate::bounded_output::active_remaining();
                        assert!(!crate::bounded_output::reserve_active_required(0));
                        assert!(!crate::bounded_output::reserve_active_required(1));
                        let live = current_metrics();
                        let retry = resolve_module(None, &program.path, current, None);
                        let after = current_metrics();
                        assert_eq!(
                            (after.allocations, after.live),
                            (live.allocations, live.live)
                        );
                        assert!(matches!(
                            retry,
                            Err(ModuleFailure::Setup(Refused::Budget(_)))
                        ));
                        assert_eq!(remaining, crate::bounded_output::active_remaining());
                        drop(retry);
                    }
                }
                result
            };
            let stage0 = run(&input);
            let stage1 = run(&borrowed);
            let retry = run(&input);
            drop((stage0, stage1, retry));
        })
    });
    assert_eq!(receipt.is_some(), failed);
    if let Some(first) = receipt {
        assert!(first.requested > first.remaining);
    }
    assert!(used <= 1 << 20);
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
}
