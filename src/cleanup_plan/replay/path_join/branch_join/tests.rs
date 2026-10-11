//! Actual checked owners at body.s999/body.s1000: identical `if` arms
//! deduplicate to one replay state, yet the producer join re-keys the roots.
//! The accounting controls below spend the same `ReplayBudget` the replay
//! uses; none raises a limit, and no fixture vector is sorted or repaired.
use super::*;
use crate::cleanup_plan::replay::physical_storage::PhysicalReservation;
use crate::cleanup_plan::{ExitContinuation, StatusLane, StorageId};

mod fixtures {
    include!("../../../../codegen/native_bytes/terminal_exits/fixtures.rs");
}
const TEMPLATE: &str =
    include_str!("../../../../codegen/native_bytes/terminal_exits/variant_owner_order.spx");
const LEXICAL_LOOP: &str = "    let mut index = 0;\n    while index < rounds {\n        index = index + 1;\n        if index == 1 { continue; }\n        if index == 3 { break; }\n        0\n    }\n";
const RUN: &str = "test.native_terminal_variant_order.run";

fn run(program: &crate::hir::ResolvedProgram) -> &crate::hir::ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == RUN)
        .expect("run is declared")
}

/// Swap the two opposing owner finalizers of one call's exact failure exit,
/// leaving every action and every other finalizer verbatim.
fn swap_owner_finalizers(program: &mut crate::hir::ResolvedProgram, call: &str) {
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == RUN)
        .expect("run is declared");
    let older = StorageId::Value(fixtures::binding(function, "older").0.id.clone());
    let newer = StorageId::Value(fixtures::binding(function, "newer").0.id.clone());
    let site = fixtures::binding(function, call).1.id.clone();
    let mut exits = function.cleanup_plan.exits.iter_mut().filter(|exit| {
        matches!(&exit.continuation, ExitContinuation::ReturnFailure { source }
            if source.expression == site && source.lane == StatusLane::OperationFailure)
    });
    let exit = exits.next().expect("call has its exact failure exit");
    let owners = exit
        .finalize_in_order
        .iter()
        .enumerate()
        .filter(|(_, action)| action.source.storage == older || action.source.storage == newer)
        .map(|(index, action)| {
            assert!(
                action.active_case.is_some(),
                "owner finalizer is conditional"
            );
            index
        })
        .collect::<Vec<_>>();
    assert_eq!(owners.len(), 2, "one conditional finalizer per owner");
    exit.finalize_in_order.swap(owners[0], owners[1]);
    assert!(exits.next().is_none(), "call selects one failure exit");
}

#[test]
fn identical_branch_arms_replay_the_producer_join_and_refuse_either_repaired_order() {
    let lexical = fixtures::source(TEMPLATE);
    assert_eq!(lexical.matches(LEXICAL_LOOP).count(), 1);
    let nonlexical = lexical.replace(LEXICAL_LOOP, "    let index = rounds;\n");
    for (source, lexical_control) in [(lexical, true), (nonlexical, false)] {
        let checked = crate::check(&source, std::path::Path::new("branch-join-order.spx"))
            .expect("variant owner order source checks");
        fixtures::assert_projection(&checked);
        let program = crate::hir::resolve(&checked)
            .expect("independent replay admits the producer's branch-join order");
        assert_eq!(
            run(&program).cleanup_plan.lexical_control.is_some(),
            lexical_control
        );
        // Genuine opposing canonical vectors: newest-first before the join,
        // re-keyed (s1000 before s999) after it. Neither is repaired here.
        fixtures::assert_opposing_ids(run(&program));
        crate::cleanup_plan::replay::validate_program(&program)
            .expect("the actual opposing vectors replay exactly");

        // `after` swapped is the deduplicated initialization order the
        // replay once demanded; `before` swapped is the join order applied
        // too early. Exact equality refuses both.
        for call in ["after", "before"] {
            let mut forged = program.clone();
            swap_owner_finalizers(&mut forged, call);
            let error = crate::cleanup_plan::replay::validate_program(&forged)
                .expect_err("a repaired owner order is not the canonical vector");
            assert_eq!(error.code, "SPX-H006", "{call}: {error:?}");
            assert_eq!(
                error.message,
                format!(
                    "cleanup plan for function `{RUN}` failed independent replay: \
                     exit does not preserve exact conditional variant finalizers"
                ),
                "{call}"
            );
        }
    }
}

const OWNING_BREAK: &str = r#"module test.branch_join_owning_break;

@id("join.sum")
fn sum(values: own Vec<Bytes>, stop: i64) -> i64 {
    let mut total = 0;
    for own item in vec_into_iter<Bytes>(values) {
        total = total + 1;
        let length = total;
        if length == 2 { continue; };
        if length == stop { break; };
        0
    }
    return total;
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn checked(source: &str) -> crate::hir::ResolvedProgram {
    let parsed = crate::check(source, std::path::Path::new("branch-join-accounting.spx"))
        .expect("accounting source checks");
    crate::hir::resolve(&parsed).expect("canonical plans replay")
}

fn work(remaining: usize) -> ReplayBudget {
    ReplayBudget {
        remaining,
        ..ReplayBudget::new()
    }
}

fn backing(bytes: usize) -> ReplayBudget {
    let mut budget = ReplayBudget::new();
    budget.physical = Some(Box::new(PhysicalReservation::new(bytes)));
    budget
}

/// Independent oracle over each predecessor's own terminator.
fn goto_joins(function: &ResolvedFunction) -> Vec<BlockId> {
    let plan = &function.cleanup_plan;
    plan.blocks
        .iter()
        .map(|block| block.id)
        .filter(|block| {
            let incoming = plan
                .edges
                .iter()
                .filter(|edge| edge.to == *block)
                .collect::<Vec<_>>();
            incoming.len() >= 2
                && incoming.iter().all(|edge| {
                    edge.condition == EdgeCondition::Always
                        && plan.blocks[edge.from.0 as usize].terminator
                            == CleanupTerminator::Goto(edge.id)
                })
        })
        .collect()
}

fn assert_refused(error: &Diagnostic, phase: &str) {
    assert_eq!(error.code, "SPX-H006", "{error:?}");
    assert!(error.message.contains(phase), "{phase}: {error:?}");
}

#[test]
fn classifier_debits_initialization_every_edge_visit_and_every_later_probe() {
    let program = checked(&fixtures::source(TEMPLATE));
    let function = run(&program);
    let plan = &function.cleanup_plan;
    let joins = goto_joins(function);
    assert!(!joins.is_empty(), "the identical `if` arms rejoin by Goto");
    let first = plan.blocks.len() + plan.edges.len() + 1;

    // Exact: one classification, then one unit per later probe.
    let mut exact = work(first + plan.blocks.len() - 1);
    let mut classifier = BranchJoins::default();
    for block in &plan.blocks {
        assert_eq!(
            classifier.contains(function, block.id, &mut exact).unwrap(),
            joins.contains(&block.id),
            "{:?}",
            block.id
        );
    }
    assert_eq!(exact.remaining, 0);
    let later = classifier
        .contains(function, joins[0], &mut exact)
        .unwrap_err();
    assert_refused(
        &later,
        "work budget exhausted during branch join classification",
    );

    // One short: refused before the zeroed shapes exist, and sticky because
    // nothing is cached or debited, so every retry pays the whole debit.
    let mut short = work(first - 1);
    let mut classifier = BranchJoins::default();
    for _ in 0..2 {
        let error = classifier
            .contains(function, joins[0], &mut short)
            .unwrap_err();
        assert_refused(
            &error,
            "work budget exhausted during branch join classification",
        );
        assert!(classifier.0.is_none(), "a refusal caches no shapes");
        assert_eq!(short.remaining, first - 1);
    }
}

#[test]
fn classifier_shape_backing_is_exact_and_a_short_ledger_stays_refused() {
    let program = checked(&fixtures::source(TEMPLATE));
    let function = run(&program);
    let joins = goto_joins(function);
    let bytes = function.cleanup_plan.blocks.len() * std::mem::size_of::<usize>();

    let mut exact = backing(bytes);
    assert!(BranchJoins::default()
        .contains(function, joins[0], &mut exact)
        .unwrap());
    assert_eq!(exact.physical.as_deref().unwrap().charged(), bytes);

    let mut short = backing(bytes - 1);
    let mut classifier = BranchJoins::default();
    for _ in 0..2 {
        let error = classifier
            .contains(function, joins[0], &mut short)
            .unwrap_err();
        assert_refused(
            &error,
            "physical reservation exhausted during branch join shapes",
        );
        assert!(classifier.0.is_none());
        assert_eq!(short.physical.as_deref().unwrap().charged(), 0);
    }
}

#[test]
fn mixed_and_single_edge_inputs_are_not_branch_joins() {
    let program = checked(&fixtures::source(TEMPLATE));
    let function = run(&program);
    let join = goto_joins(function)[0];
    let incoming = function
        .cleanup_plan
        .edges
        .iter()
        .filter(|edge| edge.to == join)
        .map(|edge| edge.id)
        .collect::<Vec<_>>();
    let edge = incoming[0];
    let from = function.cleanup_plan.edges[edge.0 as usize].from.0 as usize;
    let classify = |function: &ResolvedFunction| {
        BranchJoins::default()
            .contains(function, join, &mut ReplayBudget::new())
            .unwrap()
    };
    assert!(classify(function));

    let mut branch = function.clone();
    branch.cleanup_plan.blocks[from].terminator = CleanupTerminator::Branch(vec![edge]);
    let mut exit = function.clone();
    exit.cleanup_plan.blocks[from].terminator =
        CleanupTerminator::Exit(function.cleanup_plan.exits[0].id);
    let mut conditioned = function.clone();
    conditioned.cleanup_plan.edges[edge.0 as usize].condition =
        EdgeCondition::BooleanResult(function.body.id.clone(), true);
    let mut single = function.clone();
    for other in &incoming[1..] {
        single.cleanup_plan.edges[other.0 as usize].to = function.cleanup_plan.entry;
    }
    for (name, forged) in [
        ("branch", branch),
        ("exit continuation", exit),
        ("conditioned goto", conditioned),
        ("one edge", single),
    ] {
        assert!(!classify(&forged), "{name}");
    }
}

#[test]
fn actual_lexical_exact_rejoin_is_never_rekeyed_or_charged() {
    let program = checked(OWNING_BREAK);
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "join.sum")
        .expect("sum is declared");
    let control = function
        .cleanup_plan
        .lexical_control
        .as_deref()
        .expect("owning break carries a lexical certificate");
    let entry = control
        .exits
        .iter()
        .find(|exit| exit.kind == crate::ast::LexicalExitKind::Break)
        .and_then(|exit| exit.resume)
        .expect("owning break completes its wrapper in a fragment");
    let fragment = &control
        .resumes
        .iter()
        .find(|resume| resume.entry == entry)
        .expect("break fragment is certified")
        .blocks;
    let mut leaving = function
        .cleanup_plan
        .edges
        .iter()
        .filter(|edge| fragment.contains(&edge.from) && !fragment.contains(&edge.to));
    let rejoin = leaving.next().expect("fragment rejoins its parent").to;
    assert!(leaving.next().is_none(), "exactly one exact rejoin");

    // Opposing root key order, which a generic join would re-key.
    let crossed = ["b", "a"]
        .into_iter()
        .map(|name| ReplayConditionalVariant {
            root: CleanupPlace {
                storage: StorageId::Temporary(ExpressionId::from_owned(name.to_owned())),
                projections: Vec::new(),
            },
            variant: DeclarationId::new("join.forged"),
            cases: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut joins = BranchJoins::default();
    let mut none = work(0);
    let kept = joins
        .normalize(&program, function, rejoin, true, crossed.clone(), &mut none)
        .unwrap();
    assert_eq!(kept, crossed);
    assert!(joins.0.is_none(), "an exact rejoin never classifies");
    let error = joins
        .normalize(&program, function, rejoin, false, crossed, &mut none)
        .unwrap_err();
    assert_refused(
        &error,
        "work budget exhausted during branch join estimate visits",
    );
}

#[test]
fn normalization_debits_root_comparisons_and_the_self_join_before_rekeying() {
    let program = checked(&fixtures::source(TEMPLATE));
    let function = run(&program);
    let plan = &function.cleanup_plan;
    let join = goto_joins(function)[0];
    let choice = DeclarationId::new("test.native_terminal_variant_order.Choice");
    let domain = ReplayContext::declarations(&program)
        .variant_cases(&choice)
        .expect("Choice has a closed domain");
    let variant = |name: &str, flag: u32| ReplayConditionalVariant {
        root: CleanupPlace {
            storage: StorageId::Value(fixtures::binding(function, name).0.id.clone()),
            projections: Vec::new(),
        },
        variant: choice.clone(),
        cases: domain
            .iter()
            .map(|case| {
                let payload = case.id.as_str().ends_with(".data");
                let flags = if payload {
                    vec![LivenessFlagId(flag)]
                } else {
                    Vec::new()
                };
                (case.id.clone(), flags)
            })
            .collect(),
    };
    // Initialization order, then the producer's key order (s1000 before s999).
    let initialized = vec![variant("older", 1), variant("newer", 2)];
    let keyed = vec![initialized[1].clone(), initialized[0].clone()];

    // Key order pays exactly its one root comparison and never classifies.
    // Independent no-projection Value-root census: two storage visits, plus
    // both textual Value IDs/discriminants and one comparison boundary.
    let pair = fixtures::binding(function, "older").0.id.as_str().len()
        + fixtures::binding(function, "newer").0.id.as_str().len()
        + 7;
    let estimate_visits = 2;
    let mut estimate = work(estimate_visits);
    let predicted = root_pair_units(&keyed[0].root, &keyed[1].root, function, &mut estimate)
        .unwrap()
        .unwrap();
    assert_eq!(estimate.remaining, 0);
    assert_eq!(predicted, pair);
    let mut ordered = work(pair + estimate_visits);
    let mut joins = BranchJoins::default();
    let kept = joins
        .normalize(&program, function, join, false, keyed.clone(), &mut ordered)
        .unwrap();
    assert_eq!(kept, keyed);
    assert_eq!(ordered.remaining, 0);
    assert!(joins.0.is_none());

    let mut ample = ReplayBudget::new();
    let rekeyed = BranchJoins::default()
        .normalize(
            &program,
            function,
            join,
            false,
            initialized.clone(),
            &mut ample,
        )
        .unwrap();
    assert_eq!(rekeyed, keyed);
    let spent = MAX_REPLAY_WORK_UNITS - ample.remaining;
    assert!(
        spent > pair + plan.blocks.len() + plan.edges.len() + 1,
        "the self-join is charged beyond the comparison and classification"
    );

    let mut exact = work(spent);
    let rekeyed = BranchJoins::default()
        .normalize(
            &program,
            function,
            join,
            false,
            initialized.clone(),
            &mut exact,
        )
        .unwrap();
    assert_eq!(rekeyed, keyed);
    assert_eq!(exact.remaining, 0);

    let mut short = work(spent - 1);
    let mut joins = BranchJoins::default();
    for _ in 0..2 {
        let error = joins
            .normalize(
                &program,
                function,
                join,
                false,
                initialized.clone(),
                &mut short,
            )
            .unwrap_err();
        assert_refused(&error, "work budget exhausted during branch join");
    }
}

#[test]
fn estimator_visits_use_the_actual_paid_sticky_ledger() {
    let program = checked(&fixtures::source(TEMPLATE));
    let function = run(&program);
    let place = CleanupPlace {
        storage: StorageId::ProvisionalResult,
        projections: vec![DeclarationId::new("a"), DeclarationId::new("long")],
    };
    // One root storage probe plus two projections, before key arithmetic.
    let mut exact = work(3);
    assert_eq!(
        place_key_units(&place, function, &mut exact).unwrap(),
        Some(9)
    );
    assert_eq!(exact.remaining, 0);
    let choice = DeclarationId::new("test.native_terminal_variant_order.Choice");
    let domain = ReplayContext::declarations(&program)
        .variant_cases(&choice)
        .unwrap();
    let value = ReplayConditionalVariant {
        root: place.clone(),
        variant: choice,
        cases: vec![
            (DeclarationId::new("a"), Vec::new()),
            (DeclarationId::new("b"), Vec::new()),
        ],
    };
    // Two variant visits, two root probes (each with two projections), and
    // the one two-case scan: 2 + 2*3 + 2 = 10 actual estimator visits.
    let mut self_visits = work(10);
    assert!(
        self_join_units(std::slice::from_ref(&value), function, &mut self_visits)
            .unwrap()
            .is_some()
    );
    assert_eq!(self_visits.remaining, 0);
    // Two distinct two-case scans and one visit per actual domain member.
    let mut domain_visits = work(4 + domain.len());
    assert!(domain_units(&value, domain, function, &mut domain_visits)
        .unwrap()
        .is_some());
    assert_eq!(domain_visits.remaining, 0);
    let mut paid =
        ReplayBudget::from_reservation(function, PhysicalReservation::new(usize::MAX)).unwrap();
    paid.remaining = 2;
    assert!(place_key_units(&place, function, &mut paid).is_err());
    assert!(paid.refusal_spent);
    assert_eq!(paid.remaining, 0);
    assert!(paid.charge(function, 0, "zero retry").is_err());
}
