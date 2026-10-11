use super::*;
use crate::byte_data_capacity::CapacityDiagnostic;
use crate::source_verify::capacity::verify_selected_capacity;
use crate::source_verify::fused_construction::{derive, Derivation};
use crate::source_verify::type_table::TypeTable;

const SOURCE: &str = include_str!("../../../../tests/fixtures/record-traversal-maximum.spx");
const PATH: &str = "fused-construction-candidate.spx";

fn parsed(source: &str) -> Program {
    crate::parse(source, PATH).unwrap()
}

fn has_error(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity.is_error())
}

#[test]
fn original_at_32_rows_stays_on_the_unchanged_legacy_route() {
    let source = SOURCE.replace("4096", "32");
    assert!(crate::check(&source, PATH).is_ok());
    let program = parsed(&source);
    assert!(!has_error(&crate::source_verify::verify(&program)));
    verify_selected_capacity(&program, &TypeTable::new(&program), None).unwrap();
    assert!(matches!(
        check_candidate(&program),
        Err(CandidateRefusal::NoSelection)
    ));
}

#[test]
fn original_4096_passes_every_source_pass_as_an_unvalidated_candidate() {
    // The default route is unchanged.
    let errors = crate::check(SOURCE, PATH).unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-T267"));
    let program = parsed(SOURCE);
    assert!(crate::source_verify::verify(&program)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T267"));
    let types = TypeTable::new(&program);
    let legacy = verify_selected_capacity(&program, &types, None).unwrap_err();
    assert!(matches!(legacy.diagnostic, CapacityDiagnostic::Allocation));

    let candidate = match check_candidate(&program) {
        Ok(candidate) => candidate,
        Err(refusal) => panic!("candidate refused: {refusal:?}"),
    };
    assert_eq!(candidate.status(), CandidateStatus::UnvalidatedUntilHirJoin);
    let proof = candidate.proof();
    assert!(proof.authenticate(&program));
    assert_eq!(
        (proof.rows(), proof.width(), proof.extent()),
        (4096, 3, 12_288)
    );
    assert_eq!(
        proof.producer_site(),
        "app.main.body.s2.body.s0.value.field.2"
    );
    // One fused event and its extent, through the ordinary analysis.
    verify_selected_capacity(&program, &types, Some(proof)).unwrap();
    let status = candidate.status().describe();
    assert!(status.contains("UNVALIDATED"));
    println!("{status}");
}

#[test]
fn forged_proofs_producers_counts_and_functions_refuse() {
    let program = parsed(SOURCE);
    let types = TypeTable::new(&program);
    let Ok(Derivation::Selected(held)) = derive(&program, &program.functions[0]) else {
        panic!("held selection")
    };

    // Same text and fields, foreign AST: equal raw keys select nothing.
    let foreign_program = parsed(SOURCE);
    let Ok(Derivation::Selected(foreign)) =
        derive(&foreign_program, &foreign_program.functions[0])
    else {
        panic!("foreign selection")
    };
    assert_eq!(foreign.raw_join_key(), held.raw_join_key());
    let refused = verify_selected_capacity(&program, &types, Some(&foreign)).unwrap_err();
    assert!(matches!(refused.diagnostic, CapacityDiagnostic::Invariant));
    assert!(has_error(
        &crate::source_verify::declaration::verify_selected(&program, Some(&foreign), None)
    ));

    // Producer, count and loop forgeries derive no selection and keep the
    // default refusal.
    for (from, to) in [
        (
            "payload: bytes_zeroed(3usize)",
            "payload: bytes_zeroed_bounded(3usize, 3usize)",
        ),
        (
            "payload: bytes_zeroed(3usize)",
            "payload: bytes_copy(bytes_zeroed(3usize))",
        ),
        (
            "vec_with_capacity<Row>(4096usize)",
            "vec_with_capacity<Row>(4095usize)",
        ),
        ("next = next + 1", "next = next + 2"),
    ] {
        let source = SOURCE.replace(from, to);
        assert_ne!(source, SOURCE, "hostile mutation must be real: {from}");
        assert!(crate::check(&source, PATH).is_err(), "{from} -> {to}");
        assert!(
            matches!(
                check_candidate(&parsed(&source)),
                Err(CandidateRefusal::NoSelection)
            ),
            "{from} -> {to}"
        );
    }

    // Function forgeries: an effectful function and two selected functions.
    let mut effectful = parsed(SOURCE);
    effectful.functions[0].effects.push("clock.read".to_owned());
    assert!(matches!(
        check_candidate(&effectful),
        Err(CandidateRefusal::NoSelection)
    ));
    let mut doubled = parsed(SOURCE);
    let mut twin = doubled.functions[0].clone();
    twin.name = "build".to_owned();
    twin.stable_id = "app.build".to_owned();
    doubled.functions.push(twin);
    assert!(matches!(
        check_candidate(&doubled),
        Err(CandidateRefusal::Ambiguous)
    ));
}
