//! The historical text sink is an independent, test-only allocation control.
use crate::bounded_output::{with_limit_usage_refusal, CappedString, ReservationStage};
use crate::workspace_graph::{
    build_owned_with_builder_limit, retained_validation, WorkspaceEdge, WorkspaceSource,
    MAX_BUILDER_BYTES,
};
use std::fmt::Write as _;
use std::path::Path;

fn historical_text(edges: &[WorkspaceEdge]) -> String {
    let mut proof = CappedString::new();
    for edge in edges {
        writeln!(
            proof,
            "{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
            edge.caller_path,
            edge.caller,
            edge.target_path,
            edge.target,
            edge.kind,
            edge.site,
            edge.expression,
            edge.ast_path,
            edge.alias,
            edge.ordinal,
        )
        .unwrap();
    }
    proof.into_string()
}

#[test]
fn edge_replay_keeps_exact_projection_and_one_short_without_discarded_text_storage() {
    let sources = vec![
        WorkspaceSource {
            path: "app/main.spx".to_owned(),
            source: "module app.main;\nuse function @id(\"lib.answer\") from lib.core as answer;\n@id(\"app.main\") fn main() -> i64 { answer() + answer() }\n".to_owned(),
        },
        WorkspaceSource {
            path: "lib/core.spx".to_owned(),
            source: "module lib.core;\n@id(\"lib.answer\") fn answer() -> i64 { 21 }\n".to_owned(),
        },
    ];
    let baseline = build_owned_with_builder_limit(sources.clone(), MAX_BUILDER_BYTES).unwrap();
    let programs = sources
        .iter()
        .map(|source| crate::parse(&source.source, Path::new(&source.path)).unwrap())
        .collect::<Vec<_>>();
    let validate = |edges: &[WorkspaceEdge]| {
        retained_validation::validate_retained_facts_compact(
            &programs,
            &baseline.hir.modules,
            edges,
        )
    };
    validate(&baseline.edges).unwrap();
    let (old_text, overflow, old_bytes, _) =
        with_limit_usage_refusal(MAX_BUILDER_BYTES, || historical_text(&baseline.edges));
    assert!(!overflow);
    assert!(old_bytes > 0);
    assert_eq!(old_bytes, old_text.len());
    let (exact, overflow, _, _) =
        with_limit_usage_refusal(old_bytes, || historical_text(&baseline.edges));
    assert!(!overflow);
    assert_eq!(exact, old_text);
    let (_, overflow, _, _) =
        with_limit_usage_refusal(old_bytes - 1, || historical_text(&baseline.edges));
    assert!(
        overflow,
        "the old sink really requests backing, rather than only a forecast"
    );

    let mut low = 0;
    let mut high = MAX_BUILDER_BYTES;
    while low < high {
        let middle = low + (high - low) / 2;
        if build_owned_with_builder_limit(sources.clone(), middle).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let exact = build_owned_with_builder_limit(sources.clone(), low).unwrap();
    assert_eq!(
        exact.edges, baseline.edges,
        "all emitted projection fields and order remain exact"
    );
    assert_eq!(historical_text(&exact.edges), old_text);
    assert_eq!(exact.usage.builder_bytes, baseline.usage.builder_bytes);
    assert_eq!(
        build_owned_with_builder_limit(sources, low - 1)
            .err()
            .unwrap()[0]
            .code,
        "SPX-G171"
    );

    // Real edge-replay scratch still refuses one-short with its sticky stage.
    let (_, overflow, debit, _) =
        with_limit_usage_refusal(MAX_BUILDER_BYTES, || validate(&baseline.edges));
    assert!(!overflow);
    assert!(debit > 0);
    let (result, overflow, _, _) = with_limit_usage_refusal(debit, || validate(&baseline.edges));
    result.unwrap();
    assert!(!overflow);
    let (_, overflow, _, refusal) =
        with_limit_usage_refusal(debit - 1, || validate(&baseline.edges));
    assert!(overflow);
    assert_eq!(refusal.unwrap().stage, Some(ReservationStage::EdgeProof));
    let mut forged = baseline.edges.clone();
    let call = forged.iter_mut().find(|edge| edge.kind == "call").unwrap();
    call.expression.push_str(".forged");
    assert_eq!(validate(&forged).err().unwrap()[0].code, "SPX-G173");
}
