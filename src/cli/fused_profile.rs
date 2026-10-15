//! Shared strict syntax for the explicit source-only allocation account.
pub(super) fn account(value: &str) -> Result<semaprax::fused_source::RuntimeAccount, u8> {
    if value.is_empty()
        || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        eprintln!("--managed-bytes requires a canonical positive decimal integer");
        return Err(2);
    }
    let bytes = value.parse::<usize>().map_err(|_| {
        eprintln!("--managed-bytes is outside the finite profile account range");
        2
    })?;
    semaprax::fused_source::RuntimeAccount::new(bytes).map_err(|error| {
        eprintln!("{}", error.message);
        2
    })
}
