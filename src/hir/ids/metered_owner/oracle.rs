//! Exact full monomorphic/generic scoped owner receiving probe. The original
//! guarded monomorphic receiver remains independent in the parent module.
use super::*;

fn scoped<E>(
    actual: &str,
    owner: &FunctionExecutionId,
    kind: &str,
    path: &str,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    charge(1)?;
    let mut output = Compare {
        actual: actual.as_bytes(),
        charge,
        error: None,
    };
    let formatted = match owner {
        FunctionExecutionId::Monomorphic(owner) => write!(
            output,
            "declaration:{}:{}:{kind}:{}:{path}",
            owner.as_str().len(),
            owner,
            path.len()
        ),
        FunctionExecutionId::Generic(owner) => {
            const PREFIX: &str = "semaprax.function-execution.v1:generic:";
            let Some(length) = PREFIX
                .len()
                .checked_add(decimal_digits(owner.as_str().len()))
                .and_then(|length| length.checked_add(1))
                .and_then(|length| length.checked_add(owner.as_str().len()))
            else {
                return Ok(false);
            };
            write!(
                output,
                "function-execution:{length}:{PREFIX}{}:{}:{kind}:{}:{path}",
                owner.as_str().len(),
                owner,
                path.len()
            )
        }
    };
    if let Some(error) = output.error {
        return Err(error);
    }
    (output.charge)(1)?;
    Ok(formatted.is_ok() && output.actual.is_empty())
}
pub(crate) fn matches<E>(
    function: &crate::hir::ResolvedFunction,
    owner: &FunctionExecutionId,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    charge(1)?;
    let mut fingerprint = 0xcbf29ce484222325u64;
    let mut bytes = function.result_id.0.bytes();
    loop {
        charge(1)?;
        let Some(byte) = bytes.next() else { break };
        fingerprint = (fingerprint ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    charge(1)?;
    if function.result_id.1 != fingerprint {
        return Ok(false);
    }
    if !scoped(
        function.result_id.as_str(),
        owner,
        "value:result",
        "",
        charge,
    )? {
        return Ok(false);
    }
    scoped(
        function.body.id.as_str(),
        owner,
        "expression",
        "body",
        charge,
    )
}
