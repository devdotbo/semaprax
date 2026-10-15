//! Exact ordinary aggregate import conventions, independently checked before
//! an import-only closure receipt is created. No producer admission here.
use wasmparser::{FuncType, ValType};
use ValType::{I32, I64};

pub(super) fn matches(name: &str, ty: &FuncType) -> bool {
    let (params, results): (&[ValType], &[ValType]) = match name {
        "spx_bytes_partition" => (&[I64, I64, I64], &[I64, I32]),
        "spx_add"
        | "spx_sub"
        | "spx_mul"
        | "spx_div"
        | "spx_rem"
        | "spx_string_concat_v1"
        | "spx_string_compare_v2" => (&[I64, I64], &[I64]),
        "spx_neg"
        | "spx_bytes_zeroed"
        | "spx_bytes_copy"
        | "spx_bytes_as_slice"
        | "spx_string_from_i64_v1"
        | "spx_string_from_usize_v1"
        | "spx_string_len_chars_v1" => (&[I64], &[I64]),
        "spx_contract_fail" => (&[I32], &[]),
        "spx_bytes_drop" | "spx_vec_drop_v2" | "spx_box_drop_v2" => (&[I64], &[]),
        "spx_bytes_get" | "spx_string_starts_with_v1" | "spx_string_contains_v1" => {
            (&[I64, I64], &[I32])
        }
        "spx_bytes_set" => (&[I64, I64, I32], &[I64]),
        "spx_bytes_set5" => (&[I64, I64, I32, I32, I32, I32, I32], &[I64]),
        "spx_bytes_set1_or5" | "spx_bytes_set1_or6_or48" => (&[I64, I64, I32, I64, I64], &[I64]),
        "spx_string_from_utf8_v1" | "spx_string_from_str_v2" => (&[I64, I32], &[I32]),
        "spx_string_from_char_v1" => (&[I32], &[I64]),
        "spx_vec_with_capacity_v2" | "spx_box_new_v2" => (&[I32, I64], &[I64]),
        "spx_vec_push_v2" | "spx_vec_get_v2" | "spx_vec_reserve_exact_v2" => {
            (&[I64, I32, I64], &[I64])
        }
        "spx_vec_len_v2"
        | "spx_vec_capacity_v2"
        | "spx_vec_clear_v2"
        | "spx_box_get_v2"
        | "spx_box_into_inner_v2" => (&[I64, I32], &[I64]),
        "spx_vec_set_v2" => (&[I64, I32, I64, I64], &[I64]),
        "spx_vec_record_push_v2" => (&[I64, I32, I64, I64, I64], &[I64]),
        "spx_vec_sort_v3" => (&[I64, I32], &[I64]),
        "spx_vec_sort_by_v1" => (&[I64; 4], &[I64]),
        "spx_iter_bytes_into_v2" | "spx_iter_record_into_v3" => (&[I64, I32], &[I32]),
        "spx_iter_bytes_next_v2" => (&[I64, I64, I32], &[I32]),
        "spx_iter_record_next_v3" => (&[I64, I64, I32, I32, I32, I32, I32], &[I32]),
        "spx_iter_bytes_drop_v2" | "spx_iter_record_drop_v3" => (&[I64, I64], &[]),
        "spx_vec_leaf_new_v1" | "spx_vec_leaf_sort_v1" => (&[I64; 3], &[I64]),
        "spx_vec_leaf_push_v1" => (&[I64; 11], &[I64]),
        "spx_vec_leaf_clone_at_v1" => (&[I64, I64, I64, I64, I32], &[I32]),
        "spx_vec_leaf_replace_v1" => (&[I64; 12], &[I64]),
        "spx_vec_leaf_reserve_v1" => (&[I64; 4], &[I64]),
        "spx_vec_leaf_into_iter_v1" => (&[I64, I64, I64, I32], &[I32]),
        "spx_vec_leaf_iter_next_v1" => (&[I64, I64, I64, I64, I32], &[I32]),
        "spx_vec_leaf_iter_drop_v1" => (&[I64; 4], &[]),
        "spx_vec_leaf_field_read_v1" => (&[I64, I64, I64, I64, I64, I32], &[I32]),
        _ => return false,
    };
    ty.params() == params && ty.results() == results
}
