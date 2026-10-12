//! Caller-owned paid report route for retained source schema views.
//!
//! Each actual caller names one fixed outward form. Selected replay acquires
//! it as a paid carrier on its own ledger before preparation and transfers it
//! on refusal. Fingerprint, sidecar, builder-structure and schema-selection
//! refusals remain legacy owned Vecs and are not funded by this route. A
//! refusal with no paid carrier for the requested form fails closed to the
//! typed `SelectedReplayUnpaid` boundary; only its named `into_unfunded`
//! allocates, at the outward compatibility edge of these view wrappers.
use super::*;
use crate::cleanup_plan::{SelectedReplayRefusal, SelectedReplayReportForm, SelectedReplayUnpaid};

/// Typed view refusal. `Legacy` is an existing owned report; `Replay` is
/// either a paid carrier with its retained ledger or an unpaid inline refusal.
#[allow(
    clippy::large_enum_variant,
    reason = "the affine paid report and ledger move inline; refusal must not allocate a fresh Box"
)]
pub(crate) enum ViewRefusal {
    Legacy(Vec<Diagnostic>),
    Replay(SelectedReplayRefusal),
}
impl From<Vec<Diagnostic>> for ViewRefusal {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Legacy(diagnostics)
    }
}
impl ViewRefusal {
    /// Pass-through form. A paid replay carrier and an existing legacy Vec
    /// move out unchanged; an unpaid replay refusal stays typed.
    pub(crate) fn into_published(self) -> Result<Vec<Diagnostic>, SelectedReplayUnpaid> {
        match self {
            Self::Legacy(diagnostics) => Ok(diagnostics),
            Self::Replay(refusal) => refusal.into_published(),
        }
    }
    /// Pass-through compatibility boundary for callers that never remapped.
    /// Only the named unpaid boundary allocates its literal, unfunded.
    pub(crate) fn into_legacy(self) -> Vec<Diagnostic> {
        self.into_published()
            .unwrap_or_else(SelectedReplayUnpaid::into_unfunded)
    }
    /// Remapping callers publish exactly `form`. Only a paid replay carrier
    /// for that form moves out; every other refusal is typed unpaid. A
    /// legacy Vec is dropped rather than relabelled as the requested form.
    pub(crate) fn into_report(
        self,
        form: SelectedReplayReportForm,
    ) -> Result<Vec<Diagnostic>, SelectedReplayUnpaid> {
        match self {
            Self::Legacy(diagnostics) => {
                drop(diagnostics);
                Err(SelectedReplayUnpaid::legacy(form))
            }
            Self::Replay(refusal) => refusal.into_report(form),
        }
    }
}

/// Exact authoritative sources held by the receiving caller for the same
/// generation it built. They select nothing by themselves: a retained scalar
/// lexical inventory is admitted only when every current typed member replays
/// under the revision recomputed here from these bytes. Duplicate or missing
/// paths refuse.
pub(crate) trait ReceivingSources {
    fn source_count(&self) -> usize;
    fn source_at(&self, index: usize) -> Option<(&str, &str)>;
}
impl ReceivingSources for Vec<WorkspaceSource> {
    fn source_count(&self) -> usize {
        self.len()
    }
    fn source_at(&self, index: usize) -> Option<(&str, &str)> {
        self.get(index)
            .map(|source| (source.path.as_str(), source.source.as_str()))
    }
}

impl WorkspaceGraphBuild {
    /// Change view for a caller that publishes one fixed `form` on refusal.
    /// The unpaid case is the explicit, allocating legacy edge.
    pub(crate) fn into_change_view_reporting(
        self,
        form: SelectedReplayReportForm,
    ) -> Result<WorkspaceGraphChangeView, Vec<Diagnostic>> {
        self.into_change_view_paid(form).map_err(|refusal| {
            refusal
                .into_report(form)
                .unwrap_or_else(SelectedReplayUnpaid::into_unfunded)
        })
    }

    /// Operation view for a caller that publishes one fixed `form` on refusal.
    /// The unpaid case is the explicit, allocating legacy edge.
    pub(crate) fn into_operation_view_reporting(
        self,
        form: SelectedReplayReportForm,
    ) -> Result<WorkspaceGraphOperationView, Vec<Diagnostic>> {
        self.into_operation_view_paid(form).map_err(|refusal| {
            refusal
                .into_report(form)
                .unwrap_or_else(SelectedReplayUnpaid::into_unfunded)
        })
    }

    pub(crate) fn into_change_view_paid(
        self,
        form: SelectedReplayReportForm,
    ) -> Result<WorkspaceGraphChangeView, ViewRefusal> {
        let mut source_schemas =
            source_schema::all(&self.hir.modules, &self.hir.declarations, form)?;
        let mut fingerprints = self.change_fingerprints.ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "workspace change fingerprint sidecar is absent",
            )]
        })?;
        let modules = self
            .hir
            .modules
            .into_iter()
            .zip(source_schemas.drain(..))
            .map(|(module, source_graph_schema)| {
                Ok(WorkspaceGraphChangeModule {
                    path: module.path,
                    module: module.module,
                    source_graph_schema,
                    permits: module.permits,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let declarations = self
            .hir
            .declarations
            .into_iter()
            .map(|(id, declaration)| {
                let semantic_fingerprint =
                    if declaration.origin == hir::IdentityOrigin::CompilerOwned {
                        String::new()
                    } else {
                        fingerprints.remove(&id).ok_or_else(|| {
                            vec![graph_error(
                                "SPX-G173",
                                "workspace change declaration fingerprint is absent",
                            )]
                        })?
                    };
                Ok(WorkspaceGraphChangeDeclaration {
                    id,
                    kind: declaration.kind,
                    origin: declaration.origin,
                    owner: declaration.owner,
                    path: declaration.path,
                    module: declaration.module,
                    semantic_fingerprint,
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        if !fingerprints.is_empty() {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace change fingerprints contain unknown declarations",
            )]
            .into());
        }
        Ok(WorkspaceGraphChangeView {
            modules,
            declarations,
            edges: self.edges,
            dependency_depths: self.hir.dependency_depths,
            shared_prelude_ids: self.hir.shared_prelude_ids.into_iter().collect(),
            usage: self.usage,
        })
    }

    pub(crate) fn into_operation_view_paid(
        mut self,
        form: SelectedReplayReportForm,
    ) -> Result<WorkspaceGraphOperationView, ViewRefusal> {
        let sidecar = self.operation_sidecar.take().ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "workspace operations AST sidecar is absent",
            )]
        })?;
        let builder_bytes = self.operation_builder_bytes;
        let change_builder_bytes = self.change_builder_bytes;
        let graph = self.into_change_view_paid(form)?;
        Ok(WorkspaceGraphOperationView {
            graph,
            sidecar,
            builder_bytes,
            change_builder_bytes,
        })
    }

    #[cfg(test)]
    pub(crate) fn source_graph_schemas_paid(
        &self,
        form: SelectedReplayReportForm,
    ) -> Result<BTreeMap<String, &'static str>, ViewRefusal> {
        self.source_graph_schemas_receiving_paid(form, None)
    }

    /// The same schema view. With the receiving caller's own authoritative
    /// sources, a retained closed scalar lexical inventory may be received;
    /// without them every module keeps its existing route and refusal.
    pub(crate) fn source_graph_schemas_receiving_paid(
        &self,
        form: SelectedReplayReportForm,
        sources: Option<&dyn ReceivingSources>,
    ) -> Result<BTreeMap<String, &'static str>, ViewRefusal> {
        let mut schemas = BTreeMap::new();
        let mut proofs = source_schema::Proofs::new(&self.hir.modules, &self.hir.declarations);
        for module in &self.hir.modules {
            let schema = source_schema::schema_receiving_paid(
                module,
                &self.hir.modules,
                &self.hir.declarations,
                &mut proofs,
                form,
                sources,
            )?;
            if schemas.insert(module.path.clone(), schema).is_some() {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "workspace module paths disagree while deriving source Graph schemas",
                )]
                .into());
            }
        }
        if schemas.len() != self.usage.managed_files {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace source Graph schema facts disagree with managed files",
            )]
            .into());
        }
        Ok(schemas)
    }
}
