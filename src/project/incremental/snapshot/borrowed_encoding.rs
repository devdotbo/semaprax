//! Exact legacy wire from borrowed compiler-held state, without a snapshot clone.
use super::*;
use crate::cache_codec::{Codec, Decoder, Encoder};

pub(super) struct SnapshotView<'a> {
    pub(super) cache: &'a ProjectFrontendCache,
    pub(super) built: &'a ProjectFrontendBuild,
}
impl Codec for SnapshotView<'_> {
    fn encode(&self, out: &mut Encoder) -> Result<()> {
        out.nested(|out| {
            out.text(&self.cache.context)?;
            out.text(self.built.revision().project_revision())?;
            out.text(self.built.revision().workspace_revision())?;
            out.text(self.built.revision().semantic_graph())?;
            out.nested(|out| {
                out.length(self.cache.entries.len())?;
                for (path, entry) in &self.cache.entries {
                    let checked = &self.cache.checked[path];
                    out.nested(|out| {
                        path.encode(out)?;
                        entry.input().source.encode(out)?;
                        checked.synthetic.encode(out)?;
                        checked.resolved.encode(out)?;
                        checked.resolver_bytes.encode(out)?;
                        checked.retained_loan_bytes.encode(out)
                    })?;
                }
                Ok(())
            })
        })
    }
    fn decode(_: &mut Decoder<'_>) -> Result<Self> {
        Err(invalid(
            "borrowed snapshot encoder is not a decoded cache authority",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_snapshot_matches_independent_owned_wire_and_full_warm_replay() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        let manifest =
            ProjectManifest::parse(&std::fs::read_to_string(root.join("semaprax.toml")).unwrap())
                .unwrap();
        let sources = manifest
            .sources()
            .iter()
            .map(|path| {
                ProjectFrontendSource::new(path, &std::fs::read_to_string(root.join(path)).unwrap())
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let mut cache = ProjectFrontendCache::new_with_semantic_cache();
        let cold = cache.build(&manifest, &sources).unwrap();
        let warm = cache.build(&manifest, &sources).unwrap();
        assert_eq!(
            cold.revision().semantic_graph(),
            warm.revision().semantic_graph()
        );
        require_warm(&warm).unwrap();
        // Saved old encoder input: deep clones are outside the borrowed pass.
        let old = Snapshot {
            context: cache.context.clone(),
            project_revision: warm.revision().project_revision().to_owned(),
            workspace_revision: warm.revision().workspace_revision().to_owned(),
            graph_json: warm.revision().semantic_graph().to_owned(),
            entries: cache
                .entries
                .iter()
                .map(|(path, entry)| {
                    let checked = &cache.checked[path];
                    Entry {
                        path: path.clone(),
                        source: entry.input().source.clone(),
                        synthetic: checked.synthetic.materialize(),
                        resolved: checked.resolved.clone(),
                        resolver_bytes: checked.resolver_bytes,
                        retained_loan_bytes: checked.retained_loan_bytes,
                    }
                })
                .collect(),
        };
        let expected = crate::cache_codec::encode(&old).unwrap();
        let actual = crate::cache_codec::encode(&SnapshotView {
            cache: &cache,
            built: &warm,
        })
        .unwrap();
        assert_eq!(actual, expected);
        let mut restored = decode_snapshot(&actual).unwrap();
        let restored = restored.build(&manifest, &sources).unwrap();
        require_warm(&restored).unwrap();
        assert_eq!(
            restored.revision().semantic_graph(),
            cold.revision().semantic_graph()
        );
    }
}
