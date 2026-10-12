use super::*;
fn source(path: &str, text: &str) -> SemanticWorkspaceSource {
    SemanticWorkspaceSource::new(
        path.to_owned(),
        crate::format::canonical(&crate::parse(text, std::path::Path::new(path)).unwrap()),
    )
}
fn paths(paths: &[&str]) -> String {
    render_path_set(&paths.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>()).unwrap()
}
#[test]
fn complete_original_schema_corpus_matches_legacy_manifest_revision_and_independent_replay() {
    let cases = [
            (
                "v10.spx",
                "module schema.v10; @id(\"v10.main\") fn main()->i64{0}",
                "semaprax.graph.v10",
            ),
            (
                "v11.spx",
                "module schema.v11; @id(\"v11.target\") fn target(input:Option<i64>)->Option<bool>{let checked=input?;Option<bool>::Some { value: checked>0 }} @id(\"v11.main\") fn main()->i64{0}",
                "semaprax.graph.v11",
            ),
            (
                "v12.spx",
                "module schema.v12; @id(\"v12.box\") record Box<T>{@id(\"v12.box.value\") value:T,} @id(\"v12.main\") fn main()->i64{0}",
                "semaprax.graph.v12",
            ),
            (
                "v13.spx",
                "module schema.v13; @id(\"v13.box\") record Box{@id(\"v13.box.value\") value:i64,} @id(\"v13.read\") fn read(input:Box)->i64{match input { Box { value } => value, }} @id(\"v13.main\") fn main()->i64{0}",
                "semaprax.graph.v13",
            ),
            (
                "v14.spx",
                "module schema.v14; @id(\"v14.target\") fn target<T>()->bool{true} @id(\"v14.main\") fn main()->i64{if target<i64>(){1}else{0}}",
                "semaprax.graph.v14",
            ),
            (
                "v15.spx",
                "module schema.v15; @id(\"v15.main\") fn main()->i64{let mut n=0;while n<1{n=n+1;n<1}n}",
                "semaprax.graph.v15",
            ),
            (
                "v16.spx",
                "module schema.v16; @id(\"v16.pick\") fn pick(value:i64)->i64{match value { 0 => 1, _ => 2, }} @id(\"v16.main\") fn main()->i64{pick(0)}",
                "semaprax.graph.v16",
            ),
            (
                "v17.spx",
                "module schema.v17; @id(\"v17.length\") fn length(value:borrow Slice<u8>)->usize{byte_len(value)} @id(\"v17.main\") fn main()->i64{0}",
                "semaprax.graph.v17",
            ),
        ];
    let path_set = paths(&cases.iter().map(|(path, _, _)| *path).collect::<Vec<_>>());
    let inputs = || {
        cases
            .iter()
            .map(|(path, text, _)| source(path, text))
            .collect()
    };
    let old = preflight_owned(&path_set, inputs()).unwrap();
    let new = preflight(&path_set, inputs()).unwrap();
    assert_eq!(new.0.path_set, old.path_set);
    assert_eq!(new.0.files, old.files);
    assert_eq!(new.manifest(), old.manifest());
    assert_eq!(new.workspace_revision(), old.workspace_revision());
    validate_preflight_replay(&old).unwrap();
    new.replay().unwrap();
    for (path, _, schema) in cases {
        assert_eq!(
            new.0
                .files
                .iter()
                .find(|file| file.path == path)
                .unwrap()
                .source_graph_schema,
            schema
        );
    }
}
#[test]
fn full_path_frontier_and_late_source_errors_keep_original_diagnostics_and_order() {
    let path_set = paths(&["a.spx", "b.spx"]);
    for case in 0..5 {
        let inputs = || match case {
            0 => vec![source(
                "a.spx",
                "module report.a; @id(\"a\") fn a()->i64{1}",
            )],
            1 => vec![
                source("a.spx", "module report.a; @id(\"a\") fn a()->i64{1}"),
                source("a.spx", "module report.b; @id(\"b\") fn b()->i64{2}"),
            ],
            2 => vec![
                source("../a.spx", "module report.a; @id(\"a\") fn a()->i64{1}"),
                source("b.spx", "module report.b; @id(\"b\") fn b()->i64{2}"),
            ],
            3 => vec![
                source("a.spx", "module report.a; @id(\"a\") fn a()->i64{1}"),
                SemanticWorkspaceSource::new(
                    "b.spx".to_owned(),
                    "module report.b; this is not source".to_owned(),
                ),
            ],
            _ => vec![
                source("a.spx", "module report.a; @id(\"same\") fn a()->i64{1}"),
                source("b.spx", "module report.b; @id(\"same\") fn b()->i64{2}"),
            ],
        };
        let old = preflight_owned(&path_set, inputs())
            .err()
            .expect("old hostile refusal");
        let typed = preflight(&path_set, inputs())
            .err()
            .expect("typed hostile refusal");
        assert!(!typed.has_paid_report());
        assert_eq!(typed.reported().unwrap(), old.as_slice());
        assert_eq!(typed.into_legacy_unfunded(), old);
    }
}
#[test]
fn typed_independent_replay_retains_manifest_source_revision_and_digest_priority() {
    let path_set = paths(&["a.spx", "b.spx"]);
    for mutation in 0..5 {
        let inputs = || {
            vec![
                source("a.spx", "module report.a; @id(\"a\") fn a()->i64{1}"),
                source("b.spx", "module report.b; @id(\"b\") fn b()->i64{2}"),
            ]
        };
        let mut old = preflight_owned(&path_set, inputs()).unwrap();
        let mut new = preflight(&path_set, inputs()).unwrap();
        for receiver in [&mut old, &mut new.0] {
            match mutation {
                0 => {
                    receiver.path_set.pop().unwrap();
                }
                1 => receiver.files[0].bytes += 1,
                2 => receiver.files[0].source_digest.push('0'),
                3 => receiver.manifest.push(' '),
                _ => receiver.workspace_revision.push('0'),
            }
        }
        let expected = validate_preflight_replay(&old).unwrap_err();
        let actual = new.replay().unwrap_err();
        assert_eq!(actual.into_legacy_unfunded(), expected);
    }
}
