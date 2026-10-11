//! Owning selected Core-Wasm definitions for OPT #747. Authored, unexecuted.
//! Publishing tests resolve the original source through the sealed full-validation
//! factory. Rescaled HIR below is confined to unvalidated precursor refusal controls.
use super::precursor::inspect;
use super::*;
use crate::byte_data_capacity::{
    analyze_with_strategy, AllocationStrategy, CapacityFlow as F, FunctionCapacityInput,
    ProgramCapacitySummary,
};
use crate::hir::{
    ResolvedExpr as E, ResolvedExprKind as K, ResolvedProgram, ResolvedStatement as S,
};
use wasm_encoder::ValType;

mod view_loans;

const ORIGINAL: &str = include_str!("../../../tests/fixtures/record-traversal-maximum.spx");

fn text(count: u32) -> String {
    ORIGINAL.replace("4096", &format!("{count:>4}"))
}
fn rescale(e: &mut E, from: u64, to: u64) {
    match &mut e.kind {
        K::Int(value) if u64::try_from(*value) == Ok(from) => *value = i64::try_from(to).unwrap(),
        K::Usize(value) if *value == from => *value = to,
        K::Binary { left, right, .. } => {
            rescale(left, from, to);
            rescale(right, from, to);
        }
        K::Call { args, .. } => args.iter_mut().for_each(|arg| rescale(arg, from, to)),
        K::If { condition, .. } => rescale(condition, from, to),
        _ => {}
    }
}
fn rescaled(count: u32) -> ResolvedProgram {
    let source = crate::check(&text(32), "selected-fused-wasm.spx").unwrap();
    let mut program = crate::hir::resolve(&source).unwrap();
    let K::Block { statements, tail } = &mut program.functions[0].body.kind else {
        panic!("body")
    };
    rescale(tail, 32, u64::from(count));
    let [S::Let {
        value: allocation, ..
    }, _, S::While { condition, .. }, ..] = statements.as_mut_slice()
    else {
        panic!("prefix")
    };
    rescale(allocation, 32, u64::from(count));
    rescale(condition, 32, u64::from(count));
    program
}
struct Inputs {
    source: crate::ast::Program,
    program: ResolvedProgram,
}
fn inputs(count: u32) -> Inputs {
    Inputs {
        source: crate::parse(&text(count), "selected-fused-wasm.spx").unwrap(),
        program: rescaled(count),
    }
}
fn selected_capacity(producer: &str, rows: u32, width: u64) -> ProgramCapacitySummary {
    let flow = F::FusedConstruction {
        producer: producer.to_owned(),
        rows,
        width,
        condition: Box::new(F::Empty),
        body: Box::new(F::BytesCopy {
            site: producer.to_owned(),
            conservative_payload_bytes: width,
        }),
    };
    let input = FunctionCapacityInput {
        function: "app.main".to_owned(),
        array_slots: Vec::new(),
        execution: flow,
    };
    analyze_with_strategy(&[input], AllocationStrategy::FusedConstructionV1).unwrap()
}
fn capacity(inputs: &Inputs) -> ProgramCapacitySummary {
    capacity_for(&inputs.source, &inputs.program)
}
fn capacity_for(source: &crate::ast::Program, program: &ResolvedProgram) -> ProgramCapacitySummary {
    let descriptor = crate::construction_fusion::derive(source, program).unwrap();
    let rows = u32::try_from(descriptor.count()).unwrap();
    let width = u64::try_from(descriptor.width()).unwrap();
    selected_capacity(descriptor.byte_site().id.as_str(), rows, width)
}
fn run_node(module: &Artifact, text_bytes: u64, mode: &str, expected: i64) {
    let grant = &module.grant;
    let directory = std::env::temp_dir().join(format!(
        "semaprax-selected-fused-{}-{mode}-{}",
        std::process::id(),
        grant.rows
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let wasm = directory.join("selected.wasm");
    let driver = directory.join("driver.mjs");
    std::fs::write(&wasm, &module.bytes).unwrap();
    std::fs::write(directory.join("host.mjs"), include_str!("host.mjs")).unwrap();
    std::fs::write(&driver, include_str!("driver.mjs")).unwrap();
    let output = std::process::Command::new("node")
        .arg(&driver)
        .arg(&wasm)
        .arg(&grant.digest)
        .arg(grant.rows.to_string())
        .arg(grant.width.to_string())
        .arg(grant.metadata_bytes.to_string())
        .arg(grant.row_bytes.to_string())
        .arg(grant.account_bytes.to_string())
        .arg(grant.registry_entries.to_string())
        .arg(grant.order.join(","))
        .arg(text_bytes.to_string())
        .arg(mode)
        .arg(expected.to_string())
        .output()
        .expect("Node required by the selected Core-Wasm owning gate");
    std::fs::remove_dir_all(directory).unwrap();
    assert!(
        output.status.success(),
        "selected Core-Wasm executor failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "selected-fused-wasm-v1-ok"
    );
}

/// Sealed-source same-token controls: the payload's last read moves into a
/// `byte_len` of the same view nested in its own outer `byte_get`'s index,
/// bare (index 3 is out of range, so the arms' results swap) and under checked
/// subtraction (index 2). Both keep the original result.
const LAST_PAYLOAD_GET: &str =
    "match byte_get(data, 2usize) { Option::Some { value } => value == 0u8, Option::None {} => false, }";
const NESTED_LEN: &str =
    "match byte_get(data, byte_len(data)) { Option::Some { value } => value == 0u8, Option::None {} => true, }";
const CHECKED_INDEX: &str =
    "match byte_get(data, byte_len(data) - 1usize) { Option::Some { value } => value == 0u8, Option::None {} => false, }";
fn nested(replacement: &str) -> String {
    assert_eq!(ORIGINAL.matches(LAST_PAYLOAD_GET).count(), 1);
    ORIGINAL.replace(LAST_PAYLOAD_GET, replacement)
}

fn accepted(count: u32) -> (Artifact, u64) {
    sealed(&text(count), count)
}
fn sealed(source_text: &str, count: u32) -> (Artifact, u64) {
    let source = crate::parse(source_text, "selected-fused-wasm.spx").unwrap();
    let owner = crate::hir::selected_fused_candidate::validate_selected(&source)
        .expect("original source passes full selected validation");
    let descriptor = crate::construction_fusion::derive(owner.source(), owner.program()).unwrap();
    let text_bytes = descriptor.string_bytes() / u64::from(count);
    let artifact = accept(&owner).expect("sealed current owner passes the private Wasm receiver");
    (artifact, text_bytes)
}

#[test]
fn publishing_entry_requires_the_sealed_current_validation_owner() {
    let source = crate::parse(ORIGINAL, "selected-fused-wasm.spx").unwrap();
    let owner = crate::hir::selected_fused_candidate::validate_selected(&source).unwrap();
    let original = accept(&owner).unwrap();
    // Moving the owner cannot retain an earlier join's node addresses.
    let moved = Box::new(owner);
    assert_eq!(original.bytes, accept(&moved).unwrap().bytes);
    let parent = include_str!("../selected_fused.rs");
    assert!(parent.contains("#[cfg(test)]\nmod precursor;"));
    assert!(!parent.contains("enum PendingRootValidation {}"));
    let precursor = include_str!("precursor.rs");
    assert!(!precursor.contains("Artifact {"));
}

#[test]
fn unvalidated_precursor_stays_closed_outside_all_gates() {
    assert!(crate::check(ORIGINAL, "selected-fused-default.spx")
        .unwrap_err()
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T267"));
    let selected = inputs(4096);
    let legacy = crate::byte_data_capacity::analyze(&[]).unwrap();
    assert_eq!(
        inspect(&selected.source, &selected.program, &legacy).err(),
        Some(Refusal::Strategy)
    );
    let empty = analyze_with_strategy(&[], AllocationStrategy::FusedConstructionV1).unwrap();
    assert_eq!(
        inspect(&selected.source, &selected.program, &empty).err(),
        Some(Refusal::Capacity)
    );
    let summary = capacity(&selected);
    let precursor = inspect(&selected.source, &selected.program, &summary).unwrap();
    assert_eq!(precursor.rows(), 4096);
    assert_eq!(precursor.width(), 3);
    assert_eq!(precursor.order(), Ok(["scalar", "text", "payload"]));
    let copy = selected.program.clone();
    assert_eq!(
        precursor.module(&selected.source, &copy).err(),
        Some(Refusal::Stale)
    );
    let ordinary_rows = crate::check(&text(32), "selected-fused-32.spx").unwrap();
    let ordinary_program = crate::hir::resolve(&ordinary_rows).unwrap();
    let summary = capacity_for(&ordinary_rows, &ordinary_program);
    assert_eq!(
        inspect(&ordinary_rows, &ordinary_program, &summary).err(),
        Some(Refusal::Legacy)
    );
    let unrelated = crate::check(
        "module app.leaf;\n@id(\"app.main\") fn main() -> i64 { 0 }\n",
        "selected-fused-ordinary.spx",
    )
    .unwrap();
    let unrelated_program = crate::hir::resolve(&unrelated).unwrap();
    assert_eq!(
        inspect(&unrelated, &unrelated_program, &summary).err(),
        Some(Refusal::Join)
    );
}

#[test]
fn noncanonical_cleanup_or_loan_plans_refuse_before_lowering() {
    let selected = inputs(4096);
    let summary = capacity(&selected);
    let plan = &selected.program.functions[0].cleanup_plan;
    assert!(plan.lexical_control.is_none());
    // Reordering any finalize vector is a different, noncanonical plan.
    let exit = plan
        .exits
        .iter()
        .position(|exit| exit.finalize_in_order.len() >= 2)
        .expect("a multi-owner exit exists in the selected plan");
    let mut reordered = selected.program.clone();
    reordered.functions[0].cleanup_plan.exits[exit]
        .finalize_in_order
        .reverse();
    assert_eq!(
        inspect(&selected.source, &reordered, &summary).err(),
        Some(Refusal::Cleanup)
    );
    let mut dropped = selected.program.clone();
    let block = dropped.functions[0]
        .cleanup_plan
        .blocks
        .iter_mut()
        .find(|block| !block.transitions.is_empty())
        .unwrap();
    block.transitions.pop();
    assert_eq!(
        inspect(&selected.source, &dropped, &summary).err(),
        Some(Refusal::Cleanup)
    );
    let mut loans = selected.program.clone();
    assert!(!loans.functions[0].loan_plan.loans.is_empty());
    loans.functions[0].loan_plan.loans.pop();
    assert_eq!(
        inspect(&selected.source, &loans, &summary).err(),
        Some(Refusal::Loans)
    );
}

#[test]
fn selected_module_is_wasm1_private_exact_and_deterministic() {
    let (lowered, _) = accepted(4096);
    // Wasm 1.0 only: no reference types, multi-value, bulk memory or sign
    // extension, so no frozen profile gains a newly required feature.
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::WASM1)
        .validate_all(&lowered.bytes)
        .unwrap();
    assert_eq!(lowered.bytes, accepted(4096).0.bytes);
    let mut imports = Vec::new();
    let mut types = Vec::new();
    let mut exports = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(&lowered.bytes) {
        match payload.unwrap() {
            wasmparser::Payload::TypeSection(reader) => {
                for ty in reader.into_iter_err_on_gc_types() {
                    let ty = ty.unwrap();
                    types.push((ty.params().to_vec(), ty.results().to_vec()));
                }
            }
            wasmparser::Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = import.unwrap();
                    let wasmparser::TypeRef::Func(index) = import.ty else {
                        panic!("non-function import")
                    };
                    imports.push((import.module.to_owned(), import.name.to_owned(), index));
                }
            }
            wasmparser::Payload::ExportSection(reader) => {
                for export in reader {
                    exports.push(export.unwrap().name.to_owned());
                }
            }
            _ => {}
        }
    }
    let parsed = |types: &[ValType]| -> Vec<wasmparser::ValType> {
        types
            .iter()
            .map(|ty| match ty {
                ValType::I32 => wasmparser::ValType::I32,
                ValType::I64 => wasmparser::ValType::I64,
                _ => panic!("selected import outside Wasm 1.0 integers"),
            })
            .collect()
    };
    assert_eq!(imports.len(), IMPORTS.len());
    for ((module, name, index), (expected, params, results)) in imports.iter().zip(IMPORTS) {
        assert_eq!(module, NAMESPACE);
        assert_eq!(name, expected);
        let (actual_params, actual_results) = &types[*index as usize];
        assert_eq!(actual_params, &parsed(params));
        assert_eq!(actual_results, &parsed(results));
    }
    exports.sort();
    assert_eq!(exports, ["memory", ENTRY]);
    let host = include_str!("host.mjs");
    let list = host
        .split_once("IMPORT_NAMES = Object.freeze([")
        .and_then(|(_, rest)| rest.split_once("]);"))
        .unwrap()
        .0;
    let mirrored: Vec<&str> = list
        .split(',')
        .map(|name| name.trim().trim_matches('\''))
        .filter(|name| !name.is_empty())
        .collect();
    assert_eq!(mirrored, IMPORTS.map(|(name, _, _)| name));
    assert!(host.contains(&format!("NAMESPACE = '{NAMESPACE}'")));
    assert_eq!(lowered.grant.metadata_bytes, 91_456);
    assert_eq!(
        lowered.grant.account_bytes,
        (layout::registry_entries(4096) * 33).next_multiple_of(8) + 4096 * 24 + 91_456
    );
    assert_eq!(lowered.grant.registry_entries, 2 + 3 * 4096 + 64 + 31 + 1);
    assert_eq!(lowered.grant.digest.len(), 64);
}

#[test]
fn public_profiles_never_name_the_private_selected_namespace() {
    for public in [
        include_str!("../browser_runtime.js"),
        include_str!("../../../tests/owned_data/owned_leaf_vec/host.js"),
    ] {
        assert!(!public.contains(NAMESPACE));
    }
    let host = include_str!("host.mjs");
    assert!(!host.contains("'env'") && !host.contains("\"env\"") && !host.contains("{env"));
    assert!(host.contains("entry.module !== NAMESPACE"));
}

#[test]
fn layout_ledger_matches_actual_host_tables_registry_and_account() {
    assert_eq!(layout::SLOT_BYTES, 22);
    assert_eq!(layout::VIEW_BYTES, 21);
    assert_eq!(layout::ROW_BYTES, 24);
    assert_eq!(layout::REGISTRY_ENTRY_BYTES, 33);
    assert_eq!(layout::MAX_VIEWS, 64);
    assert_eq!(layout::metadata_bytes(4096), 4096 * 22 + 64 * 21);
    assert_eq!(layout::metadata_bytes(33), 2_070);
    assert_eq!(
        layout::account_bytes(33),
        (layout::registry_entries(33) * 33).next_multiple_of(8) + 33 * 24 + 2_072
    );
    assert_eq!(layout::registry_entries(33), 2 + 99 + 64 + 31 + 1);
    assert!(layout::fits_ordinary_vec(4096));
    assert!(!layout::fits_ordinary_vec(4097));
    let host = include_str!("host.mjs");
    for line in [
        "const SLOT_BYTES = 8 + 8 + 1 + 1 + 4;",
        "const VIEW_BYTES = 8 + 4 + 8 + 1;",
        "const ROW_BYTES = 8 + 8 + 8;",
        "const REGISTRY_ENTRY_BYTES = 8 + 4 * 6 + 1;",
        "const MAX_VIEWS = 64;",
        "const MAX_EVENTS = 32;",
        "const MAX_CUMULATIVE = 2097152;",
        "const registryEntries = rows => 2 + 3 * rows + MAX_VIEWS + MAX_CLONES + 1;",
    ] {
        assert!(host.contains(line), "{line}");
    }
}

#[test]
fn original_4096_rows_execute_exact_payload_and_sum_through_selected_host() {
    let (lowered, text) = accepted(4096);
    let expected: i64 = (0..4096).sum();
    assert_eq!(expected, 8_386_560);
    assert_eq!(u64::from(lowered.grant.rows) * lowered.grant.width, 12_288);
    assert_eq!(text * 4096, 12_288);
    run_node(&lowered, text, "original", expected);
    // The outer `view_byte` must still receive a live token after the nested
    // same-view `view_len` (the plan's last read) ran: no `stale` refusal.
    for replacement in [NESTED_LEN, CHECKED_INDEX] {
        let (variant, text) = sealed(&nested(replacement), 4096);
        assert_ne!(variant.bytes, lowered.bytes);
        run_node(&variant, text, "original", expected);
    }
}

#[test]
fn selected_host_controls_accounts_foreign_tokens_faults_and_cleanup() {
    for count in [33, 4096] {
        let (artifact, text_bytes) = accepted(count);
        let expected: i64 = (0..i64::from(count)).sum();
        run_node(&artifact, text_bytes, "controls", expected);
    }
}

#[test]
fn shared_namespace_exhaustion_refuses_before_mutation_and_never_resets() {
    let (artifact, text_bytes) = accepted(33);
    let expected: i64 = (0..33).sum();
    run_node(&artifact, text_bytes, "exhaustion", expected);
}
