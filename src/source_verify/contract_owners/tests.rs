use std::path::Path;

const LOCAL: &str = r#"module contracts.fresh;
@id("inspect") fn inspect(payload:own Bytes)->bool {byte_len(bytes_as_slice(payload))==2usize}
@id("main") fn main()->i64
 requires {let required=bytes_zeroed(2usize);let moved=required;inspect(moved)}
 ensures {let checked="ok";let moved=checked;string_len(moved)==2}
 ensures result==29 {29}
"#;
fn errors(source: &str) -> Vec<crate::diagnostic::Diagnostic> {
    crate::check(source, Path::new("fresh-contract.spx")).unwrap_err()
}
#[test]
fn fresh_local_aliases_keep_ordinary_consuming_calls_and_canonical_roundtrip() {
    let ast = crate::check(LOCAL, "fresh-contract.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let replay = crate::check(&canonical, "fresh-contract.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&replay));
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
}
#[test]
fn preexisting_owner_and_provisional_result_transfers_stay_refused() {
    for source in [
        r#"module contracts.refused;
@id("take") fn take(input:own Bytes)->i64 requires {let stolen=input;true}{0}
@id("main") fn main()->i64{0}"#,
        r#"module contracts.refused;
@id("make") fn make()->Bytes ensures {let stolen=result;true}{bytes_zeroed(1usize)}
@id("main") fn main()->i64{0}"#,
    ] {
        assert!(errors(source).iter().any(|error| error.code == "SPX-O105"));
    }
}
#[test]
fn local_alias_transfer_is_single_use_and_general_owned_call_results_remain_closed() {
    let duplicate = LOCAL.replace("inspect(moved)", "inspect(moved) && inspect(moved)");
    assert!(errors(&duplicate)
        .iter()
        .any(|error| error.code == "SPX-O101"));
    let factory = LOCAL
        .replace(
            "@id(\"inspect\")",
            "@id(\"factory\") fn factory(n:usize)->Bytes {bytes_zeroed(2usize)}\n@id(\"inspect\")",
        )
        .replace(
            "let required=bytes_zeroed(2usize)",
            "let required=factory(2usize)",
        );
    assert!(errors(&factory)
        .iter()
        .any(|error| error.code == "SPX-O105"));
}
#[test]
fn freshness_cannot_cross_contract_clauses_or_owning_closure_capture() {
    let stale = LOCAL.replace(
        "ensures result==29",
        "ensures byte_len(bytes_as_slice(required))==2usize\n ensures result==29",
    );
    assert!(errors(&stale).iter().any(|error| error.code == "SPX-T202"));
    let capture = LOCAL.replace(
        "let moved=required;inspect(moved)",
        "let captured=own fn()->bool {inspect(required)};true",
    );
    assert!(errors(&capture)
        .iter()
        .any(|error| error.code == "SPX-O119"));
}
fn budget_source(before: usize, after: usize, payload: u64, extra: bool) -> String {
    let block = |count: usize| {
        let mut text = String::from("{");
        for i in 0..count {
            text.push_str(&format!("let local{i}=bytes_zeroed({payload}usize);"));
        }
        text.push_str("true}");
        text
    };
    format!(
        "module contracts.budget; @id(\"main\") fn main()->i64 requires {} ensures {} {} {{0}}",
        block(before),
        block(after),
        if extra {
            "ensures {let overflow=bytes_zeroed(1usize);true}"
        } else {
            ""
        }
    )
}
#[test]
fn contract_body_and_ensures_allocation_events_are_cumulative_without_refunds() {
    let source =
        budget_source(15, 16, 1, false).replace("{0}", "{let body=bytes_zeroed(1usize);0}");
    crate::check(&source, "events-exact.spx").unwrap();
    let excess = source.replace("ensures {", "ensures {let overflow=bytes_zeroed(1usize);");
    assert!(errors(&excess).iter().any(|error| error.code == "SPX-T267"));
}
#[test]
fn cumulative_payload_exact_and_one_more_byte_keep_two_mib_limit() {
    crate::check(&budget_source(8, 8, 131072, false), "payload-exact.spx").unwrap();
    assert!(errors(&budget_source(8, 8, 131072, true))
        .iter()
        .any(|error| error.code == "SPX-T267"));
}
