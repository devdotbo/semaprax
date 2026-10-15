//! Exact streamed scoped identities for the private resolver path arena.
//!
//! `scoped_identity` remains the formatting encoder for every other caller and
//! the independent oracle for this writer. This writer emits the identical
//! owner/domain/length/path spelling into one buffer whose exact length is
//! computed with checked arithmetic and charged before allocation. The path
//! region is filled by the caller, back to front, without traversal scratch.

use super::{decimal_digits, FunctionExecutionId};

const DECLARATION: &str = "declaration:";
const EXECUTION: &str = "function-execution:";
const GENERIC_KEY: &str = "semaprax.function-execution.v1:generic:";

/// Exact byte length of `scoped_identity(owner, kind, path)` for any path of
/// `path_len` bytes. `None` means the spelling is not addressable.
pub(in crate::hir) fn scoped_len(
    owner: &FunctionExecutionId,
    kind: &str,
    path_len: usize,
) -> Option<usize> {
    // `:{kind}:{path_len}:{path}` follows the owner spelling.
    let tail = kind
        .len()
        .checked_add(decimal_digits(path_len))?
        .checked_add(path_len)?
        .checked_add(3)?;
    let (marker, owner_len) = match owner {
        FunctionExecutionId::Monomorphic(owner) => (DECLARATION, owner.as_str().len()),
        FunctionExecutionId::Generic(instance) => (EXECUTION, generic_key_len(instance.as_str())?),
    };
    marker
        .len()
        .checked_add(decimal_digits(owner_len))?
        .checked_add(1)?
        .checked_add(owner_len)?
        .checked_add(tail)
}

/// `semaprax.function-execution.v1:generic:{len}:{instance}` without
/// materializing `FunctionExecutionId::identity_key`.
fn generic_key_len(instance: &str) -> Option<usize> {
    GENERIC_KEY
        .len()
        .checked_add(decimal_digits(instance.len()))?
        .checked_add(1)?
        .checked_add(instance.len())
}

/// Emit one scoped identity. `fill_path` receives exactly `path_len` bytes and
/// must write complete UTF-8 components into them.
///
/// The whole identity is charged with the same non-required reservation the
/// formatting encoder uses for its appends; a refusal yields the same empty
/// text that encoder yields, and nothing is allocated. The buffer is allocated
/// once at its exact final length, so `exact_string` has nothing to normalize.
pub(in crate::hir) fn emit(
    owner: &FunctionExecutionId,
    kind: &str,
    path_len: usize,
    fill_path: impl FnOnce(&mut [u8]),
) -> String {
    let Some(total) = scoped_len(owner, kind, path_len) else {
        return refuse_unaddressable();
    };
    if !crate::bounded_output::reserve_active(total) {
        return String::new();
    }
    let mut bytes = vec![0u8; total];
    let mut at = 0;
    match owner {
        FunctionExecutionId::Monomorphic(owner) => {
            let owner = owner.as_str();
            at = put(&mut bytes, at, DECLARATION);
            at = put_decimal(&mut bytes, at, owner.len());
            at = put(&mut bytes, at, ":");
            at = put(&mut bytes, at, owner);
        }
        FunctionExecutionId::Generic(instance) => {
            let instance = instance.as_str();
            let key_len = generic_key_len(instance).expect("checked by scoped_len");
            at = put(&mut bytes, at, EXECUTION);
            at = put_decimal(&mut bytes, at, key_len);
            at = put(&mut bytes, at, ":");
            at = put(&mut bytes, at, GENERIC_KEY);
            at = put_decimal(&mut bytes, at, instance.len());
            at = put(&mut bytes, at, ":");
            at = put(&mut bytes, at, instance);
        }
    }
    at = put(&mut bytes, at, ":");
    at = put(&mut bytes, at, kind);
    at = put(&mut bytes, at, ":");
    at = put_decimal(&mut bytes, at, path_len);
    at = put(&mut bytes, at, ":");
    debug_assert_eq!(at + path_len, total);
    fill_path(&mut bytes[at..]);
    String::from_utf8(bytes).expect("streamed identity writes complete UTF-8 components")
}

/// The formatting encoder would request an unaddressable allocation here.
/// Inside a budget the refusal is sticky; outside one it aborts as that
/// encoder's allocation would, rather than publishing an empty spelling.
pub(in crate::hir) fn refuse_unaddressable() -> String {
    assert!(
        !crate::bounded_output::reserve_active_required(usize::MAX),
        "scoped identity length exceeds the address space"
    );
    String::new()
}

fn put(bytes: &mut [u8], at: usize, text: &str) -> usize {
    let end = at + text.len();
    bytes[at..end].copy_from_slice(text.as_bytes());
    end
}

fn put_decimal(bytes: &mut [u8], at: usize, value: usize) -> usize {
    let end = at + decimal_digits(value);
    put_decimal_back(bytes, end, value);
    end
}

/// Decimal length used by the path arena for its checked index components.
pub(in crate::hir) fn decimal_len(value: usize) -> usize {
    decimal_digits(value)
}

/// Write `value` in decimal so that it ends at `end`; returns its start.
pub(in crate::hir) fn put_decimal_back(bytes: &mut [u8], end: usize, mut value: usize) -> usize {
    let mut at = end;
    loop {
        at -= 1;
        bytes[at] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            return at;
        }
    }
}

#[cfg(test)]
#[path = "streamed_tests.rs"]
mod streamed_tests;
