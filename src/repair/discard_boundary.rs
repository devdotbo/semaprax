use super::{repair_delta_error, Diagnostic};

pub(super) fn rebase_mismatch() -> Vec<Diagnostic> {
    vec![repair_delta_error(
        "candidate HIR exceeds the selected function identity rebase",
    )]
}
