//! Owning regressions for the ordinary Core-Wasm shared-owner consumer
//! closure. Node cases skip when Node is absent; they are source-authored.
use super::{
    admit_producer, classify, consumer_closure, Crossing, Disposition, Refusal, DOMAIN,
    HOST_SOURCE, LEASE_AWARE, REGISTRY_SOURCE,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const HOSTILE: &str = include_str!("tests/hostile.mjs");
const ORDINARY: &str = include_str!("tests/ordinary.mjs");

const VEC_SOURCE: &str = r#"module shared.owner.vec;
@id("app.main") fn main()->i64 {
 let input=[1u8,2u8,3u8];
 let empty=vec_with_capacity<Bytes>(1usize);
 let first=vec_push<Bytes>(empty,bytes_copy(array_as_slice(input)));
 let reserved=vec_reserve_exact<Bytes>(first,1usize);
 let replaced=vec_set<Bytes>(reserved,0usize,bytes_copy(array_as_slice(input)));
 let second=vec_push<Bytes>(replaced,bytes_copy(array_as_slice(input)));
 let observed=vec_len<Bytes>(second)==2usize && vec_capacity<Bytes>(second)==2usize;
 let cleared=vec_clear<Bytes>(second);
 let lexical=vec_push<Bytes>(cleared,bytes_copy(array_as_slice(input)));
 if observed && vec_len<Bytes>(lexical)==1usize && vec_capacity<Bytes>(lexical)==2usize {29}else{0}
}
"#;

const ITER_SOURCE: &str = r#"module shared.owner.iter;
@id("iter.size") fn size(value:own Bytes)->usize {
 let view=bytes_as_slice(value);byte_len(view)
}
@id("app.main") fn main()->i64 {
 let values=vec_push<Bytes>(vec_with_capacity<Bytes>(2usize),bytes_set(bytes_zeroed(3usize),1usize,7u8));
 let more=vec_push<Bytes>(values,bytes_zeroed(2usize));
 let mut seen=0;
 for own item in vec_into_iter<Bytes>(more) {
  let length=size(item);
  if length==3usize {seen=seen+1;0}else{seen=seen+10;0}
 }
 seen
}
"#;

/// Every `env` import name the ordinary emitters can request.
const ORDINARY_IMPORTS: &[&str] = &[
    "spx_add",
    "spx_sub",
    "spx_mul",
    "spx_div",
    "spx_rem",
    "spx_neg",
    "spx_contract_fail",
    "spx_box_drop",
    "spx_box_drop_v2",
    "spx_box_get",
    "spx_box_get_v2",
    "spx_box_into_inner",
    "spx_box_into_inner_v2",
    "spx_box_new",
    "spx_box_new_v2",
    "spx_bytes_as_slice",
    "spx_bytes_copy",
    "spx_bytes_drop",
    "spx_bytes_get",
    "spx_bytes_set",
    "spx_bytes_set1_or5",
    "spx_bytes_set1_or6_or48",
    "spx_bytes_set5",
    "spx_bytes_zeroed",
    "spx_collection_checked_v2",
    "spx_collection_drop_v2",
    "spx_command_arg_utf8_v1",
    "spx_command_args_len_v1",
    "spx_command_owned_bytes_validate_v1",
    "spx_command_stdin_read_v1",
    "spx_environment_len_v1",
    "spx_environment_name_utf8_v1",
    "spx_environment_value_utf8_v1",
    "spx_file_read_text_v2",
    "spx_filesystem_create_dir_v2",
    "spx_filesystem_list_v2",
    "spx_filesystem_read_v1",
    "spx_filesystem_remove_v2",
    "spx_filesystem_stat_v2",
    "spx_filesystem_write_atomic_checked_v3",
    "spx_filesystem_write_atomic_v2",
    "spx_filesystem_write_new_v1",
    "spx_format_step_v1",
    "spx_https_get_v1",
    "spx_https_post_v1",
    "spx_iter_bytes_drop_v2",
    "spx_iter_bytes_into_v2",
    "spx_iter_bytes_next_v2",
    "spx_iter_record_drop_v3",
    "spx_iter_record_into_v3",
    "spx_iter_record_next_v3",
    "spx_network_close_v1",
    "spx_network_connect_v1",
    "spx_network_recv_v1",
    "spx_network_send_v1",
    "spx_network_settle_v1",
    "spx_network_stream_stdout_v1",
    "spx_network_wait_v1",
    "spx_owned_abort",
    "spx_owned_begin",
    "spx_owned_cancel_result",
    "spx_owned_commit",
    "spx_owned_drop",
    "spx_owned_publish",
    "spx_owned_reserve_result",
    "spx_owned_stage",
    "spx_owned_success",
    "spx_owned_utf8_validate_v1",
    "spx_pg_component_input_encode_v1",
    "spx_pg_component_result_copy_v1",
    "spx_pg_v1_call",
    "spx_pg_v1_input_prepare",
    "spx_pg_v1_open",
    "spx_pg_v1_provider_close",
    "spx_pg_v1_result_export",
    "spx_pg_v1_result_release",
    "spx_pg_v1_scratch_capacity",
    "spx_pg_v1_scratch_ptr",
    "spx_pg_v1_scratch_reserve",
    "spx_pg_v1_value_release",
    "spx_process_run_v1",
    "spx_process_settle_v1",
    "spx_semantic_event",
    "spx_status_record",
    "spx_string_byte_at_v2",
    "spx_string_clone",
    "spx_string_compare_v2",
    "spx_string_concat",
    "spx_string_concat_v1",
    "spx_string_contains",
    "spx_string_contains_v1",
    "spx_string_eq",
    "spx_string_find_v2",
    "spx_string_from_char",
    "spx_string_from_char_v1",
    "spx_string_from_i64",
    "spx_string_from_i64_v1",
    "spx_string_from_str_v2",
    "spx_string_from_usize",
    "spx_string_from_usize_v1",
    "spx_string_from_utf8_v1",
    "spx_string_len",
    "spx_string_len_chars",
    "spx_string_len_chars_v1",
    "spx_string_new",
    "spx_string_slice_v2",
    "spx_string_starts_with",
    "spx_string_starts_with_v1",
    "spx_string_to_i64_v2",
    "spx_string_trim_v2",
    "spx_vec_capacity",
    "spx_vec_capacity_v2",
    "spx_vec_clear",
    "spx_vec_clear_v2",
    "spx_vec_drop",
    "spx_vec_drop_v2",
    "spx_vec_get",
    "spx_vec_get_v2",
    "spx_vec_leaf_clone_at_v1",
    "spx_vec_leaf_field_read_v1",
    "spx_vec_leaf_into_iter_v1",
    "spx_vec_leaf_iter_drop_v1",
    "spx_vec_leaf_iter_next_v1",
    "spx_vec_leaf_new_v1",
    "spx_vec_leaf_push_v1",
    "spx_vec_leaf_replace_v1",
    "spx_vec_leaf_reserve_v1",
    "spx_vec_leaf_sort_v1",
    "spx_vec_len",
    "spx_vec_len_v2",
    "spx_vec_push",
    "spx_vec_push_v2",
    "spx_vec_record_push_v2",
    "spx_vec_reserve_exact",
    "spx_vec_reserve_exact_v2",
    "spx_vec_set",
    "spx_vec_set_v2",
    "spx_vec_sort_by_v1",
    "spx_vec_sort_v3",
    "spx_vec_with_capacity",
    "spx_vec_with_capacity_v2",
];

fn host_import_names() -> BTreeSet<String> {
    let body = HOST_SOURCE
        .split_once("const imports = Object.freeze({")
        .and_then(|(_, rest)| rest.split_once("\n  });"))
        .expect("host import object")
        .0;
    body.lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let name = line.split_once(':')?.0;
            (name.starts_with("spx_")
                && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
            .then(|| name.to_owned())
        })
        .collect()
}

fn module(source: &str) -> Vec<u8> {
    let checked = crate::check(source, "shared-owner.spx").unwrap();
    crate::wasm::emit_module(&checked).unwrap()
}

fn node_available() -> bool {
    Command::new("node").arg("--version").output().is_ok()
}

fn stage(files: &[(&str, &[u8])]) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-wasm-shared-owner-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).unwrap();
    for (name, contents) in [
        ("registry.mjs", REGISTRY_SOURCE.as_bytes()),
        ("host.mjs", HOST_SOURCE.as_bytes()),
    ]
    .into_iter()
    .chain(files.iter().copied())
    {
        std::fs::write(directory.join(name), contents).unwrap();
    }
    directory
}

fn run_node(directory: &Path, script: &str, args: &[&str]) -> String {
    let output = Command::new("node")
        .arg(directory.join(script))
        .args(args)
        .current_dir(directory)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{script} failed\nstdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(directory).unwrap();
    stdout
}

#[test]
fn every_ordinary_import_is_classified_without_an_unknown_default() {
    for name in ORDINARY_IMPORTS {
        match classify("env", name) {
            Ok(_) | Err(Refusal::Crossing(_, _)) => {}
            other => panic!("{name} is unclassified: {other:?}"),
        }
    }
    assert_eq!(
        classify("env", "spx_bytes_partition_v1"),
        Err(Refusal::Unknown("spx_bytes_partition_v1".to_owned()))
    );
    assert_eq!(
        classify("semaprax.private.selected-fused.v1", "spx_bytes_drop"),
        Err(Refusal::Namespace(
            "semaprax.private.selected-fused.v1.spx_bytes_drop".to_owned()
        ))
    );
    for (name, crossing) in [
        ("spx_owned_drop", Crossing::PublicOwnedEnvelope),
        ("spx_pg_v1_value_release", Crossing::Provider),
        ("spx_filesystem_write_new_v1", Crossing::Provider),
        ("spx_collection_checked_v2", Crossing::Collection),
        ("spx_format_step_v1", Crossing::Format),
        ("spx_string_concat_v1", Crossing::StringToolkit),
        ("spx_vec_leaf_replace_v1", Crossing::RecordPayload),
        ("spx_iter_record_next_v3", Crossing::RecordPayload),
        ("spx_box_new_v2", Crossing::BoxPayload),
        ("spx_vec_push", Crossing::LegacyVec),
    ] {
        assert_eq!(
            classify("env", name),
            Err(Refusal::Crossing(name.to_owned(), crossing))
        );
    }
    assert_eq!(classify("env", "spx_add"), Ok(Disposition::CarrierFree));
    assert_eq!(classify("env", "spx_bytes_drop"), Ok(Disposition::LeaseAware));
}

#[test]
fn private_host_implements_exactly_the_lease_aware_consumers() {
    let expected: BTreeSet<String> = LEASE_AWARE.iter().map(|name| (*name).to_owned()).collect();
    assert_eq!(host_import_names(), expected);
    assert_eq!(expected.len(), LEASE_AWARE.len(), "duplicate consumer");
    // No `env` import issues a lease; producer admission stays closed.
    assert!(!HOST_SOURCE.contains("spx_bytes_partition"));
    assert!(REGISTRY_SOURCE.contains(DOMAIN));
}

#[test]
fn public_runtime_keeps_its_token_table_and_never_sees_the_domain() {
    let runtime = super::super::browser_runtime();
    assert!(!runtime.contains(DOMAIN));
    assert!(!runtime.contains("shared-byte-owner"));
    assert!(runtime.contains("const entries = new Map();"));
}

#[test]
fn ordinary_vec_and_iterator_modules_close_but_the_producer_stays_closed() {
    for source in [VEC_SOURCE, ITER_SOURCE] {
        let bytes = module(source);
        let closure = consumer_closure(&bytes).unwrap();
        assert!(closure.lease_aware.iter().any(|name| name == "spx_bytes_drop"
            || name == "spx_vec_drop_v2"
            || name == "spx_iter_bytes_drop_v2"));
        assert_eq!(admit_producer(&closure), Err(Refusal::ProducerClosed));
    }
}

#[test]
fn closure_refuses_provider_and_non_function_crossings() {
    use wasm_encoder::{EntityType, ImportSection, MemoryType, Module, TypeSection, ValType};
    let mut types = TypeSection::new();
    types.ty().function([] as [ValType; 0], [] as [ValType; 0]);
    let mut imports = ImportSection::new();
    imports.import("env", "spx_bytes_drop", EntityType::Function(0));
    imports.import("env", "spx_filesystem_read_v1", EntityType::Function(0));
    let mut provider = Module::new();
    provider.section(&types).section(&imports);
    assert_eq!(
        consumer_closure(&provider.finish()),
        Err(Refusal::Crossing(
            "spx_filesystem_read_v1".to_owned(),
            Crossing::Provider
        ))
    );
    let mut imports = ImportSection::new();
    imports.import(
        "env",
        "memory",
        EntityType::Memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        }),
    );
    let mut memory = Module::new();
    memory.section(&imports);
    assert_eq!(
        consumer_closure(&memory.finish()),
        Err(Refusal::NotFunction("env.memory".to_owned()))
    );
    assert_eq!(consumer_closure(b"\0asm\x02"), Err(Refusal::Malformed));
}

#[test]
fn private_host_runs_ordinary_vec_and_iterator_programs_unchanged() {
    if !node_available() {
        return;
    }
    for (source, expected) in [(VEC_SOURCE, "29"), (ITER_SOURCE, "11")] {
        let bytes = module(source);
        consumer_closure(&bytes).unwrap();
        let directory = stage(&[("ordinary.mjs", ORDINARY.as_bytes()), ("app.wasm", &bytes)]);
        let stdout = run_node(&directory, "ordinary.mjs", &["app.wasm", expected]);
        assert!(stdout.contains("shared-owner-ordinary-ok"), "{stdout}");
    }
}

#[test]
fn shared_owner_hostile_lifecycle_regressions() {
    if !node_available() {
        return;
    }
    let directory = stage(&[("hostile.mjs", HOSTILE.as_bytes())]);
    let stdout = run_node(&directory, "hostile.mjs", &[]);
    assert!(stdout.contains("shared-owner-ok"), "{stdout}");
}
