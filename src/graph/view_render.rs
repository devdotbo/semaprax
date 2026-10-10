//! Canonical view rendering; private selection is never serialized as authority.
use super::*;
impl GraphView<'_> {
    pub(super) fn static_text(&self) -> bool {
        match self {
            Self::Module { static_text } | Self::Context { static_text, .. } => *static_text,
        }
    }
}
pub(super) fn view_json(view: &GraphView<'_>) -> String {
    match view {
        GraphView::Module { .. } => "{\"kind\":\"module\"}".to_owned(),
        GraphView::Context {
            root,
            depth,
            frontier,
            ..
        } => format!(
            "{{\"kind\":\"context\",\"root\":{},\"depth\":{depth},\"truncated\":{},\"frontier\":[{}]}}",
            quote_json(root.as_str()),
            !frontier.is_empty(),
            frontier
                .iter()
                .map(|id| quote_json(id.as_str()))
                .collect::<Vec<_>>()
                .budgeted_join(",")
        ),
    }
}
