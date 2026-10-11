//! Owning tests use the authenticated held-source context without dropping builtins
//! or manufacturing a declaration index from standalone HIR.
use super::*;
pub(crate) struct SourceRecordReplayFixture(WorkspaceGraphBuild);
impl SourceRecordReplayFixture {
    pub(crate) fn new(source: &str) -> Self {
        let sources = [
            ("flat.spx", source),
            (
                "support.spx",
                "module support.flat;@id(\"support.ready\") fn ready()->i64{0}",
            ),
        ]
        .into_iter()
        .map(|(path, source)| WorkspaceSource {
            path: path.to_owned(),
            source: crate::format::canonical(&crate::parse(source, path).unwrap()),
        })
        .collect();
        Self(
            build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES)
                .unwrap()
                .0,
        )
    }
    fn module(&self) -> &WorkspaceResolvedModule {
        self.0
            .hir
            .modules
            .iter()
            .find(|module| module.path == "flat.spx")
            .unwrap()
    }
    pub(crate) fn functions(&self) -> &[hir::ResolvedFunction] {
        &self.module().functions
    }
    pub(crate) fn function_mut(&mut self, id: &str) -> &mut hir::ResolvedFunction {
        self.0
            .hir
            .modules
            .iter_mut()
            .find(|module| module.path == "flat.spx")
            .unwrap()
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == id)
            .unwrap()
    }
    pub(crate) fn with_context<T>(
        &self,
        apply: impl FnOnce(&dyn crate::cleanup::ReplayContext) -> T,
    ) -> T {
        source_schema::with_test_context(self.module(), &self.0.hir.declarations, apply).unwrap()
    }
}
