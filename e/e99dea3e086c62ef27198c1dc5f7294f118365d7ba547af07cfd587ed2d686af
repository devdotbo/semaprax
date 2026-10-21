//! Complete immediate-pre-directory proof and immutable-header DATA controls.
use super::*;
use crate::test_allocation_observer;
use sha2::{Digest, Sha256};

fn parse(source: &str) -> Program {
    crate::parse(source, "ordinary.spx").unwrap()
}
fn padding(n: usize) -> String {
    (0..n)
        .map(|i| format!("@id(\"ordinary.pad{i}\") fn pad{i}()->i64{{0}} "))
        .collect()
}
fn flat(headers: usize, groups: usize, reverse: bool) -> String {
    assert!(headers >= 2);
    let mut functions: Vec<_> = (0..headers - 2)
        .map(|i| format!("@id(\"ordinary.f{i}\") fn f{i}()->i64{{{i}}} "))
        .collect();
    if reverse {
        functions.reverse();
    }
    let params = (0..16)
        .map(|i| format!("a{i}:i64"))
        .collect::<Vec<_>>()
        .join(",");
    let args = (0..16)
        .map(|i| {
            if headers == 2 {
                "0".to_owned()
            } else {
                format!("f{}()", i % (headers - 2))
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    let statements = format!("sink({args});").repeat(groups);
    format!(
        "module ordinary; {} @id(\"ordinary.sink\") fn sink({params})->i64{{a0}} @id(\"ordinary.main\") fn main()->i64{{{}0}}",
        functions.join(""),
        statements,
    )
}
fn parity(program: &Program, index: &Index<'_>, imports: bool) -> (Proof, Census) {
    let old = super::super::before_directory::count(program, index, imports);
    take_census();
    let current = count(program, index, imports);
    let census = take_census();
    assert_eq!(
        current.roots, old.roots,
        "complete pre-directory root census"
    );
    (current, census)
}

#[test]
fn complete_before_source_and_all_header_capacity_boundaries_are_retained() {
    let before = include_str!("call_carriers_before_directory.rs");
    assert_eq!(
        format!("{:x}", crate::digest_hex::LowerHex(&Sha256::digest(before))),
        "87142dc1b3f98f9d381459f0f4f8925c807d28645aa0646ea453eea3c63cc512",
    );
    let root = include_str!("call_carriers.rs");
    let linear = root.split("\n#[cfg(test)]").next().unwrap();
    assert_eq!(linear.matches("fn count_linear(").count(), 1);
    assert_eq!(
        linear.replacen("fn count_linear(", "pub(super) fn count(", 1),
        before
    );
    for n in [0, 1, 63, 64, 128, 129, 512, 513] {
        let program = parse(&format!("module ordinary; {}", padding(n)));
        let programs = [program];
        let index = crate::workspace_graph::index_authored(&programs).unwrap();
        let (current, census) = parity(&programs[0], &index, true);
        assert_eq!(current.roots, 0);
        if (64..=512).contains(&n) {
            assert_eq!((census.builds, census.rows), (1, n));
            assert!(census.sort_compares > 0);
        } else {
            let old = super::super::before_directory::count(&programs[0], &index, true);
            assert_eq!(current.bytes, old.bytes + EMPTY_SELECTOR);
            assert_eq!((census.builds, census.rows, census.queries), (0, 0, 0));
        }
    }
}

#[test]
fn sorted_multiset_keeps_every_actual_borrow_duplicate_and_order_independent_member() {
    for reverse in [false, true] {
        let program = parse(&flat(128, 1, reverse));
        let mut proof = Proof::default();
        let mut directory = Directory::<128> {
            rows: [Header::EMPTY; 128],
            len: 0,
            last: None,
        };
        directory.populate(&program, true, &mut proof);
        assert_eq!(directory.len, program.functions.len());
        directory.sort(&mut proof);
        assert!(directory.rows.windows(2).all(|p| p[0].name <= p[1].name));
        for function in &program.functions {
            let matches: Vec<_> = directory
                .rows
                .iter()
                .filter(|r| r.name == function.name)
                .collect();
            assert_eq!(matches.len(), 1);
            assert!(
                matches!(&matches[0].source, Source::Function(found) if std::ptr::eq(*found, function))
            );
        }
        assert!(!std::mem::needs_drop::<Directory<'_, 128>>());
        assert_eq!(
            size_of::<Directory<'_, 128>>(),
            128 * size_of::<Header<'_>>() + size_of::<usize>() + size_of::<Option<usize>>()
        );
        assert_eq!(
            size_of::<Directory<'_, 512>>(),
            512 * size_of::<Header<'_>>() + size_of::<usize>() + size_of::<Option<usize>>()
        );
        let programs = [program];
        let index = crate::workspace_graph::index_authored(&programs).unwrap();
        let ((current, census), allocation) =
            test_allocation_observer::measure(|| parity(&programs[0], &index, true));
        assert_eq!(current.roots, 16);
        assert_eq!(census.queries, 16);
        assert_eq!(census.rows, 128);
        assert!(census.query_compares > 0);
        assert!(!allocation.invalid, "{allocation:?}");
        assert_eq!(
            (
                allocation.allocations,
                allocation.allocated,
                allocation.freed,
                allocation.live
            ),
            (0, 0, 0, 0)
        );
    }
}

#[test]
fn indexed_live_header_hostility_and_disabled_imports_match_complete_old_proof() {
    let provider = parse("module provider; @id(\"provider.f\") fn f()->i64{1}");
    let consumer = parse(&format!(
        "module ordinary; use function @id(\"provider.f\") from provider as f; {} @id(\"ordinary.main\") fn main()->i64{{f()}}",
        padding(63),
    ));
    let programs = [provider, consumer];
    let index = crate::workspace_graph::index_authored(&programs).unwrap();
    assert_eq!(parity(&programs[1], &index, true).0.roots, 1);
    assert_eq!(parity(&programs[1], &index, false).0.roots, 0);
    for mutation in 0..7 {
        let mut candidate = programs[1].clone();
        match mutation {
            0 => candidate.module_uses[0].target_module = "foreign".into(),
            1 => candidate.module_uses[0].persistent_id = "absent".into(),
            2 => candidate.module_uses.push(candidate.module_uses[0].clone()),
            3 => candidate.module_uses[0].kind = ModuleUseKind::Type,
            4 => {
                let mut function = programs[0].functions[0].clone();
                function.stable_id = "ordinary.local".into();
                candidate.functions.push(function);
            }
            5 => {
                candidate.functions.last_mut().unwrap().body =
                    parse("module ordinary; @id(\"temporary\") fn main()->i64{f(1)}")
                        .functions
                        .remove(0)
                        .body
            }
            _ => {
                candidate.functions.last_mut().unwrap().body =
                    parse("module ordinary; @id(\"temporary\") fn main()->i64{let f = 1; f()}")
                        .functions
                        .remove(0)
                        .body
            }
        }
        assert_eq!(
            parity(&candidate, &index, true).0.roots,
            0,
            "mutation {mutation}"
        );
    }
    for body in [
        "f<i64>()",
        "missing()",
        "let x = f(); x",
        "let x = fn()->i64{f()}; x()",
    ] {
        let candidate = parse(&format!(
            "module ordinary; {} @id(\"ordinary.f\") fn f()->i64{{1}} @id(\"ordinary.main\") fn main()->i64{{{body}}}",
            padding(62)
        ));
        assert_eq!(parity(&candidate, &index, true).0.roots, 0, "{body}");
    }
    let candidate = parse(&format!(
        "module ordinary; {} @id(\"ordinary.f\") fn f<T>()->i64{{1}} @id(\"ordinary.main\") fn main()->i64{{f()}}",
        padding(62)
    ));
    assert_eq!(parity(&candidate, &index, true).0.roots, 0);
    let candidate = parse(&format!(
        "module ordinary; {} @id(\"ordinary.f\") fn f()->i64{{1}} @id(\"ordinary.host\") interface Host permits {{}} {{ @id(\"host.f\") import rust fn f()->i64 effects {{}} failure infallible; }} @id(\"ordinary.main\") fn main()->i64{{f()}}",
        padding(62)
    ));
    assert_eq!(parity(&candidate, &index, true).0.roots, 0);
}

#[test]
fn full_cold_invoice_retains_short_losses_and_genuine_dense_net_gate() {
    for (headers, groups) in [(2, 1), (64, 0), (128, 0), (512, 128)] {
        let mut source = flat(headers, groups, false);
        if headers == 512 {
            for i in 1..16 {
                source = source.replace(&format!("f{i}()"), "f0()");
            }
        }
        let program = parse(&source);
        let programs = [program];
        let index = crate::workspace_graph::index_authored(&programs).unwrap();
        let old = super::super::before_directory::count(&programs[0], &index, true);
        let (current, census) = parity(&programs[0], &index, true);
        if headers == 2 {
            assert_eq!(current.bytes, old.bytes + EMPTY_SELECTOR);
        } else if groups == 0 {
            assert!(
                current.bytes > old.bytes,
                "cold directory regression must remain visible"
            );
            assert_eq!(census.queries, 0);
        } else {
            assert!(census.last_hits > 0);
            assert_eq!((current.roots, census.queries), (16 * groups, 16 * groups));
            assert!(
                current.bytes < old.bytes,
                "full typed cold/sort/query invoice must beat the complete old scan: old={} current={} census={census:?}",
                old.bytes,
                current.bytes
            );
        }
    }
}

#[test]
fn current_exact_short_zero_and_source_drift_keep_real_forecast_order_and_drop() {
    use crate::workspace_graph::expected_projection::cached_exact::{self, Retention};
    let program = parse(&flat(64, 1, false));
    let programs = [program];
    let index = crate::workspace_graph::index_authored(&programs).unwrap();
    let current = parity(&programs[0], &index, true).0;
    assert_eq!(current.roots, 16);
    for retention in [Retention::Parsed, Retention::Semantic] {
        let expected = cached_exact::summed(&programs, &index, retention).unwrap();
        struct Restore(usize);
        impl Drop for Restore {
            fn drop(&mut self) {
                crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|v| v.set(self.0));
            }
        }
        let _restore =
            Restore(crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|v| v.replace(expected.1)));
        assert_eq!(
            cached_exact::summed(&programs, &index, retention).unwrap(),
            expected
        );
        for limit in [expected.1 - 1, 0] {
            crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|v| v.set(limit));
            let first = cached_exact::summed(&programs, &index, retention).unwrap_err();
            let second = cached_exact::summed(&programs, &index, retention).unwrap_err();
            assert_eq!(
                first, second,
                "all six diagnostic fields, sticky repeated refusal"
            );
            assert!(first.iter().all(|d| d.code == "SPX-G171"));
        }
    }
    // The directory never survives count(): new source bytes and live headers
    // are read again; no successful classification or provider verdict is cached.
    let mut changed = programs[0].clone();
    changed.functions[0].name = "renamed".into();
    assert_eq!(parity(&changed, &index, true).0.roots, 15);
    assert_eq!(parity(&programs[0], &index, true).0.roots, 16);
}

#[test]
fn complete_pre898_forecast_comparison_discloses_every_cold_and_dense_row() {
    use crate::workspace_graph::expected_projection::{
        cached_exact::Retention, call_carriers_original, dependency_identity_max, held_projection,
    };
    for (headers, groups, warm) in [
        (2, 1, false),
        (64, 0, false),
        (128, 1, false),
        (512, 128, false),
        (64, 128, true),
    ] {
        let mut source = flat(headers, groups, false);
        if warm {
            for i in 1..16 {
                source = source.replace(&format!("f{i}()"), "f0()");
            }
        }
        let programs = [parse(&source)];
        let index = crate::workspace_graph::index_authored(&programs).unwrap();
        let program = &programs[0];
        let maximum = Some(dependency_identity_max(program, &index, &programs).unwrap());
        let before_directory = super::super::before_directory::count(program, &index, true);
        let current_proof = count(program, &index, true);
        for retention in [Retention::Parsed, Retention::Semantic] {
            let old = call_carriers_original::synthetic_builder_bytes_scoped_with_retention_with_registry(
                program, &index, &programs, maximum, true, (5, retention), None,
            ).unwrap();
            let current =
                held_projection::synthetic_builder_bytes_scoped_with_retention_with_registry(
                    program,
                    &index,
                    &programs,
                    maximum,
                    true,
                    (5, retention),
                    None,
                )
                .unwrap();
            let copies = match retention {
                Retention::Parsed => 1,
                Retention::Semantic => 2,
            };
            let auxiliary = size_of::<crate::hir::ResolvedBinding>()
                + size_of::<crate::hir::ResolvedStatement>()
                + size_of::<crate::hir::ResolvedFieldInitializer>()
                + size_of::<crate::hir::ResolvedMatchArm>()
                + size_of::<crate::hir::ResolvedMatchPatternField>()
                + size_of::<crate::hir::ResolvedRecordMatchPatternField>();
            assert_eq!(
                current.retained_clone_and_hir + copies * current_proof.roots * auxiliary,
                old.retained_clone_and_hir + current_proof.bytes
            );
            if warm {
                assert!(
                    current.retained_clone_and_hir < old.retained_clone_and_hir,
                    "adoption gate includes the complete pre898 forecast and every current cold/query fee: old={} current={}",
                    old.retained_clone_and_hir,
                    current.retained_clone_and_hir
                );
            }
            // This complete pre898 comparison is separate from beating the
            // immediate linear proof. Negative rows remain explicit; neither
            // test supplies application adoption authority or cap changes.
            eprintln!(
                "AUTHORED_HEADER_DIRECTORY headers={headers} groups={groups} warm={warm} copies={copies} roots={} pre898={} immediate_linear={} current={} old_proof={} current_proof={} auxiliary={auxiliary}",
                current_proof.roots,
                old.retained_clone_and_hir,
                current.retained_clone_and_hir - current_proof.bytes + before_directory.bytes,
                current.retained_clone_and_hir,
                before_directory.bytes,
                current_proof.bytes
            );
        }
    }
}

#[test]
fn warm_raw_position_rechecks_arity_multiplicity_and_live_provider_header() {
    let provider = parse("module provider; @id(\"provider.f\") fn f()->i64{1}");
    let consumer = parse(&format!(
        "module ordinary; use function @id(\"provider.f\") from provider as f; {} @id(\"ordinary.main\") fn main()->i64{{f() + f(1)}}",
        padding(63)
    ));
    let programs = [provider, consumer];
    let mut index = crate::workspace_graph::index_authored(&programs).unwrap();
    assert_eq!(parity(&programs[1], &index, true).0.roots, 1);
    let mut directory = Directory::<128> {
        rows: [Header::EMPTY; 128],
        len: 0,
        last: None,
    };
    let mut proof = Proof::default();
    directory.populate(&programs[1], true, &mut proof);
    directory.sort(&mut proof);
    take_census();
    assert!(directory.target(&index, "f", 0, &mut proof));
    assert!(!directory.target(&index, "f", 1, &mut proof));
    assert_eq!(take_census().last_hits, 1);
    index.remove("provider.f");
    assert!(
        !directory.target(&index, "f", 0, &mut proof),
        "warm raw position grants no provider authority"
    );
    assert_eq!(take_census().last_hits, 1);
    let mut duplicate = programs[1].clone();
    duplicate.module_uses.push(duplicate.module_uses[0].clone());
    assert_eq!(parity(&duplicate, &index, true).0.roots, 0);
}
