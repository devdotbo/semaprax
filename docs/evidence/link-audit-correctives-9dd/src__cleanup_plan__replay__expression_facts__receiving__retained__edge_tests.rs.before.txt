use super::*;
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    let source = include_str!("../../../expression_facts.rs")
        .split_once("r#\"module replay.fact_owner;").unwrap().1
        .split_once("\"#,").unwrap().0;
    crate::hir::resolve(&crate::check(&format!("module replay.fact_owner;{source}"),
        "edge-ownership-proof.spx").unwrap()).unwrap()
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
        edge_before::validate_blocks_and_edges_retained(program, function, facts,
            storage, leaves, None, budget, &statuses)
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

#[test]
fn complete_edge_receivers_preserve_full_diagnostics_order_and_drop() {
    let program = program();
    let mut saved = false;
    let mut branch_attacked = false;
    let mut foreign_reference_attacked = false;
    let mut foreign_block_attacked = false;
    for function in &program.functions {
        let expected = original(&program, function);
        assert!(expected.is_ok());
        let old = observe(&program, function, false, MAX_REPLAY_WORK_UNITS, 1 << 26, Some(&expected));
        let new = observe(&program, function, true, MAX_REPLAY_WORK_UNITS, 1 << 26, Some(&expected));
        assert!(old.0 && new.0 && !old.3 && !new.3);
        if !function.cleanup_plan.edges.is_empty() {
            assert!(new.1 < old.1 && new.2 < old.2 && new.5 < old.5);
            saved = true;
        }
        let mut attacks = Vec::new();
        if !function.cleanup_plan.edges.is_empty() {
            let mut bad = function.clone(); bad.cleanup_plan.edges[0].id = EdgeId(u32::MAX); attacks.push(bad);
            let mut bad = function.clone(); bad.cleanup_plan.edges[0].from = BlockId(u32::MAX); attacks.push(bad);
            if let Some(other) = function.cleanup_plan.blocks.iter().find(|b| b.id != function.cleanup_plan.edges[0].from) {
                let mut bad = function.clone(); bad.cleanup_plan.edges[0].from = other.id; attacks.push(bad);
            }
            let mut bad = function.clone(); bad.cleanup_plan.edges[0].to = BlockId(u32::MAX); attacks.push(bad);
            // Two independent faults retain the first complete original error.
            let mut bad = function.clone(); bad.cleanup_plan.edges[0].id = EdgeId(u32::MAX);
            bad.cleanup_plan.blocks[0].id = BlockId(u32::MAX); attacks.push(bad);
            if let Some((at, _)) = function.cleanup_plan.blocks.iter().enumerate().find(|(_, b)|
                !matches!(b.terminator, CleanupTerminator::Exit(_)))
            {
                let mut bad = function.clone();
                bad.cleanup_plan.blocks[at].terminator = CleanupTerminator::Goto(EdgeId(u32::MAX)); attacks.push(bad);
                // Equal edge inventory count; borrow another existing block's edge.
                if let Some(edge) = function.cleanup_plan.edges.iter().find(|e| e.from != function.cleanup_plan.blocks[at].id) {
                    foreign_reference_attacked = true;
                    let mut bad = function.clone(); bad.cleanup_plan.blocks[at].terminator = CleanupTerminator::Goto(edge.id); attacks.push(bad);
                }
            }
        }
        for (at, block) in function.cleanup_plan.blocks.iter().enumerate() {
            if let CleanupTerminator::Branch(edges) = &block.terminator {
                branch_attacked = true;
                assert_eq!(edges.len(), 2);
                let mut bad = function.clone();
                bad.cleanup_plan.blocks[at].terminator = CleanupTerminator::Branch(vec![edges[0], edges[0]]); attacks.push(bad);
                let mut bad = function.clone();
                bad.cleanup_plan.blocks[at].terminator = CleanupTerminator::Branch(vec![edges[0]]); attacks.push(bad);
            }
            if at > 0 {
                foreign_block_attacked = true;
                let mut bad = function.clone(); bad.cleanup_plan.blocks[at].id = function.cleanup_plan.blocks[at-1].id; attacks.push(bad);
            }
        }
        for hostile in attacks {
            let expected = original(&program, &hostile);
            assert!(expected.is_err());
            for current in [false, true] {
                let result = observe(&program, &hostile, current, MAX_REPLAY_WORK_UNITS, 1 << 26, Some(&expected));
                assert!(!result.0 && !result.3);
            }
        }
    }
    assert!(saved && branch_attacked && foreign_reference_attacked && foreign_block_attacked);
}

#[test]
fn actual_edge_receivers_keep_own_exact_short_zero_work_and_bytes() {
    let program = program();
    let function = program.functions.iter().find(|f| !f.cleanup_plan.edges.is_empty()).unwrap();
    for current in [false, true] {
        let full = observe(&program, function, current, MAX_REPLAY_WORK_UNITS, 1 << 26, None);
        assert!(full.0 && !full.3 && full.1 > 0 && full.2 > 0);
        for limit in [0, full.1-1, full.1] {
            let result = observe(&program, function, current, limit, 1 << 26, None);
            assert_eq!(result.0, limit == full.1);
            assert!(!result.3);
            if result.0 { assert_eq!(result.1, full.1); }
        }
        for limit in [0, full.2-1, full.2] {
            let result = observe(&program, function, current, MAX_REPLAY_WORK_UNITS, limit, None);
            assert_eq!(result.0, limit == full.2);
            assert_eq!(result.3, limit != full.2);
            assert_eq!(result.4.is_some(), limit != full.2);
            if result.0 { assert_eq!(result.2, full.2); }
        }
    }
}

#[test]
fn complete_before_edge_receiver_and_current_full_source_remain_exact() {
    use sha2::{Digest, Sha256};
    let old = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/referenced-edge-ownership-proof/retained.rs.before.txt"));
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/referenced-edge-ownership-proof/exact-source-inverses.json"))).unwrap();
    let fixed = [
        ("tests/hir_module_boundaries.rs", "5be1edee738ee12b426939fbfbc09635229d4c7a3da4f45d3c1074254e0eb0e2"),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "844628e0b1a3f3dafd563a7a9ef3396f513e664dc63e7d70237912736d6558ea"),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", "52fd9fb9d542ec3916b7427610885d934f7a9e0e9987e515feea226a0bbabfdb"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", "02901e6e9fb8922ee4be72dab3899cbafaba25b942a9f62daf0979ea1fb8556d"),
    ];
    for row in rows.as_array().unwrap() {
        let fixed_hash = fixed.iter().find(|(path, _)| *path == row["source"].as_str().unwrap()).unwrap().1;
        assert_eq!(row["sha256"].as_str().unwrap(), fixed_hash);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut source = std::fs::read_to_string(root.join(row["source"].as_str().unwrap())).unwrap();
        let archive = std::fs::read_to_string(root.join("docs/evidence/referenced-edge-ownership-proof")
            .join(row["archive"].as_str().unwrap())).unwrap();
        assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(archive.as_bytes()))), row["sha256"].as_str().unwrap());
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap(); assert_eq!(source.matches(after).count(), 1);
            source = source.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(source, archive);
    }
    let start = old.find("// The retained key proof is the sole additional borrowed argument").unwrap();
    let end = old.find("\n#[cfg(test)]\nmod tests;").unwrap();
    assert_eq!(include_str!("edge_before.rs"), format!(
        "//! Complete pre-elimination receiving body, independently compiled.\nuse super::*;\n\n{}\n", &old[start..end]));
}
