//! Actual checked source -> module -> private host invocation, never witness substitution.
use super::*;
use super::super::tests::{node_available, run_node, stage, ORDINARY};

const SOURCE: &str = r#"module partition_wasm;
@id("partition_wasm.split") fn split()->Vec<Bytes>{
 let buffer=bytes_zeroed(65536usize);
 bytes_partition(buffer,4096usize,16usize)
}
@id("partition_wasm.main") fn main()->i64{
 let values=split();
 if vec_len<Bytes>(values)==4096usize {7}else{0}
}
"#;

fn emitted(source: &str) -> PartitionModule {
    let ast = crate::parse(source, "partition_wasm.spx").unwrap();
    let owner = crate::hir::byte_partition::ValidatedPartitionProgram::compile_source(
        &ast, &mut usize::MAX).unwrap();
    assert!(crate::check(source, "partition_wasm.spx").is_err());
    let module = PartitionModule::emit(&owner, &mut usize::MAX).unwrap();
    assert!(consumer_closure(module.bytes(), Host::PrivateOrdinary).is_err());
    assert!(module.closure.lease_aware.iter().any(|name| name == "spx_bytes_partition"));
    assert_eq!(admit_producer(&module.closure), Err(Refusal::ProducerClosed));
    module
}

#[test]
fn checked_source_partition_emits_real_4096_vec_and_consuming_iterator_with_settlement() {
    let iterator = SOURCE.replace(
        "if vec_len<Bytes>(values)==4096usize {7}else{0}",
        "let mut seen=0; for own item in vec_into_iter<Bytes>(values) { let view=bytes_as_slice(item); if byte_len(view)==16usize {seen=seen+1;0}else{seen=0;0} } seen",
    );
    for (source, expected) in [(SOURCE, "7"), (iterator.as_str(), "4096")] {
        let module = emitted(source);
        if node_available() {
            let directory = stage(&[("ordinary.mjs", ORDINARY.as_bytes()), ("app.wasm", module.bytes())]);
            assert!(run_node(&directory, "ordinary.mjs", &["app.wasm", expected, "plain"])
                .contains("shared-owner-ordinary-ok"));
        }
    }
}

#[test]
fn actual_partition_failure_codes_keep_owner_live_until_canonical_cleanup_and_reentry() {
    for (source, code) in [
        (SOURCE.replace("4096usize,16usize", "4096usize,0usize"), 1),
        (SOURCE.replace("4096usize,16usize", "18446744073709551615usize,2usize"), 2),
        (SOURCE.replace("4096usize,16usize", "4095usize,16usize"), 3),
        (SOURCE.replace("4096usize,16usize", "65536usize,1usize"), 4),
    ] {
        let module = emitted(&source);
        if node_available() {
            let expected = format!("status:{}", 0x7000 + code);
            let directory = stage(&[("ordinary.mjs", ORDINARY.as_bytes()), ("app.wasm", module.bytes())]);
            assert!(run_node(&directory, "ordinary.mjs", &["app.wasm", &expected, "plain"])
                .contains("shared-owner-ordinary-ok"));
        }
    }
}
