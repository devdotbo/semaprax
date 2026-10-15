//! Private lookup rows, never canonical HIR/cleanup vectors. A sorted prefix
//! maximum keeps an outer freeze active after a nested same-root freeze ends.
//! These functions lend no admission or payer; the authenticated producer owns
//! and prepays every row, and validates the exact function/region on lookup.
use crate::hir::ValueId;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Kind {
    Rebind,
    HiddenPayloadRead,
}
#[derive(Clone, Copy)]
pub(super) struct Freeze<'a> {
    pub(super) id: &'a ValueId,
    pub(super) kind: Kind,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) prefix_end: usize,
}
pub(super) fn prepare(rows: &mut [Freeze<'_>]) {
    rows.sort_unstable_by(|a, b| {
        a.id.cmp(b.id)
            .then(a.kind.cmp(&b.kind))
            .then(a.start.cmp(&b.start))
    });
    let mut previous: Option<(&ValueId, Kind)> = None;
    let mut end = 0;
    for row in rows {
        let key = (row.id, row.kind);
        if previous != Some(key) {
            end = 0;
        }
        end = end.max(row.end);
        row.prefix_end = end;
        previous = Some(key);
    }
}
pub(super) fn active(rows: &[Freeze<'_>], id: &ValueId, kind: Kind, position: usize) -> bool {
    let next = rows.partition_point(|row| {
        row.id < id
            || (row.id == id && (row.kind < kind || (row.kind == kind && row.start <= position)))
    });
    next.checked_sub(1).is_some_and(|previous| {
        let row = &rows[previous];
        row.id == id && row.kind == kind && position < row.prefix_end
    })
}
