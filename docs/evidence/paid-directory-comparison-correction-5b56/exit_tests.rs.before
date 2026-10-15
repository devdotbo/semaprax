use super::*;
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    let source = include_str!("../../../expression_facts.rs")
        .split_once("r#\"module replay.fact_owner;").unwrap().1
        .split_once("\"#,").unwrap().0;
    crate::hir::resolve(&crate::check(&format!("module replay.fact_owner;{source}"),
        "exit-prefix-proof.spx").unwrap()).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn receive(program: &ResolvedProgram, function: &ResolvedFunction, facts: &Facts<'_>,
    storage: &BTreeSet<StorageId>, leaves: &Leaves, budget: &mut ReplayBudget,
    current: bool) -> Result<(), Diagnostic>
{
    let statuses = validate_status_sources_retained(function, facts, budget)?;
    if current {
        validate_blocks_and_edges_retained(program, function, facts, storage, leaves,
            None, budget, &statuses)
    } else {
        exit_before::validate_blocks_and_edges_retained(program, function, facts, storage, leaves,
            None, budget, &statuses)
    }
}

// HIR/facts are existing immutable inputs. Every receiver allocation, result,
// retained token and budget drops while the actual observer is still active.
fn observe(program: &ResolvedProgram, function: &ResolvedFunction, current: bool,
    work_limit: usize, byte_limit: usize, expected: Option<&Result<(), Diagnostic>>)
    -> (bool, usize, usize, bool, Option<ReservationRefusal>, usize)
{
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(super::super::super::arena::build::<errors::Legacy>(function,
        capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
        &mut preparation).unwrap());
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    let ((value, allocation), refused, debit, first) = with_limit_usage_refusal(byte_limit, || measure(|| {
        let mut budget = ReplayBudget::new();
        budget.remaining = work_limit;
        let result = receive(program, function, &facts, &storage, &leaves, &mut budget, current);
        if let Some(expected) = expected { assert_eq!(&result, expected); }
        let success = result.is_ok();
        if !success && work_limit < MAX_REPLAY_WORK_UNITS && byte_limit == 1 << 26 {
            assert_eq!(result.as_ref().unwrap_err().code, "SPX-H006");
            assert_eq!(budget.remaining, 0);
        }
        let used = work_limit - budget.remaining;
        drop(result);
        if !success && budget.refusal_spent {
            let remaining = budget.remaining;
            let bytes = crate::bounded_output::active_remaining();
            let retry = receive(program, function, &facts, &storage, &leaves, &mut budget, current);
            assert!(retry.is_err());
            assert_eq!(budget.remaining, remaining);
            assert_eq!(crate::bounded_output::active_remaining(), bytes);
            drop(retry);
        }
        drop(budget);
        (success, used)
    }));
    assert!(!allocation.invalid);
    assert_eq!(allocation.live, 0);
    assert_eq!(allocation.allocated, allocation.freed);
    (value.0, value.1, debit, refused, first, allocation.allocated)
}

fn original(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let mut budget = ReplayBudget::unobserved();
    let facts = Facts::Arena(super::super::super::arena::build::<errors::Legacy>(function,
        capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
        &mut budget).unwrap());
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    receive(program, function, &facts, &storage, &leaves, &mut budget, false)
}

// Independently execute the actual removed AVL operations after an unchanged
// current receiver has bootstrapped the same owner. No numeric gap is fitted.
fn removed_directory_cost(program: &ResolvedProgram, function: &ResolvedFunction) -> (usize, usize) {
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(super::super::super::arena::build::<errors::Legacy>(function,
        capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
        &mut preparation).unwrap());
    let storage = validate_slots(function).unwrap(); let leaves = collect_leaves(function).unwrap();
    let ((cost, allocation), refused, _, _) = with_limit_usage_refusal(1 << 26, || measure(|| {
        let mut budget = ReplayBudget::new();
        receive(program, function, &facts, &storage, &leaves, &mut budget, true).unwrap();
        let work = budget.remaining; let bytes = crate::bounded_output::active_remaining().unwrap();
        let mut removed = Directory::<crate::cleanup_plan::ExitTargetId>::new(function, &mut budget,
            "cleanup referenced exit directory").unwrap();
        for block in &function.cleanup_plan.blocks {
            if let CleanupTerminator::Exit(exit) = block.terminator {
                assert!(removed.insert(function, exit, &mut budget).unwrap());
            }
        }
        let cost = (work - budget.remaining, bytes - crate::bounded_output::active_remaining().unwrap());
        drop(removed); drop(budget); cost
    }));
    assert!(!refused && !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed);
    cost
}

#[test]
fn complete_exit_receivers_preserve_prior_owner_repeated_and_competing_six_field_errors() {
    let program = program();
    let mut saved = false;
    let mut repeated = false;
    let mut prior = false;
    let mut future = false;
    let mut forged_id_accepted = false;
    for function in &program.functions {
        let expected = original(&program, function); assert!(expected.is_ok());
        let old = observe(&program, function, false, MAX_REPLAY_WORK_UNITS, 1 << 26, Some(&expected));
        let new = observe(&program, function, true, MAX_REPLAY_WORK_UNITS, 1 << 26, Some(&expected));
        assert!(old.0 && new.0 && !old.3 && !new.3);
        let removed = removed_directory_cost(&program, function);
        assert_eq!(old.1, new.1.checked_add(removed.0).unwrap());
        assert_eq!(old.2, new.2.checked_add(removed.1).unwrap());
        let mut no_exit = function.clone(); no_exit.cleanup_plan.blocks.clear(); no_exit.cleanup_plan.edges.clear();
        let expected_empty = original(&program, &no_exit);
        for current in [false, true] {
            let result = observe(&program, &no_exit, current, MAX_REPLAY_WORK_UNITS, 1 << 26, Some(&expected_empty));
            assert_eq!(result.0, expected_empty.is_ok()); assert!(!result.3);
            let zero = observe(&program, &no_exit, current, MAX_REPLAY_WORK_UNITS, 0, None);
            assert!(!zero.0 && zero.3 && zero.4.is_some());
        }
        if function.cleanup_plan.blocks.iter().any(|b| matches!(b.terminator, CleanupTerminator::Exit(_))) {
            assert!(new.1 < old.1 && new.2 < old.2 && new.5 < old.5); saved = true;
        }
        let mut attacks = Vec::new();
        for (at, block) in function.cleanup_plan.blocks.iter().enumerate() {
            let CleanupTerminator::Exit(exit) = block.terminator else { continue; };
            // Current declared owner is the ordinary success path; the old
            // receiver deliberately never checks ExitTarget.id at this phase.
            let mut bad = function.clone(); bad.cleanup_plan.exits[exit.0 as usize].id = crate::cleanup_plan::ExitTargetId(u32::MAX);
            assert!(original(&program, &bad).is_ok()); forged_id_accepted = true;
            attacks.push(bad);
            let mut bad = function.clone(); bad.cleanup_plan.blocks[at].terminator =
                CleanupTerminator::Exit(crate::cleanup_plan::ExitTargetId(u32::MAX)); attacks.push(bad);
            let mut bad = function.clone(); bad.cleanup_plan.exits[exit.0 as usize].from = BlockId(u32::MAX); attacks.push(bad);
            if at + 1 < function.cleanup_plan.blocks.len() {
                future = true;
                let mut bad = function.clone(); bad.cleanup_plan.exits[exit.0 as usize].from = function.cleanup_plan.blocks[at+1].id; attacks.push(bad);
                // A real prior successful Exit(x) means repeat must outrank the
                // later wrong-owner check even though the inventory count fits.
                repeated = true;
                let mut bad = function.clone(); bad.cleanup_plan.blocks[at+1].terminator = CleanupTerminator::Exit(exit);
                let expected = original(&program, &bad).unwrap_err();
                assert!(expected.message.contains("unknown or repeated exit")); attacks.push(bad);
                let mut bad = function.clone(); bad.cleanup_plan.blocks[at+1].terminator = CleanupTerminator::Exit(exit);
                bad.cleanup_plan.blocks[at+1].id = block.id; attacks.push(bad);
                let mut bad = function.clone(); bad.cleanup_plan.blocks[at].id = BlockId(u32::MAX);
                bad.cleanup_plan.blocks[at+1].terminator = CleanupTerminator::Exit(exit); attacks.push(bad);
            }
            if at > 0 {
                prior = true;
                // Claimed earlier owner with another exit or non-exit term is
                // not a duplicate. Preserve the distinct wrong-owner error.
                let mut bad = function.clone(); bad.cleanup_plan.exits[exit.0 as usize].from = function.cleanup_plan.blocks[at-1].id;
                let expected = original(&program, &bad).unwrap_err();
                assert!(expected.message.contains("owned by the wrong block")); attacks.push(bad);
            }
            // Bounds selection must precede any malformed declared-owner probe.
            let mut bad = function.clone(); bad.cleanup_plan.exits[exit.0 as usize].from = BlockId(u32::MAX);
            bad.cleanup_plan.blocks[at].terminator = CleanupTerminator::Exit(crate::cleanup_plan::ExitTargetId(u32::MAX)); attacks.push(bad);
        }
        for hostile in attacks {
            let expected = original(&program, &hostile);
            for current in [false, true] {
                let result = observe(&program, &hostile, current, MAX_REPLAY_WORK_UNITS, 1 << 26, Some(&expected));
                assert_eq!(result.0, expected.is_ok()); assert!(!result.3);
            }
        }
    }
    assert!(saved && repeated && prior && future && forged_id_accepted);
}

#[test]
fn actual_exit_receivers_keep_full_work_bytes_exact_short_zero_and_sticky_drop() {
    let program = program();
    let function = program.functions.iter().find(|f| !f.cleanup_plan.exits.is_empty()).unwrap();
    for current in [false, true] {
        let full = observe(&program, function, current, MAX_REPLAY_WORK_UNITS, 1 << 26, None);
        assert!(full.0 && !full.3 && full.1 > 0 && full.2 > 0);
        for limit in [0, full.1-1, full.1] {
            let result = observe(&program, function, current, limit, 1 << 26, None);
            assert_eq!(result.0, limit == full.1); assert!(!result.3);
            if result.0 { assert_eq!(result.1, full.1); }
        }
        for limit in [0, full.2-1, full.2] {
            let result = observe(&program, function, current, MAX_REPLAY_WORK_UNITS, limit, None);
            assert_eq!(result.0, limit == full.2); assert_eq!(result.3, limit != full.2);
            assert_eq!(result.4.is_some(), limit != full.2);
            if result.0 { assert_eq!(result.2, full.2); }
        }
    }
    let mut hostile = function.clone();
    let (at, exit) = hostile.cleanup_plan.blocks.iter().enumerate().find_map(|(at, b)| {
        if let CleanupTerminator::Exit(exit) = b.terminator { Some((at, exit)) } else { None }
    }).unwrap();
    hostile.cleanup_plan.exits[exit.0 as usize].from = BlockId(u32::MAX);
    let expected = original(&program, &hostile); assert!(expected.is_err());
    assert_eq!(hostile.cleanup_plan.blocks[at].id.0 as usize, at);
    let full = observe(&program, &hostile, true, MAX_REPLAY_WORK_UNITS, 1 << 26, Some(&expected));
    // Full malformed route includes every new probe/control payment.
    assert!(!full.0 && !full.3 && full.1 >= exit_prefix::PROBE_UNITS && full.2 >= exit_prefix::CONTROL_BYTES);
    let exact_work = observe(&program, &hostile, true, full.1, 1 << 26, Some(&expected));
    assert!(!exact_work.0 && !exact_work.3);
    let exact_bytes = observe(&program, &hostile, true, MAX_REPLAY_WORK_UNITS, full.2, Some(&expected));
    assert!(!exact_bytes.0 && !exact_bytes.3 && exact_bytes.4.is_none());
    for limit in [0, full.1-1] { let result = observe(&program, &hostile, true, limit, 1 << 26, None); assert!(!result.0 && !result.3); }
    for limit in [0, full.2-1] { let result = observe(&program, &hostile, true, MAX_REPLAY_WORK_UNITS, limit, None); assert!(!result.0 && result.3 && result.4.is_some()); }
}

#[test]
fn complete_before_exit_receiver_and_full_source_inverses_remain_exact() {
    use sha2::{Digest, Sha256};
    let old = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/exit-prefix-reference-proof/retained.rs.before.txt"));
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/exit-prefix-reference-proof/exact-source-inverses.json"))).unwrap();
    let fixed = [
        ("tests/hir_module_boundaries.rs", "03fde58554846398c379bbaa6f60cd1368d2573ff9fc715d340074f4c6ea56dc"),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "bb72229bff1d33b7e6f03887a4022003465f077710cd643d75be9afcf4577961"),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", "7e0364a194df13c8fc374e4dd1ae2464609a8040ea2087429d06843a00f417c7"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", "222853f211323dde7b91b7e5ba0dfebf0aeee26a4383174075718c662386bbe7"),
    ];
    let sources = [
        ("tests/hir_module_boundaries.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-prefix-reference-proof/tests__hir_module_boundaries.rs.before.txt"))),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-prefix-reference-proof/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt"))),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-prefix-reference-proof/src__cleanup_plan__replay__live_oracle__source_custody__b.rs.before.txt"))),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-prefix-reference-proof/retained.rs.before.txt"))),
    ];
    for row in rows.as_array().unwrap() {
        let fixed_hash = fixed.iter().find(|(path, _)| *path == row["source"].as_str().unwrap()).unwrap().1;
        assert_eq!(row["sha256"].as_str().unwrap(), fixed_hash);
        let (_, current, archive) = sources.iter().find(|(path, _, _)| *path == row["source"].as_str().unwrap()).unwrap();
        let mut source = (*current).to_owned();
        assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(archive.as_bytes()))), fixed_hash);
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap(); assert_eq!(source.matches(after).count(), 1);
            source = source.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(source, *archive);
    }
    let start = old.find("// The retained key proof").unwrap();
    let end = old.find("\n#[cfg(test)]\nmod tests;").unwrap();
    assert_eq!(include_str!("exit_before.rs"), format!(
        "//! Complete pre-exit-elimination receiving body, independently compiled.\nuse super::*;\n\n{}\n", &old[start..end]));
}
