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
        .ok_or_else(|| {
            crate::bounded_output::refuse_active_required(usize::MAX);
            Refused::Overflow
        })?;
    if crate::bounded_output::active_refused()
        || !crate::bounded_output::reserve_active_required(bytes)
    {
        return Err(Refused::Budget);
    }
    let mut entries = Vec::new();
    if entries.try_reserve_exact(count).is_err() {
        return Err(capacity_refusal(bytes, 0));
    }
    if entries.capacity() != count {
        let excess = entries
            .capacity()
            .saturating_sub(count)
            .checked_mul(std::mem::size_of::<Entry<'_>>());
        return Err(capacity_refusal(bytes, excess.unwrap_or(usize::MAX)));
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

// Allocators expose their actual requested capacity only after allocation.
// Account any excess explicitly, then fail closed even when it fits. An earlier
// excess reservation refusal retains its own first receipt.
fn capacity_refusal(requested: usize, excess: usize) -> Refused {
    if excess != 0 {
        crate::bounded_output::reserve_active_required(excess);
    }
    crate::bounded_output::refuse_active_required(requested);
    Refused::Capacity
}

#[cfg(test)]
pub(in crate::hir) fn exercise_capacity_refusal(requested: usize, excess: usize) -> Refused {
    capacity_refusal(requested, excess)
}
