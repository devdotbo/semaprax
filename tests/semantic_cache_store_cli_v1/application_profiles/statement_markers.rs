//! Persisted syntax-specific witnesses, using the real CLI store and replay.
//! Source-byte limits are distinct from private compiler work/backing budgets.
use super::{envelope_hostile, same_product, value, write, Fixture};
use semaprax::{ast, check, format, graph, project, verify};

const PATH: &str = "src/app.spx";
const TESTS: &str = r#"module calculator.tests;
use function @id("calculator.app.main") from calculator.app as run;
@id("calculator.tests.main") fn main() -> i64 { run() }
"#;

fn source(body: &str) -> String {
    format!("module calculator.app; @id(\"calculator.app.main\") fn main()->i64 {{ {body} }}")
}

fn fixture(text: &str) -> Fixture {
    let fixture = Fixture::new();
    write(&fixture, PATH, text);
    write(&fixture, "src/tests.spx", TESTS);
    fixture
}

fn canonical(text: &str) -> String {
    let parsed = check(text, PATH).unwrap();
    let canonical = format::canonical(&parsed);
    let round = check(&canonical, PATH).unwrap();
    assert_eq!(canonical, format::canonical(&round));
    assert_eq!(
        graph::to_json(&parsed).unwrap(),
        graph::to_json(&round).unwrap()
    );
    canonical
}

fn run(fixture: &Fixture, expected: i64) {
    project::with_authenticated_project(&fixture.root.join("semaprax.toml"), |snapshot| {
        assert!(
            matches!(snapshot.execute_test(semaprax::interpreter::InterpreterOptions::default()),
            semaprax::interpreter::InterpreterOutcome::Returned(value) if value == expected)
        );
        Ok(())
    })
    .unwrap();
}

fn warm(fixture: &Fixture, digest: &str) -> serde_json::Value {
    let cold = value(fixture.cold_open());
    assert_eq!(cold["frontend_work"]["work"]["modules_resolved"], 3);
    let loaded = value(fixture.load(digest));
    assert_eq!(loaded["work"]["modules_resolved"], 0);
    assert_eq!(loaded["work"]["checked_HIR_reused"], 3);
    let report = value(fixture.warm_open(digest));
    same_product(&cold, &report);
    assert_eq!(report["frontend_work"]["work"]["modules_resolved"], 0);
    assert_eq!(report["frontend_work"]["work"]["checked_HIR_reused"], 3);
    assert_eq!(report["frontend_work"]["manifest_context_reset"], false);
    assert_eq!(
        report["frontend_work"]["invalidated_sources"],
        serde_json::json!([])
    );
    report
}

fn stale(fixture: &Fixture, digest: &str, text: &str, expected: i64) {
    write(fixture, PATH, text);
    let report = value(fixture.warm_open(digest));
    same_product(&value(fixture.cold_open()), &report);
    assert!(report["frontend_work"]["invalidated_sources"]
        .as_array()
        .unwrap()
        .iter()
        .any(|path| path == PATH));
    assert!(
        report["frontend_work"]["work"]["modules_resolved"]
            .as_u64()
            .unwrap()
            > 0
    );
    run(fixture, expected);
}

fn refused(fixture: &Fixture, digest: &str, text: &str, code: &str) {
    let errors = check(text, PATH).unwrap_err();
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
    // Preserve the actual invalid authored bytes; canonical formatting cannot
    // authenticate a forged marker or turn a value boundary into a guard.
    std::fs::write(fixture.root.join(PATH), text).unwrap();
    for output in [
        fixture.cold_open(),
        fixture.warm_open(digest),
        fixture.persist_output(),
    ] {
        assert!(!output.status.success(), "{text}");
    }
    // The immutable old entry remains independently readable, never authority
    // for this changed current source.
    assert_eq!(value(fixture.load(digest))["work"]["modules_resolved"], 0);
}

fn source_storage_boundary(text: &str) {
    use project::{ProjectFrontendSource, MAX_PROJECT_FRONTEND_CACHE_SOURCE_BYTES};
    let mut exact = canonical(text);
    exact.push_str("\n//");
    exact.extend(std::iter::repeat_n(
        ' ',
        MAX_PROJECT_FRONTEND_CACHE_SOURCE_BYTES - exact.len(),
    ));
    assert_eq!(exact.len(), MAX_PROJECT_FRONTEND_CACHE_SOURCE_BYTES);
    let proposal = ProjectFrontendSource::new(PATH, &exact).unwrap();
    assert_eq!(proposal.source(), exact);
    exact.push(' ');
    let errors = ProjectFrontendSource::new(PATH, &exact)
        .err()
        .expect("one excess source byte");
    assert!(
        errors.iter().any(|error| error.code == "SPX-G256"),
        "{errors:?}"
    );
    // Proposal construction alone carries no compiler/cache authority.
}

fn persist(fixture: &Fixture) -> String {
    assert_eq!(fixture.initialize()["source_authority"], false);
    let receipt = fixture.persist();
    assert_eq!(receipt["source_authority"], false);
    let bytes = receipt["payload_bytes"].as_u64().unwrap();
    assert!(
        bytes > 0
            && bytes <= semaprax::semantic_cache_store::MAX_SEMANTIC_CACHE_PAYLOAD_BYTES as u64
    );
    receipt["entry_digest"].as_str().unwrap().to_owned()
}

#[test]
fn scalar_discard_marker_persisted_reuse_keeps_source_shape_and_storage_guards() {
    let text = source("let _if1=3; false && 1/0>0; 5; _if1+4");
    let text = canonical(&text);
    let mut parsed = check(&text, PATH).unwrap();
    let ast::ExprKind::Block { statements, .. } = &mut parsed.functions[0].body.kind else {
        panic!("block")
    };
    assert_eq!(
        statements
            .iter()
            .filter(|statement| matches!(
                statement,
                ast::Statement::Let {
                    syntax: ast::LetSyntax::ExpressionStatement,
                    ..
                }
            ))
            .count(),
        2
    );
    let ast::Statement::Let { mutable, .. } = &mut statements[1] else {
        panic!("discard")
    };
    *mutable = true;
    assert!(verify::verify(&parsed)
        .iter()
        .any(|error| error.code == "SPX-T315"));
    assert!(format::canonical(&parsed).contains("let mut"));

    let fixture = fixture(&text);
    run(&fixture, 7);
    let digest = persist(&fixture);
    let original = warm(&fixture, &digest);
    envelope_hostile(&fixture, &digest);
    let explicit = source("let _if1=3; let skipped=false && 1/0>0; let ignored=5; _if1+4");
    stale(&fixture, &digest, &explicit, 7);
    let changed = value(fixture.cold_open());
    assert_ne!(original["project_revision"], changed["project_revision"]);
    for body in [
        "\"owned\"; 7",
        "bytes_zeroed(1usize); 7",
        "let text=\"x\"; string_as_str(text); 7",
    ] {
        refused(&fixture, &digest, &source(body), "SPX-T315");
    }
    refused(&fixture, &digest, &source("5;"), "SPX-P106");
    write(&fixture, PATH, &text);
    assert_eq!(
        warm(&fixture, &digest)["project_revision"],
        original["project_revision"]
    );
    run(&fixture, 7);
    source_storage_boundary(&text);
}

#[test]
fn terminated_guard_marker_persisted_reuse_keeps_lexical_shape_and_storage_guards() {
    let body = "let mut i=0; let mut total=0; while i<3 { i=i+1; if i==2 {continue;} else {0}; total=total+i; 0 } total";
    let text = canonical(&source(body));
    let parsed = check(&text, PATH).unwrap();
    let ast::ExprKind::Block { statements, .. } = &parsed.functions[0].body.kind else {
        panic!("block")
    };
    let ast::Statement::While {
        body: loop_body, ..
    } = &statements[2]
    else {
        panic!("while")
    };
    let ast::ExprKind::Block { statements, .. } = &loop_body.kind else {
        panic!("loop block")
    };
    assert!(matches!(
        &statements[1],
        ast::Statement::Let {
            syntax: ast::LetSyntax::StatementIf(_),
            ..
        }
    ));
    let mut forged = parsed.clone();
    let ast::ExprKind::Block { statements, .. } = &mut forged.functions[0].body.kind else {
        unreachable!()
    };
    let ast::Statement::While { body, .. } = &mut statements[2] else {
        unreachable!()
    };
    let ast::ExprKind::Block { statements, .. } = &mut body.kind else {
        unreachable!()
    };
    let ast::Statement::Let { syntax, .. } = &mut statements[1] else {
        unreachable!()
    };
    *syntax = ast::LetSyntax::ExpressionStatement;
    assert!(verify::verify(&forged)
        .iter()
        .any(|error| error.code == "SPX-T312"));

    let fixture = fixture(&text);
    run(&fixture, 4);
    let digest = persist(&fixture);
    let original = warm(&fixture, &digest);
    envelope_hostile(&fixture, &digest);
    // Canonical projection erases the optional guard semicolon. It is the
    // same normalized source generation, not a spelling-based cache lease.
    let control = source(&body.replace("else {0};", "else {0}"));
    assert_eq!(canonical(&control), text);
    write(&fixture, PATH, &control);
    assert_eq!(
        warm(&fixture, &digest)["project_revision"],
        original["project_revision"]
    );
    run(&fixture, 4);
    stale(
        &fixture,
        &digest,
        &source(&body.replace("total+i", "total+i+1")),
        6,
    );
    refused(&fixture, &digest, &source("let mut i=0; while i<3 { i=i+1; let guard=if i==2 {continue;} else {0}; i=i+guard; 0 } i"), "SPX-T312");
    refused(
        &fixture,
        &digest,
        &source("if true {continue;} else {0}; 7"),
        "SPX-T313",
    );
    write(&fixture, PATH, &text);
    warm(&fixture, &digest);
    run(&fixture, 4);
    source_storage_boundary(&text);
}
