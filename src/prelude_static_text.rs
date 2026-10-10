//! Selective literal-root contract: old programs keep their prior prelude bytes.
use sha2::{Digest, Sha256};
pub(crate) fn contract_bytes() -> Vec<u8> {
    let old = crate::prelude::sort_keys::contract_bytes();
    let mut bytes = std::str::from_utf8(&old)
        .expect("prelude UTF-8")
        .replacen(crate::prelude::SCHEMA_V15, crate::prelude::SCHEMA_V16, 1)
        .into_bytes();
    bytes.extend_from_slice(b"rule static_borrowed_utf8.v1 explicit_adjacent_str_literal decoded_pool_bytes:65536 exact_source_expression_module_revision immutable_nonowning_root local_alias_synchronous_borrow_only no_borrow_result_or_storage no_foreign_input_discount no_runtime_authority
");
    bytes
}
pub(crate) fn digest_text() -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(contract_bytes()))
    )
}
