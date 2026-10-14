use super::*;
use crate::ast::{Function, Type};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::{Availability, Binding};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

fn source(declaration: &str) -> String {
    format!("module t; {declaration} @id(\"t.main\") fn main()->i64{{0}}")
}
fn codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}
fn variables(function: &Function) -> HashMap<String, Binding> {
    function
        .params
        .iter()
        .map(|parameter| {
            (
                parameter.name.clone(),
                Binding {
                    contract_local: false,
                    traversal_row: None,
                    ty: parameter.ty.clone(),
                    mode: parameter.mode,
                    availability: Availability::Available,
                    moved_places: HashMap::new(),
                    definitely_partial: HashSet::new(),
                    native_unit_discard: false,
                    mutable: false,
                    active_loans: BTreeSet::new(),
                    borrow_origin: None,
                },
            )
        })
        .collect()
}

#[test]
fn complete_prior_public_four_diagnostics_become_exact_unknown_and_independent_sibling() {
    let program = crate::parse(
        &source("@id(\"t.f\") fn f<T>(b:Missing,c:bool)->i64{b+missing_sibling}"),
        Path::new("public-template-recovery.spx"),
    )
    .unwrap();
    let old = crate::source_verify::declaration::public_signature_before::verify(&program);
    assert_eq!(
        codes(&old),
        ["SPX-T224", "SPX-T220", "SPX-T202", "SPX-T208"]
    );
    // Complete preceding public producer, not a hand-written receipt route.
    for repeated in [
        crate::source_verify::declaration::public_signature_before::verify_presenting(
            &program, None,
        ),
        crate::source_verify::declaration::public_signature_before::verify_selected(
            &program, None, None,
        ),
        crate::source_verify::declaration::public_signature_before::verify_partition(&program),
    ] {
        assert_eq!(repeated, old);
    }
    let current = crate::source_verify::verify(&program);
    assert_eq!(current, vec![old[1].clone(), old[2].clone()]);
}

#[test]
fn known_sibling_generic_profile_refusal_and_all_admitted_validation_stay_independent() {
    let program = crate::parse(
        &source("@id(\"t.f\") fn f<T>(b:Missing,p:Bytes)->i64{b+missing_sibling}"),
        Path::new("public-template-recovery.spx"),
    )
    .unwrap();
    let old = crate::source_verify::declaration::public_signature_before::verify(&program);
    let current = crate::source_verify::verify(&program);
    let known = old
        .iter()
        .find(|diagnostic| diagnostic.code == "SPX-T224" && diagnostic.message.contains("`f.p`"))
        .unwrap();
    assert!(current.contains(known));
    for diagnostic in &old {
        let dependent_profile =
            diagnostic.code == "SPX-T224" && diagnostic.message.contains("`f.b`");
        let dependent_operator = diagnostic.code == "SPX-T208";
        if !dependent_profile && !dependent_operator {
            assert!(
                current.contains(diagnostic),
                "independent full diagnostic lost: {diagnostic:?}"
            );
        }
    }
    for declaration in [
        "@id(\"t.f\") fn f<T>(b:T)->T{b}",
        "@id(\"t.f\") fn f<T,U,V>(b:i64)->i64{b}",
    ] {
        let program = crate::parse(
            &source(declaration),
            Path::new("public-template-recovery.spx"),
        )
        .unwrap();
        assert_eq!(
            crate::source_verify::verify(&program),
            crate::source_verify::declaration::public_signature_before::verify(&program)
        );
    }
}

#[test]
fn borrowed_producer_origin_rejects_foreign_stale_and_malformed_templates_without_allocation() {
    let program = crate::parse(
        &source("@id(\"t.f\") fn f<T>(b:Missing,c:bool)->i64{b+missing_sibling}"),
        Path::new("public-template-recovery.spx"),
    )
    .unwrap();
    let raw = &program.functions[0];
    let current =
        crate::source_verify::declared_type::validation_specialize_function(raw, &[Type::I64])
            .unwrap();
    let foreign = program.clone();
    let copy = current.clone();
    let ((), metrics) = crate::test_allocation_observer::measure(|| {
        let held = origin(&program, raw, &current).unwrap();
        assert!(std::ptr::eq(
            held.template_for(&program, &current).unwrap(),
            raw
        ));
        assert!(held.template_for(&foreign, &current).is_none());
        assert!(held.template_for(&program, &copy).is_none());
        assert!(origin(&program, &foreign.functions[0], &current).is_none());
        assert!(origin(&program, raw, raw).is_none());
    });
    assert!(!metrics.invalid);
    assert_eq!(
        (
            metrics.allocations,
            metrics.reallocations,
            metrics.allocated,
            metrics.freed,
            metrics.live
        ),
        (0, 0, 0, 0, 0)
    );
    for field in 0..4 {
        let mut malformed = program.clone();
        match field {
            0 => malformed.functions[0].type_parameters.clear(),
            1 => malformed.functions[0].type_parameters[0].name = "bad-name".to_owned(),
            2 => {
                let duplicate = malformed.functions[0].type_parameters[0].clone();
                malformed.functions[0].type_parameters.push(duplicate);
            }
            3 => malformed.functions[0].params[0].span.start += 1,
            _ => unreachable!(),
        }
        assert!(origin(&malformed, &malformed.functions[0], &current).is_none());
    }
}

#[test]
fn specialized_public_receiver_keeps_exact_six_field_authentication() {
    let program = crate::parse(
        &source("@id(\"t.f\") fn f<T>(b:Missing,c:bool)->i64{b+missing_sibling}"),
        Path::new("public-template-recovery.spx"),
    )
    .unwrap();
    let raw = &program.functions[0];
    let current =
        crate::source_verify::declared_type::validation_specialize_function(raw, &[Type::I64])
            .unwrap();
    let types = TypeTable::new(&program);
    let functions = program
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect();
    let mut receipt = Vec::new();
    crate::source_verify::declared_type::check_declared_type(
        &program,
        &raw.params[0].ty,
        raw.params[0].span,
        &types,
        &HashSet::from(["T"]),
        &mut receipt,
    );
    assert_eq!(codes(&receipt), ["SPX-T220"]);
    for field in 0..7 {
        let mut diagnostics = receipt.clone();
        match field {
            0 => diagnostics[0].code = "SPX-T001",
            1 => diagnostics[0].severity = crate::diagnostic::Severity::Warning,
            2 => diagnostics[0].message = "forged".into(),
            3 => diagnostics[0].path = Some("foreign.spx".to_owned()),
            4 => diagnostics[0].span = None,
            5 => diagnostics[0].help = Some("forged".to_owned()),
            6 => {}
            _ => unreachable!(),
        }
        let mut bindings = variables(&current);
        let (value, _) = crate::source_verify::iterative::check_expr_with_signature_view(
            &program,
            &current,
            &current.body,
            &mut bindings,
            &functions,
            &types,
            None,
            true,
            &mut diagnostics,
            None,
            None,
            origin(&program, raw, &current),
        );
        assert!(value.is_none());
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T202"
                && diagnostic.message.contains("missing_sibling")));
        assert_eq!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-T208"),
            field != 6
        );
    }
}

#[test]
fn full_production_sources_and_complete_public_predecessors_keep_exact_inverses() {
    use sha2::{Digest, Sha256};
    let inverse: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/public-template-recovery-799/whole-source-inverses.json"
    )))
    .unwrap();
    let sources = [
        (include_str!("../../../declaration.rs"), Some(include_str!("../../public_signature_before.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/public-template-recovery-799/src__source_verify__declaration.rs.before.txt"))),
        (include_str!("../../functions.rs"), Some(include_str!("../public_signature_before.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/public-template-recovery-799/src__source_verify__declaration__functions.rs.before.txt"))),
        (include_str!("../../../iterative.rs"), None, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/public-template-recovery-799/src__source_verify__iterative.rs.before.txt"))),
    ];
    let fixed_original_sha256 = [
        "5c4987a9e83eb0f80c7c3d6211910e3c1f62cb91e60b9cc3b9d8f87c8000351f",
        "f8de677d0ee06872c4ddae8415eb84ca145c8ecda961a5379714451f55e3e5f2",
        "8bc16b846d6ce4b01d9bdbe73c15efca61b3d8c9b1ed13afc233f50ca748c241",
    ];
    for (index, (current, compiled_old, original)) in sources.into_iter().enumerate() {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            fixed_original_sha256[index]
        );
        let record = &inverse["files"][index];
        for (source, key) in [
            (Some(current), "current_splices"),
            (compiled_old, "compiled_old_splices"),
        ] {
            let Some(source) = source else { continue };
            let mut restored = source.to_owned();
            for splice in record[key].as_array().unwrap().iter().rev() {
                let start = splice["start"].as_u64().unwrap() as usize;
                let after = splice["after"].as_str().unwrap();
                assert_eq!(restored.get(start..start + after.len()), Some(after));
                restored.replace_range(
                    start..start + after.len(),
                    splice["before"].as_str().unwrap(),
                );
            }
            assert_eq!(restored, original);
        }
    }
}

#[test]
fn public_error_receipts_and_complete_drop_keep_zero_and_sticky_unpaid_boundary_explicit() {
    let program = crate::parse(
        &source("@id(\"t.f\") fn f<T>(b:Missing,c:bool)->i64{b+missing_sibling}"),
        Path::new("public-template-recovery.spx"),
    )
    .unwrap();
    let expected = crate::source_verify::verify(&program);
    assert_eq!(codes(&expected), ["SPX-T220", "SPX-T202"]);
    for poisoned in [false, true] {
        let (((), metrics), refused, paid, first) =
            crate::bounded_output::with_limit_usage_refusal(0, || {
                if poisoned {
                    assert!(!crate::bounded_output::reserve_active_required(1));
                }
                crate::test_allocation_observer::measure(|| {
                    let current = crate::source_verify::verify(&program);
                    assert_eq!(current, expected);
                    drop(current);
                })
            });
        assert_eq!(refused, poisoned);
        assert_eq!(paid, 0);
        assert_eq!(
            first.map(|receipt| receipt.requested),
            poisoned.then_some(1)
        );
        assert!(!metrics.invalid);
        assert!(
            metrics.allocated > 0,
            "ordinary verifier storage remains unpaid"
        );
        assert_eq!(metrics.allocated, metrics.freed);
        assert_eq!(metrics.live, 0);
    }
}
