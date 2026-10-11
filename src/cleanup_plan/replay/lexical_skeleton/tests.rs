//! Continuation state is independent of observation tails and DAG factoring.
use super::*;

fn owner() -> crate::hir::FunctionExecutionId {
    crate::hir::FunctionExecutionId::Monomorphic(DeclarationId::new("test.continuation"))
}

fn exit(kind: LexicalExitKind, site: &str) -> std::rc::Rc<SkeletonObservation> {
    std::rc::Rc::new(SkeletonObservation::LexicalExit {
        site: ExpressionId::new(&owner(), site),
        kind,
        target: ControlTarget::Function(owner()),
    })
}

#[test]
fn return_marker_survives_an_empty_tail_and_releases_once() {
    let observation = exit(LexicalExitKind::Return, "return.site");
    let mut path = empty_expr_path();
    path.continuation.record(&observation);
    path.failed = true;
    // A marker in a merged prefix has no flat tail to inspect.
    path.observations.prefix = Some(std::rc::Rc::new(skeleton_work::ObservationDag::Seq(
        None,
        vec![observation],
    )));
    assert!(path.observations.is_empty());
    assert!(matches!(
        marker(&path),
        Some(Marker::Exit {
            kind: LexicalExitKind::Return,
            ..
        })
    ));
    release_returns(std::slice::from_mut(&mut path));
    assert!(!path.failed);
    assert_eq!(path.continuation, Continuation::Normal);
    assert!(marker(&path).is_none());
    release_returns(std::slice::from_mut(&mut path));
    assert_eq!(terminal(&path), SkeletonTerminal::Success);
}

#[test]
fn continuation_identity_preserves_exit_site_and_advance_phase() {
    let first = exit(LexicalExitKind::Continue, "first.site");
    let second = exit(LexicalExitKind::Continue, "second.site");
    assert_ne!(
        Continuation::Exit(first.clone()),
        Continuation::Exit(second)
    );
    assert_ne!(
        Continuation::Exit(first.clone()),
        Continuation::Advancing(first.clone())
    );
    let advancing = Continuation::Advancing(first);
    assert_eq!(
        Continuation::sequenced(&advancing, &Continuation::Normal),
        advancing
    );
    let failed = Continuation::Exit(exit(LexicalExitKind::Return, "return.site"));
    assert_eq!(Continuation::sequenced(&advancing, &failed), failed);
}
