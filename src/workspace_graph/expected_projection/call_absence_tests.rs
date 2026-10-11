use super::{collect_expected_edges, collect_expected_edges_impl};
use std::collections::BTreeMap;

#[test]
fn type_only_import_edges_match_the_original_call_walk_with_nested_local_calls() {
    let provider = r#"module provider;
@id("provider.record") record Record { @id("provider.record.n") n:i64, }
@id("provider.read") fn read(n:i64)->i64 {n}
"#;
    let consumer = r#"module consumer;
use type @id("provider.record") from provider as Imported;
@id("consumer.local") fn local(n:i64)->i64 {n+1}
@id("consumer.main") fn main()->i64 {
 let row=Imported{n:local(local(i64_from_usize(byte_len(array_as_slice([1u8,2u8])))))};
 match row {Imported{n}=>n,}
}
"#;
    for source in [
        consumer.to_owned(),
        consumer
            .replace(
                "use type",
                "use function @id(\"provider.read\") from provider as remote;\nuse type",
            )
            .replace("local(local(", "remote(local("),
    ] {
        let programs = [provider, source.as_str()]
            .into_iter()
            .enumerate()
            .map(|(i, s)| {
                crate::parse(
                    s,
                    std::path::Path::new(if i == 0 {
                        "provider.spx"
                    } else {
                        "consumer.spx"
                    }),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let authored = super::super::index_authored(&programs).unwrap();
        let paths = programs
            .iter()
            .map(|p| (p.module.as_str(), p.path.as_str()))
            .collect::<BTreeMap<_, _>>();
        let mut original = Vec::new();
        let mut selected = Vec::new();
        collect_expected_edges_impl::<true>(
            &programs[1],
            &programs,
            &paths,
            &authored,
            &mut original,
        )
        .unwrap();
        collect_expected_edges(&programs[1], &programs, &paths, &authored, &mut selected).unwrap();
        assert_eq!(selected, original);
        // Bind every edge field and vector order to the same serialized bytes.
        let bytes = |edges: &[super::super::WorkspaceEdge]| {
            serde_json::to_vec(
                &edges
                    .iter()
                    .map(|e| {
                        (
                            &e.caller_path,
                            &e.caller,
                            &e.target_path,
                            &e.target,
                            e.kind,
                            e.site,
                            &e.expression,
                            &e.ast_path,
                            &e.alias,
                            e.ordinal,
                        )
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap()
        };
        assert_eq!(bytes(&selected), bytes(&original));
        assert!(selected
            .iter()
            .any(|e| e.kind == "type_reference" && e.target == "provider.record"));
        assert_eq!(
            selected.iter().filter(|e| e.kind == "call").count(),
            usize::from(source.contains("as remote"))
        );
    }
}
