//! Selective v18 Borrowed Record-Field Uniqueness contract. Programs without
//! `vec_unique_by` retain their exact prior prelude selection and bytes.
use sha2::{Digest, Sha256};
pub(crate) fn contract_bytes() -> Vec<u8> {
    let old = crate::prelude::string_slots::contract_bytes();
    let mut bytes = std::str::from_utf8(&old)
        .expect("prelude UTF-8")
        .replacen(crate::prelude::SCHEMA_V17, crate::prelude::SCHEMA_V18, 1)
        .into_bytes();
    bytes.extend_from_slice(b"operation core.vec.unique-by.v1:<descriptor> vec_unique_by <T>(borrow:Vec<T>,literal:field)->value:bool\nrule borrowed_record_field_uniqueness.v1 one_literal_field explicit_monomorphic_copy_or_owned_leaf_record complete_layout persistent_field_id keys:String,i64,i32,u8,usize,bool,char no_floating_keys no_Bytes_keys named_available_owner_or_borrow whole_root_call_loan full_key_equality no_digest_authority\nrule borrowed_record_field_uniqueness_runtime ordinal_scratch_u32xlen in_place_heapsort no_row_or_payload_copy no_generation_renewal scratch_released_before_result Vec_code:3_on_scratch_refusal\nrule borrowed_record_field_uniqueness_metadata literal_is_not_runtime_String canonical_length_delimited_v1 independently_replayed_before_use\n");
    bytes
}
pub(crate) fn digest_text() -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(contract_bytes()))
    )
}
