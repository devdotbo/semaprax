//! Exact legacy type-fact JSON; relocated verbatim to keep the root budget.
use super::{quote_json, TypeFacts};

pub(super) fn facts_object(facts: &TypeFacts) -> String {
    format!(
        "{{\"copy\":{},\"contains_resource\":{},\"sized\":{},\"needs_drop\":{},\"layout_key\":{}}}",
        facts.copy,
        facts.contains_resource,
        facts.sized,
        facts.needs_drop,
        quote_json(&facts.layout_key)
    )
}
