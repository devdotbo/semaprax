//! Independently enumerate the small plan automaton, including duplicate routes.
//! This oracle never calls the weighted comparison or its closure/canonicalizer.
use super::*;

pub(super) const WHILE: &str = r#"module factored.lexical;
@id("probe") fn probe(n:i64)->i64 {
    let mut i=0;
    while i<n {
        i=i+1;
        if i==1 {continue;};
        if i==2 {break;};
        if i==3 {return i;};
        0
    }
    return i;
}
@id("main") fn main()->i64 {0}
"#;
pub(super) const GENERATED: &str = r#"module factored.generated;
@id("probe") fn probe(n:i64)->i64 {
    let mut building=vec_with_capacity<i64>(3usize);
    building=vec_push<i64>(building,1);
    building=vec_push<i64>(building,2);
    let values=building;
    let mut total=0;
    for item in values {
        if item==1 {continue;};
        if item==n {break;};
        total=total+item;
        0
    }
    return total;
}
@id("main") fn main()->i64 {0}
"#;

pub(super) fn fixture(source: &str) -> (ResolvedProgram, ResolvedFunction) {
    let parsed = crate::check(source, "factored-lexical-plan.spx").unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    let function = program
        .functions()
        .iter()
        .find(|function| function.id.as_str() == "probe")
        .unwrap()
        .clone();
    validate_structure(&program, &function).unwrap();
    (program, function)
}

fn enumerate(automaton: &Automaton, root: usize) -> Vec<SkeletonPath> {
    fn visit(
        automaton: &Automaton,
        node: usize,
        observations: Vec<Rc<SkeletonObservation>>,
        depth: usize,
        result: &mut Vec<SkeletonPath>,
    ) {
        assert!(
            depth < 512 && result.len() < MAX_REPLAY_PATHS,
            "small independent oracle bound"
        );
        for target in &automaton.nodes[node].epsilon {
            visit(automaton, *target, observations.clone(), depth + 1, result);
        }
        for (label, target) in &automaton.nodes[node].moves {
            match label {
                Label::Observation(observation) => {
                    let mut next = observations.clone();
                    next.push(observation.clone());
                    visit(automaton, *target, next, depth + 1, result);
                }
                Label::End(terminal) => result.push(SkeletonPath {
                    observations: observations.clone().into(),
                    terminal: *terminal,
                }),
            }
        }
    }
    let mut result = Vec::new();
    visit(automaton, root, vec![], 0, &mut result);
    result.sort();
    result
}

fn assert_plan_oracle(function: &ResolvedFunction) {
    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    let mut expected = plan_skeleton_paths(function, &mut budget).unwrap();
    expected.sort();
    let mut automaton = Automaton::default();
    let sink = automaton.node(function, &mut budget).unwrap();
    let success = automaton
        .end(function, &mut budget, sink, SkeletonTerminal::Success)
        .unwrap();
    let failure = automaton
        .end(function, &mut budget, sink, SkeletonTerminal::Failure)
        .unwrap();
    let latch = automaton
        .end(function, &mut budget, sink, SkeletonTerminal::LoopLatch)
        .unwrap();
    let root = automaton
        .add_plan(function, success, failure, latch, &mut budget)
        .unwrap();
    assert_eq!(enumerate(&automaton, root), expected);
    // Every lexical exit directly contributes a marker edge in the converter;
    // marker targets retain either the certified resume or the exact terminal.
    assert!(automaton.nodes.iter().any(|node|node.moves.iter().any(|(label,_)|matches!(label,Label::Observation(observation) if matches!(observation.as_ref(),SkeletonObservation::LexicalExit{..})))));
}

#[test]
fn scalar_lexical_plan_automaton_equals_the_complete_enumerated_multiset() {
    let (_, function) = fixture(WHILE);
    assert_plan_oracle(&function);
}

#[test]
fn generated_continue_plan_preserves_advance_resume_and_latch_trace() {
    let (_, function) = fixture(GENERATED);
    assert_plan_oracle(&function);
    let control = function.cleanup_plan.lexical_control.as_ref().unwrap();
    assert!(control.exits.iter().any(|certificate| certificate.kind
        == crate::ast::LexicalExitKind::Continue
        && lexical_cert::latch_resume(&function.cleanup_plan, certificate.exit).is_some()));
}

#[test]
fn lexical_marker_site_and_owner_tampering_changes_the_typed_trace() {
    let (program, function) = fixture(WHILE);
    let mut budget = ReplayBudget::new();
    assert_eq!(
        compare(&program, &function, &mut budget).unwrap(),
        Some(true)
    );
    for mutate_owner in [false, true] {
        let mut forged = function.clone();
        let certificate = forged
            .cleanup_plan
            .lexical_control
            .as_mut()
            .unwrap()
            .exits
            .iter_mut()
            .find(|certificate| certificate.kind == crate::ast::LexicalExitKind::Return)
            .unwrap();
        if mutate_owner {
            certificate.target = crate::hir::ControlTarget::Function(
                crate::hir::FunctionExecutionId::Monomorphic(DeclarationId::new("wrong.owner")),
            );
        } else {
            let owner = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
            certificate.site = ExpressionId::new(&owner, "forged.return.site");
        }
        let mut budget = ReplayBudget::new();
        assert_eq!(
            compare(&program, &forged, &mut budget).unwrap(),
            Some(false)
        );
        assert!(validate_structure(&program, &forged).is_err());
    }
}

#[test]
fn generated_continue_resume_omission_changes_the_complete_typed_trace() {
    let (program, function) = fixture(GENERATED);
    let mut budget = ReplayBudget::new();
    assert_eq!(
        compare(&program, &function, &mut budget).unwrap(),
        Some(true)
    );
    let mut forged = function.clone();
    let certificate = forged
        .cleanup_plan
        .lexical_control
        .as_mut()
        .unwrap()
        .exits
        .iter_mut()
        .find(|certificate| certificate.kind == crate::ast::LexicalExitKind::Continue)
        .unwrap();
    assert!(certificate.resume.take().is_some());
    let mut budget = ReplayBudget::new();
    assert_eq!(
        compare(&program, &forged, &mut budget).unwrap(),
        Some(false)
    );
    assert!(validate_structure(&program, &forged).is_err());
}
