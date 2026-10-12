//! Caller allocation frontier. The pure validator receives only borrowed data.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::hir) enum Refused {
    Overflow,
    Budget,
    Capacity,
}
pub(in crate::hir) fn prepare(program: &ResolvedProgram) -> Result<Directory<'_>, Refused> {
    let count = program.functions.len();
    let bytes = count
        .checked_mul(std::mem::size_of::<Entry<'_>>())
        .ok_or(Refused::Overflow)?;
    if crate::bounded_output::active_refused()
        || !crate::bounded_output::reserve_active_required(bytes)
    {
        return Err(Refused::Budget);
    }
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(count)
        .map_err(|_| Refused::Capacity)?;
    if entries.capacity() != count {
        return Err(Refused::Capacity);
    }
    entries.extend(
        program
            .functions
            .iter()
            .enumerate()
            .map(|(ordinal, function)| Entry { ordinal, function }),
    );
    // Equal IDs retain original ordinal precedence. Allocation-free sorting
    // changes only lookup metadata, never program or canonical vector order.
    entries.sort_unstable_by(|left, right| {
        left.function
            .id
            .cmp(&right.function.id)
            .then(left.ordinal.cmp(&right.ordinal))
    });
    Ok(Directory { program, entries })
}
