use super::*;
use crate::test_allocation_observer::measure;
use sha2::Digest;

const CAP: usize = 64 * 1024 * 1024;
fn fixture(rows: usize, edges: bool) -> ResolvedProgram {
    let mut source = String::from("module cross.phase; @id(\"cross.add\") fn add(x:i64)->i64 requires x>=0 ensures result>=0 {x+1} @id(\"cross.main\") fn main()->i64 {");
    for n in 0..rows {
        source.push_str(&format!("let v{n}=add(0)+3-2*4/2%3;"));
        if edges {
            source.push_str(&format!("let b{n}=if v{n}>0 {{1}} else {{2}};"));
        }
    }
    source.push_str("-1}");
    crate::hir::resolve(&crate::check(&source, "cross-phase.spx").unwrap()).unwrap()
}
fn function(program: &ResolvedProgram) -> &ResolvedFunction {
    program.functions.iter().find(|f| f.name == "main").unwrap()
}
fn queries(function: &ResolvedFunction) -> usize {
    function
        .cleanup_plan
        .edges
        .iter()
        .filter(|e| {
            matches!(
                &e.condition,
                EdgeCondition::BooleanResult(..)
                    | EdgeCondition::VariantCase { .. }
                    | EdgeCondition::ArmSelected { .. }
            )
        })
        .count()
}
fn run(
    program: &ResolvedProgram,
    old: bool,
    cap: usize,
    work: usize,
) -> (Result<(), Diagnostic>, bool, usize, usize) {
    let ((result, work), refused, paid) = crate::bounded_output::with_limit_usage(cap, || {
        let mut budget = ReplayBudget::new();
        budget.remaining = work;
        let result = if old {
            before::validate_structure_with_budget_storage(
                program,
                function(program),
                &mut budget,
                None,
                None,
            )
        } else {
            validate_structure_with_budget_storage(
                program,
                function(program),
                &mut budget,
                None,
                None,
            )
        };
        (result, work - budget.remaining)
    });
    (result, refused, paid, work)
}

#[test]
fn actual_status_to_initial_edge_owner_has_full_typed_net_and_unchanged_work() {
    let program = fixture(64, true);
    let function = function(&program);
    assert!(!lexical_cert::requires_lexical(function));
    assert!(function.cleanup_plan.status_sources.len() >= retained_lookup::MIN_ROWS);
    let q = queries(function);
    assert!(
        q > 0,
        "genuine typed expression-bearing edge witness required"
    );
    let old = run(&program, true, CAP, MAX_REPLAY_WORK_UNITS);
    let current = run(&program, false, CAP, MAX_REPLAY_WORK_UNITS);
    assert_eq!(current.0, old.0);
    assert!(current.0.is_ok() && !current.1 && !old.1);
    let s = Session::OLD_SCRATCH;
    let r = Session::EDGE_QUERY_BYTES;
    assert!(s > r);
    assert_eq!(current.3, old.3, "all original work cuts remain");
    // Existing status acquisition remains fully paid; no second owner or
    // transported continuation is formed. Both old compound Results remain.
    assert_eq!(old.2.checked_sub(current.2), Some(q * (s - r)));
    eprintln!("cross-phase actual status={} edge_queries={q} old={old:?} current={current:?} S={s} R={r} M_new=0 A_new=0", function.cleanup_plan.status_sources.len());
    let exact = run(&program, false, current.2, MAX_REPLAY_WORK_UNITS);
    assert!(exact.0.is_ok() && !exact.1);
    for cap in [0, current.2 - 1] {
        let short = run(&program, false, cap, MAX_REPLAY_WORK_UNITS);
        assert!(short.0.is_err() && short.1);
    }
}

#[test]
fn dense_status_with_zero_expression_edges_keeps_the_whole_immediate_bill() {
    let program = fixture(64, false);
    assert_eq!(queries(function(&program)), 0);
    assert!(function(&program).cleanup_plan.status_sources.len() >= retained_lookup::MIN_ROWS);
    assert_eq!(
        run(&program, false, CAP, MAX_REPLAY_WORK_UNITS),
        run(&program, true, CAP, MAX_REPLAY_WORK_UNITS)
    );
    let tiny = fixture(0, false);
    assert_eq!(
        run(&tiny, false, CAP, MAX_REPLAY_WORK_UNITS),
        run(&tiny, true, CAP, MAX_REPLAY_WORK_UNITS)
    );
}

#[test]
fn schema_regions_entry_and_edge_identity_keep_first_six_field_diagnostics() {
    let original = fixture(64, true);
    for mode in 0..4 {
        let mut program = original.clone();
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.name == "main")
            .unwrap();
        match mode {
            0 => function.cleanup_plan.schema = "foreign.cleanup.schema",
            1 => function.cleanup_plan.regions[0].id.0 = u32::MAX,
            2 => function.cleanup_plan.entry.0 = u32::MAX,
            _ => function.cleanup_plan.edges[0].id.0 = u32::MAX,
        }
        let old = run(&program, true, CAP, MAX_REPLAY_WORK_UNITS);
        let current = run(&program, false, CAP, MAX_REPLAY_WORK_UNITS);
        assert!(old.0.is_err());
        assert_eq!(current, old, "complete early tuple mode={mode}");
    }
}

#[test]
fn later_unknown_expression_keeps_full_diagnostic_and_original_work_frontier() {
    let mut program = fixture(64, true);
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.name == "main")
        .unwrap();
    let edge = function
        .cleanup_plan
        .edges
        .iter_mut()
        .rev()
        .find(|e| {
            matches!(
                &e.condition,
                EdgeCondition::BooleanResult(..)
                    | EdgeCondition::VariantCase { .. }
                    | EdgeCondition::ArmSelected { .. }
            )
        })
        .unwrap();
    match &mut edge.condition {
        EdgeCondition::BooleanResult(id, _)
        | EdgeCondition::VariantCase { scrutinee: id, .. }
        | EdgeCondition::ArmSelected { scrutinee: id, .. } => {
            *id = ExpressionId::from_owned("missing.λ.edge".into())
        }
        _ => unreachable!(),
    }
    let old = run(&program, true, CAP, MAX_REPLAY_WORK_UNITS);
    let current = run(&program, false, CAP, MAX_REPLAY_WORK_UNITS);
    assert!(old.0.is_err());
    assert_eq!(current.0, old.0, "all six diagnostic fields");
    assert_eq!(current.3, old.3);
}

#[test]
fn first_and_later_original_work_cuts_preserve_complete_diagnostics() {
    let program = fixture(64, true);
    let complete = run(&program, true, CAP, MAX_REPLAY_WORK_UNITS);
    assert!(complete.0.is_ok());
    for work in [0, 1, 12, 64, complete.3 / 2, complete.3 - 1, complete.3] {
        let old = run(&program, true, CAP, work);
        let current = run(&program, false, CAP, work);
        assert_eq!(current.0, old.0, "all six fields work={work}");
        assert_eq!(current.3, old.3);
    }
}

#[test]
fn whole_caller_drop_releases_actual_backing_on_success_and_refusal() {
    let program = fixture(64, true);
    // Warm only test observer state, outside either actual measured caller.
    for old in [true, false] {
        drop(run(&program, old, CAP, MAX_REPLAY_WORK_UNITS));
    }
    for cap in [0, CAP] {
        for old in [true, false] {
            let (_, metrics) = measure(|| {
                drop(run(&program, old, cap, MAX_REPLAY_WORK_UNITS));
            });
            assert!(!metrics.invalid);
            assert_eq!(
                metrics.live, 0,
                "all backing released old={old} cap={cap}: {metrics:?}"
            );
            eprintln!("cross-phase physical old={old} cap={cap} {metrics:?}");
        }
    }
}

#[test]
fn same_abi_source_has_no_early_owner_or_selector_and_consumes_budget_before_callbacks() {
    let source = include_str!("ordinary_cross_phase.rs");
    let guard = source
        .find("if !prefix_owner.insert(function, &source.id, budget)?")
        .unwrap();
    let receive = source
        .find("retained status fact receiving assignment")
        .unwrap();
    let acquire = source
        .find("let mut session = Session::new_prepaid")
        .unwrap();
    assert!(guard < receive && receive < acquire);
    let start = source
        .find("let mut session = Session::new_prepaid")
        .unwrap();
    let end = source[start..]
        .find("let budget = session.budget;")
        .unwrap()
        + start;
    let closed = &source[start..end];
    for forbidden in [
        "program.full_program(",
        "renewal::validate_binding(",
        "type_needs_drop(",
        "RefCell",
        "FnMut",
        "Option<Session",
        "with_limit(",
        "ACTIVE.with",
    ] {
        assert!(
            !closed.contains(forbidden),
            "closed mutable custody admits {forbidden}"
        );
    }
    assert!(closed.contains("Session::EDGE_RECEIVING_BYTES"));
    assert!(source[end..].contains("renewal::validate_binding("));
    assert!(!source[end..].contains("session.fact"));
}

#[test]
fn exact_source_inverses_restore_every_whole_immediate_owner() {
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/retained-expression-fact-cross-phase-20261018/inverses.json"
    )))
    .unwrap();
    for (path,current) in [
        ("src/cleanup_plan/replay.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay.rs"))),
        ("src/cleanup_plan/replay/expression_facts/arena/retained.rs",include_str!("../../arena/retained.rs")),
        ("src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs",include_str!("../../arena/retained/tests.rs")),
        ("src/cleanup_plan/replay/expression_facts/receiving.rs",include_str!("../../receiving.rs")),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs",include_str!("../retained.rs")),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs",include_str!("case_certified.rs")),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs",include_str!("case_certified/retained_lookup.rs")),
    ] {
        // Acquire the exact whole c5 caller generation before its unchanged
        // c5 -> c336 -> 847 inverse authorities. Combined CI/raw-source edits
        // are authenticated by the existing current whole-source reader first.
        let expected_c5 = match path {
            "src/cleanup_plan/replay.rs" => "3f59678dffcd84f1fe09fd3e7dae5530501ab63ded29f032ad0831bc4244ee3e",
            "src/cleanup_plan/replay/expression_facts/arena/retained.rs" => "663cc41755cb8083333cb1702352ea59478a35ff427a6bc1039c96007b2a5093",
            "src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs" => "8368b0a218ceedd2e55dcd4c9176ccd4579b6a8389a9bc6b875c906688d0799f",
            "src/cleanup_plan/replay/expression_facts/receiving.rs" => "67f00fcd1acb1f5cd3d218670af3acc80c7ba245d21a7fb2b43d9fd3867a5d44",
            "src/cleanup_plan/replay/expression_facts/receiving/retained.rs" => "8e54417f73a57f98c016451910680f81dcc0ba7da32b08c5dea2e9b91473c63b",
            "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs" => "d9826beee4de70f840309b81447450a4e0373cdee3bb4a7c1a7f8e4f14e77709",
            "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs" => "c567429125b11ddaa563be75a0bfbb93330f581279bcd68544a2033e3dc6cbfc",
            _ => unreachable!("closed seven-authority source set"),
        };
        let acquired_c5 = crate::source_reflows_merge::main_e177_final_inputs::value_direct(
            path, expected_c5, current,
        );
        let current = acquired_c5.as_str();
        let restored_c336 = before_bare_pattern_source(path, current);
        let current = restored_c336.as_str();
        let row=records["sources"].as_array().unwrap().iter().find(|r|r["path"]==path).unwrap();
        assert_eq!(current.len(),row["after"]["length"].as_u64().unwrap()as usize);
        assert_eq!(format!("{:x}",crate::digest_hex::LowerHex(sha2::Sha256::digest(current.as_bytes()))),row["after"]["sha256"].as_str().unwrap());
        let mut restored=current.to_owned();
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let after=edit["after"].as_str().unwrap();assert_eq!(restored.matches(after).count(),1);
            restored=restored.replacen(after,edit["before"].as_str().unwrap(),1);
        }
        assert_eq!(restored.len(),row["before"]["length"].as_u64().unwrap()as usize);
        assert_eq!(format!("{:x}",crate::digest_hex::LowerHex(sha2::Sha256::digest(restored.as_bytes()))),row["before"]["sha256"].as_str().unwrap());
    }
    eprintln!(
        "cross-phase retained LexicalAttempt={} Session={} S={} Re={}",
        std::mem::size_of::<case_proof::LexicalAttempt<'_, '_>>(),
        std::mem::size_of::<Session<'_, '_, '_>>(),
        Session::OLD_SCRATCH,
        Session::EDGE_QUERY_BYTES
    );
    let path = "src/cleanup_plan/replay/expression_facts/arena/retained.rs";
    let raw = include_str!("../../arena/retained.rs");
    for bad in [
        raw[..raw.len() - 1].to_owned(),
        format!("{raw}//foreign\n"),
        format!("{raw}{raw}"),
    ] {
        assert_ne!(bad, raw);
        assert!(std::panic::catch_unwind(|| before_bare_pattern_source(path, &bad)).is_err());
    }
}

// Compose only the exact independently reviewed post-c336 source delta before
// the immutable c336 whole-source controls above. Other owners stay unchanged.
fn before_bare_pattern_source(path: &str, current: &str) -> String {
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/retained-expression-fact-cross-phase-bare-pattern-20261018/inverses.json"
    )))
    .unwrap();
    let Some(row) = records["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == path)
    else {
        return current.to_owned();
    };
    assert_eq!(
        current.len(),
        row["after"]["length"].as_u64().unwrap() as usize
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(sha2::Sha256::digest(current.as_bytes()))
        ),
        row["after"]["sha256"].as_str().unwrap()
    );
    let mut restored = current.to_owned();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(restored.matches(after).count(), 1);
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        restored.len(),
        row["before"]["length"].as_u64().unwrap() as usize
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(sha2::Sha256::digest(restored.as_bytes()))
        ),
        row["before"]["sha256"].as_str().unwrap()
    );
    restored
}
