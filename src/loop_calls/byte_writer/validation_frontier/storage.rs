use super::*;
pub(super) fn reserve<T>(count: usize) -> Result<Vec<T>, Diagnostic> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(error)?;
    if !crate::bounded_output::reserve_active_required(std::mem::size_of::<Vec<T>>())
        || !crate::bounded_output::reserve_active_required(bytes)
    {
        return Err(error());
    }
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| {
        crate::bounded_output::refuse_active_required(bytes);
        error()
    })?;
    if values.capacity() != count {
        if values.capacity() > count {
            let excess = (values.capacity() - count)
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(error)?;
            let _ = crate::bounded_output::reserve_active_required(excess);
        }
        crate::bounded_output::refuse_active_required(
            values.capacity().saturating_mul(std::mem::size_of::<T>()),
        );
        return Err(error());
    }
    Ok(values)
}

/// Positive proof storage has the original Boolean refusal carrier. A rejected
/// replacement cannot become a new census diagnostic or a retry allowance.
pub(super) fn remember<'p>(
    memo: &mut Vec<&'p ResolvedFunction>,
    target: &'p ResolvedFunction,
) -> bool {
    if crate::bounded_output::active_refused() {
        return false;
    }
    if memo.len() == memo.capacity() {
        let Some(count) = memo.capacity().checked_mul(2).map(|n| n.max(1)) else {
            crate::bounded_output::refuse_active_required(usize::MAX);
            return false;
        };
        let Some(bytes) = count.checked_mul(std::mem::size_of::<&ResolvedFunction>()) else {
            crate::bounded_output::refuse_active_required(usize::MAX);
            return false;
        };
        if !crate::bounded_output::reserve_active_required(std::mem::size_of::<
            Vec<&ResolvedFunction>,
        >()) || !crate::bounded_output::reserve_active_required(bytes)
        {
            return false;
        }
        let mut replacement = Vec::new();
        if replacement.try_reserve_exact(count).is_err() {
            crate::bounded_output::refuse_active_required(bytes);
            return false;
        }
        if replacement.capacity() != count {
            if replacement.capacity() > count {
                let _ = crate::bounded_output::reserve_active_required(
                    (replacement.capacity() - count)
                        .saturating_mul(std::mem::size_of::<&ResolvedFunction>()),
                );
            }
            crate::bounded_output::refuse_active_required(
                replacement
                    .capacity()
                    .saturating_mul(std::mem::size_of::<&ResolvedFunction>()),
            );
            return false;
        }
        replacement.extend_from_slice(memo);
        *memo = replacement;
    }
    memo.push(target);
    true
}
