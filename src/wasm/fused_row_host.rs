//! Private host-boundary probe, not a source emitter or a public byte ABI.
//! The module reexports imports to exercise their actual Core-Wasm signatures.
use wasm_encoder::{
    EntityType, ExportKind, ExportSection, ImportSection, Module, TypeSection, ValType,
};
const NAMESPACE: &str = "semaprax.private.fused-row-host.v1";
const NAMES: [&str; 12] = [
    "start",
    "issue",
    "publish",
    "complete",
    "borrow",
    "read",
    "end_borrow",
    "move_owner",
    "drop_owner",
    "abort",
    "settle_abort",
    "phase",
];
fn probe_module(namespace: &str) -> Vec<u8> {
    use ValType::{I32, I64};
    let issuer = ValType::Ref(wasm_encoder::RefType::EXTERNREF);
    let signatures: [(Vec<ValType>, ValType); 12] = [
        (vec![issuer], I32),
        (vec![issuer, I32], I64),
        (vec![issuer, I32, I64], I32),
        (vec![issuer], I32),
        (vec![issuer, I32, I64, I32], I64),
        (vec![issuer, I32, I64, I32], I32),
        (vec![issuer, I32, I64], I32),
        (vec![issuer, I32, I64], I64),
        (vec![issuer, I32, I64], I32),
        (vec![issuer], I32),
        (vec![issuer], I32),
        (vec![issuer], I32),
    ];
    let mut types = TypeSection::new();
    let mut imports = ImportSection::new();
    let mut exports = ExportSection::new();
    for (index, (params, result)) in signatures.into_iter().enumerate() {
        types.ty().function(params, [result]);
        imports.import(namespace, NAMES[index], EntityType::Function(index as u32));
        exports.export(NAMES[index], ExportKind::Func, index as u32);
    }
    let mut module = Module::new();
    module.section(&types).section(&imports).section(&exports);
    module.finish()
}
#[test]
fn private_namespace_module_has_exact_import_signature_inventory() {
    let bytes = probe_module(NAMESPACE);
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    assert_eq!(bytes, probe_module(NAMESPACE));
    assert_ne!(bytes, probe_module("env"));
    let mut actual = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
        if let wasmparser::Payload::ImportSection(imports) = payload.unwrap() {
            for import in imports.into_imports() {
                let import = import.unwrap();
                actual.push((import.module.to_owned(), import.name.to_owned()));
            }
        }
    }
    assert_eq!(
        actual,
        NAMES.map(|name| (NAMESPACE.to_owned(), name.to_owned()))
    );
}
#[test]
fn actual_core_wasm_host_rejects_foreign_stale_ranges_and_settles_one_backing() {
    use sha2::{Digest, Sha256};
    // Source-bound ordinary eight-row dimensions, not a checked4096 execution.
    let source_text =
        include_str!("../../tests/fixtures/record-traversal-maximum.spx").replace("4096", "8");
    let source = crate::check(&source_text, "private-wasm-fusion.spx").unwrap();
    let typed = crate::hir::resolve(&source).unwrap();
    let descriptor = crate::construction_fusion::derive(&source, &typed).unwrap();
    let bytes = probe_module(NAMESPACE);
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let stem = format!("semaprax-private-fused-host-{}", std::process::id());
    let directory = std::env::temp_dir().join(stem);
    std::fs::create_dir(&directory).unwrap();
    let wasm = directory.join("probe.wasm");
    let script = directory.join("probe.mjs");
    std::fs::write(&wasm, bytes).unwrap();
    std::fs::write(&script, include_str!("fused_row_host/probe.mjs")).unwrap();
    let output = std::process::Command::new("node")
        .arg(&script)
        .arg(&wasm)
        .arg(digest)
        .arg(descriptor.count().to_string())
        .arg(descriptor.width().to_string())
        .output()
        .expect("Node required by existing Core-Wasm owning gate");
    std::fs::remove_dir_all(directory).unwrap();
    assert!(
        output.status.success(),
        "private Core-Wasm boundary failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "private-fused-row-host-v1-ok"
    );
}
