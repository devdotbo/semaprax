use super::*;
// Ordinary reserve calls are intentionally unchanged: only reserve_legacy
// acquired backing in the complete old ordinary constructor.
pub(super) fn reserve<T>(_values: &mut T, _additional: usize) -> Result<(), Diagnostic> {
    Ok(())
}
pub(super) fn reserve_legacy<T>(
    values: &mut Vec<T>,
    additional: usize,
    message: &'static str,
) -> Result<(), Diagnostic> {
    values
        .try_reserve(additional)
        .map_err(|_| cleanup_error(message))
}
