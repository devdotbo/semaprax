//! Selective fixed String-slot view contract; prior programs keep prior bytes.
use sha2::{Digest, Sha256};
pub(crate) fn contract_bytes() -> Vec<u8> {
    let old = crate::prelude::static_text::contract_bytes();
    let mut bytes = std::str::from_utf8(&old)
        .expect("prelude UTF-8")
        .replacen(crate::prelude::SCHEMA_V16, crate::prelude::SCHEMA_V17, 1)
        .into_bytes();
    bytes.extend_from_slice(b"operation core.vec.str-at vec_str_at source_type_arguments:0 hir_type_arguments:[String] (borrow:Vec<String>,value:usize)->borrow:str\noperation core.vec.str-at-bytes compiler_only str_as_bytes(vec_str_at) (borrow:Vec<String>,value:usize)->borrow:Slice<u8>\nrule string_slot whole_named_available_owner_or_borrow exact_builtin_Vec_String current_index_identity whole_owner_freeze scoped_no_escape no_clone no_allocation Vec_code:2 source_inner_call_identity ordinary_String_extent separate_Slice_bound\n");
    bytes
}
pub(crate) fn digest_text() -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(contract_bytes()))
    )
}
