//! Owning selected fused-row receiver definitions. Authored; the root
//! executes them only after the integration batch. Every emitted unit here
//! uses either the ordinary-count UNVALIDATED precursor or the concrete full
//! selected owner for the immutable original 4096-row source. All new owning
//! execution assertions remain unqualified until the root runs this batch.
use super::{
    cleanup::PlanProof,
    layout,
    lower::FUNCTION,
    precursor::{emit_unvalidated_precursor, UnvalidatedPrecursor},
    ENTRY,
};
use crate::cleanup_plan::{ExitContinuation, StatusLane};
use crate::{ast, hir};
use std::process::{Command, Output};

const ORIGINAL: &str = include_str!("../../../tests/fixtures/record-traversal-maximum.spx");
const PREFIX: &str = include_str!("probe_prefix.c");
const DRIVER: &str = include_str!("driver.c");

fn inputs(text: &str) -> (ast::Program, hir::ResolvedProgram) {
    let source = crate::check(text, "selected-fused.spx").unwrap();
    let typed = hir::resolve(&source).unwrap();
    (source, typed)
}
fn fixture(count: u32) -> String {
    ORIGINAL.replace("4096", &count.to_string())
}
fn precursor_unit(text: &str) -> String {
    let (source, typed) = inputs(text);
    let precursor = UnvalidatedPrecursor::derive(&source, &typed).expect("precursor join");
    emit_unvalidated_precursor(&source, &typed, &precursor)
        .unwrap()
        .unvalidated_source
}
/// The emitted selected function, excluding prelude, runtime and entry.
fn function_body(source: &str) -> &str {
    let start = source
        .find(&format!("spx_status_token {FUNCTION}("))
        .expect("selected function");
    let end = source[start..]
        .find("enum { SPX_FUSED_PUBLISHED")
        .expect("entry")
        + start;
    &source[start..end]
}
/// Every used failure exit's finalizers appear verbatim, in canonical order,
/// and every plan failure exit is used.
fn assert_exits_verbatim(body: &str, function: &hir::ResolvedFunction) {
    for exit in &function.cleanup_plan.exits {
        let label = format!("spx_exit_{}:", exit.id.0);
        let failure = matches!(exit.continuation, ExitContinuation::ReturnFailure { .. });
        assert_eq!(body.contains(&label), failure, "{label}");
        let Some(start) = body.find(&label) else {
            continue;
        };
        let block = &body[start..];
        let block = &block[..block.find("goto spx_return;").unwrap()];
        let mut cursor = block.find("spx_fused_abort(spx_domain);").unwrap();
        for action in &exit.finalize_in_order {
            let needle = format!("if (spx_flag_{}) {{", action.guard_flag.0);
            cursor += block[cursor..]
                .find(&needle)
                .expect("finalizer in canonical order");
            cursor += needle.len();
        }
        assert!(block[cursor..].contains("spx_fused_settle_abort(spx_domain)"));
    }
}

#[test]
fn precursor_lowers_plan_bound_rows_and_leaves_default_emission_unchanged() {
    for count in [1u32, 8, 32] {
        let text = fixture(count);
        let (source, typed) = inputs(&text);
        let precursor = UnvalidatedPrecursor::derive(&source, &typed).unwrap();
        let unit = emit_unvalidated_precursor(&source, &typed, &precursor).unwrap();
        // One staged row, the items/rows/renewed headers, one bound leased view.
        assert_eq!(unit.placement, [1, 3, 1]);
        let emitted = &unit.unvalidated_source;
        assert!(emitted.contains(&format!("static __attribute__((unused)) int {ENTRY}(")));
        assert!(!emitted.contains("int main("));
        let body = function_body(emitted);
        // Interior ranges never reach the ordinary tag-10 drop or public carriers.
        for closed in [
            "spx_bytes_drop",
            "spx_bytes_v1",
            "spx_leaf_",
            "free(",
            "spx_vec_v1",
        ] {
            assert!(!body.contains(closed), "{closed} in selected body");
        }
        // Private carriers live in the admitted heap frame, never the C stack.
        for carrier in [
            "spx_fused_staged spx_l",
            "spx_fused_rows spx_l",
            "spx_fused_view spx_l",
        ] {
            assert!(!body.contains(carrier), "{carrier} on the stack");
        }
        assert!(emitted.contains("} spx_fused_frame;") && body.contains("spx_frame->"));
        // Original preparation order: Vec, scalar, ordinary String, then the issue.
        let string = body.find("spx_string_from_literal(").unwrap();
        let issue = body.find("spx_fused_issue(").unwrap();
        assert!(body.find("spx_fused_rows_allocate_funded(").unwrap() < string && string < issue);
        assert!(issue < body.find("spx_fused_rows_has_room(").unwrap());
        assert_eq!(body.matches("spx_fused_issue(").count(), 1);
        assert_eq!(body.matches("spx_fused_rows_borrow(").count(), 1);
        assert_eq!(body.matches("spx_fused_rows_sort(").count(), 1);
        assert!(body.contains("SPX_STATUS_ARITHMETIC_ADD_OVERFLOW"));
        assert!(body.contains(crate::string_ops::CONVERT_STATUS_DOMAIN));
        assert!(!body.contains("spx_refused"));
        assert_exits_verbatim(body, &typed.functions[0]);
        let ordinary = crate::codegen::emit_hir_c(&typed).unwrap();
        assert!(
            !ordinary.contains("spx_fused"),
            "default emitter reached the private route"
        );
    }
}

#[test]
fn stale_foreign_decoded_and_drifted_precursors_refuse_before_lowering() {
    let text = fixture(8);
    let (source, typed) = inputs(&text);
    let precursor = UnvalidatedPrecursor::derive(&source, &typed).unwrap();
    // A decoded or cloned program is a different held object, never the precursor's.
    let decoded = typed.clone();
    assert!(emit_unvalidated_precursor(&source, &decoded, &precursor).is_err());
    let reparsed = source.clone();
    assert!(emit_unvalidated_precursor(&reparsed, &typed, &precursor).is_err());
    let held = UnvalidatedPrecursor::derive(&source, &decoded).unwrap();
    assert!(emit_unvalidated_precursor(&source, &typed, &held).is_err());
    assert!(emit_unvalidated_precursor(&source, &decoded, &held).is_ok());
    for (from, to) in [
        ("bytes_zeroed(3usize)", "bytes_zeroed(4usize)"),
        ("next = next + 1", "next = next + 2"),
        (
            "let rows = items;",
            "let rows = items;let copied=vec_clone_at<Row>(rows,0usize);",
        ),
    ] {
        let drifted = crate::parse(&text.replace(from, to), "drift.spx").unwrap();
        assert!(
            UnvalidatedPrecursor::derive(&drifted, &typed).is_none(),
            "{to}"
        );
    }
    let mut forged = typed.clone();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut forged.functions[0].body.kind else {
        panic!("body")
    };
    let hir::ResolvedStatement::While { body, .. } = &mut statements[2] else {
        panic!("loop")
    };
    let hir::ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("rows")
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("row")
    };
    let hir::ResolvedExprKind::ConstructRecord { fields, .. } = &mut value.kind else {
        panic!("record")
    };
    fields.swap(1, 2);
    assert!(UnvalidatedPrecursor::derive(&source, &forged).is_none());
    let ordinary = "module app.plain; @id(\"app.main\") fn main() -> i64 { 0 }";
    let (plain, plain_typed) = inputs(ordinary);
    assert!(UnvalidatedPrecursor::derive(&plain, &plain_typed).is_none());
}

#[test]
fn closed_suffix_census_refuses_escape_clone_and_unknown_consumers() {
    let text = fixture(8);
    for (from, to) in [
        (
            "let renewed = vec_sort_owned<Row>(rows);",
            "let renewed = vec_sort_owned<Row>(rows); let leaked = vec_len<Row>(rows);",
        ),
        (
            "let data = row.payload;",
            "let data = row.payload; let copy = bytes_copy(data);",
        ),
        (
            "total = total + row.priority;",
            "total = total + row.priority; let again = vec_clone_at<Row>(rows, 0usize);",
        ),
    ] {
        let changed = text.replace(from, to);
        let Ok(source) = crate::check(&changed, "census.spx") else {
            continue; // ordinary verification already refuses the variant
        };
        let Ok(typed) = hir::resolve(&source) else {
            continue;
        };
        assert!(
            UnvalidatedPrecursor::derive(&source, &typed).is_none(),
            "{to}"
        );
    }
}

/// The ordinary route retains its original 4096-row refusal. The separate
/// concrete selected factory does not change this default verifier.
#[test]
fn original_maximum_remains_refused_until_independent_capacity_admission() {
    assert!(crate::check(ORIGINAL, "original.spx")
        .unwrap_err()
        .iter()
        .any(|d| d.code == "SPX-T267"));
}

#[test]
fn accepting_api_binds_concrete_current_owner_and_refuses_actual_source_mutations() {
    let source = crate::parse(ORIGINAL, "original-selected-native.spx").unwrap();
    let owner = hir::selected_fused_candidate::validate_selected(&source)
        .unwrap_or_else(|e| panic!("selected factory refused: {e:?}"));
    let artifact = super::emit_validated(&owner).unwrap();
    assert_eq!(artifact.count(), 4096);
    assert_eq!(artifact.width(), 3);
    assert!(std::ptr::eq(owner.source(), &source));
    // A cloned AST/HIR cannot be supplied to the concrete receiving API.
    let cloned_source = source.clone();
    let cloned_program = owner.program().clone();
    assert!(!std::ptr::eq(owner.source(), &cloned_source));
    assert!(!std::ptr::eq(owner.program(), &cloned_program));
    assert_eq!(
        super::emit_validated(&owner).unwrap().source(),
        artifact.source()
    );
    for (from, to) in [
        ("next < 4096", "next <= 4096"),
        ("bytes_zeroed(3usize)", "bytes_zeroed(next)"),
        ("next = next + 1", "next = next + 2"),
    ] {
        let changed = ORIGINAL.replace(from, to);
        assert_ne!(changed, ORIGINAL);
        let mutated = crate::parse(&changed, "mutated-selected-native.spx").unwrap();
        assert!(
            hir::selected_fused_candidate::validate_selected(&mutated).is_err(),
            "{from}"
        );
    }
}

#[test]
fn canonical_cleanup_proof_refuses_unprovable_plan_shapes() {
    let (_, typed) = inputs(&fixture(8));
    let function = &typed.functions[0];
    assert!(PlanProof::admit(function).is_ok());
    assert!(!function.cleanup_plan.status_sources.is_empty());
    let mut live = function.clone();
    let place = live.cleanup_plan.slots[0].storage.clone();
    live.cleanup_plan
        .entry_state
        .live_owned_parameters
        .push(crate::cleanup_plan::CleanupPlace {
            storage: place,
            projections: Vec::new(),
        });
    assert!(PlanProof::admit(&live).is_err());
    let mut lane = function.clone();
    lane.cleanup_plan.status_sources[0].id.lane = StatusLane::ContractFalse;
    assert!(PlanProof::admit(&lane).is_err());
    let mut edgeless = function.clone();
    edgeless.cleanup_plan.edges.retain(|edge| {
        !matches!(
            edge.condition,
            crate::cleanup_plan::EdgeCondition::StatusNonzero(_)
        )
    });
    assert!(PlanProof::admit(&edgeless).is_err());
    let mut variant = function.clone();
    variant.cleanup_plan.slots[0].field_liveness_shape =
        crate::cleanup::FieldLivenessShape::Variant {
            declaration: hir::DeclarationId::new("forged.variant"),
            cases: Vec::new(),
        };
    assert!(PlanProof::admit(&variant).is_err());
    // A finalizer whose lifecycle disagrees with its leaf refuses lowering.
    let mut forged = typed.clone();
    let action = forged.functions[0]
        .cleanup_plan
        .exits
        .iter_mut()
        .flat_map(|exit| &mut exit.finalize_in_order)
        .next()
        .expect("a finalizing exit");
    action.lifecycle_id = hir::DeclarationId::new("forged.drop");
    let source = crate::check(&fixture(8), "forged.spx").unwrap();
    let precursor = UnvalidatedPrecursor::derive(&source, &forged).unwrap();
    assert!(emit_unvalidated_precursor(&source, &forged, &precursor).is_err());
}

#[test]
fn receiver_layout_mirror_matches_runtime_and_ordinary_stride() {
    assert!(layout::native64());
    let sizes: Vec<usize> = layout::mirror().iter().map(|(_, size)| *size).collect();
    assert_eq!(sizes, [24, 16, 48, 24, 24, 32, 40, 56, 32, 32, 40, 240, 80]);
    assert_eq!(layout::row(), 32);
    let frame = layout::frame(1, 3, 1).unwrap();
    assert_eq!(frame, 240 + 40 + 3 * 56 + 48);
    assert_eq!(
        layout::metadata(4096, frame),
        Some(frame + 4096 * 24 + 64 * 24)
    );
    assert_eq!(
        layout::funding(4096, frame),
        Some(frame + 4096 * 24 + 64 * 24 + 4096 * 32)
    );
    assert!(layout::frame(0, 0, 2048).is_none());
    let emitted = precursor_unit(&fixture(8));
    assert!(emitted.contains(&format!(
        "_Static_assert(sizeof(spx_fused_frame) == {frame}u"
    )));
    for (ty, size) in layout::mirror() {
        assert!(emitted.contains(&format!("_Static_assert(sizeof({ty}) == {size}u")));
    }
}

fn compile(emitted: &str, expected: i64, label: &str) -> Vec<std::path::PathBuf> {
    let root = std::env::temp_dir().join(format!(
        "fused-row-selected-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("selected.c");
    std::fs::write(
        &source,
        format!(
            "#define PROBE_KEEP_FAIL_STOP 1\n{PREFIX}\n{emitted}\n#define SPX_EXPECTED_RESULT INT64_C({expected})\n{DRIVER}"
        ),
    )
    .unwrap();
    ["-O0", "-O2"]
        .into_iter()
        .map(|optimization| {
            let binary = root.join(format!(
                "selected{optimization}{}",
                std::env::consts::EXE_SUFFIX
            ));
            let compiled =
                Command::new(std::env::var_os("CLANG").unwrap_or_else(|| "clang".into()))
                    .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
                    .arg(&source)
                    .arg("-o")
                    .arg(&binary)
                    .output()
                    .unwrap();
            assert!(
                compiled.status.success(),
                "{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            binary
        })
        .collect()
}
fn run(binary: &std::path::Path, mode: &str) -> Output {
    Command::new(binary).arg(mode).output().unwrap()
}
fn passes(output: &Output, mode: &str) {
    assert!(
        output.status.success(),
        "{mode}: {:?} {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn fail_stops(output: &Output, message: &str) {
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(&format!(
            "SEMAPRAX native runtime invariant failure: {message}"
        )),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn precursor_unit_admission_order_plan_exits_and_reentry_at_o0_o2() {
    for count in [1u32, 8, 32] {
        let expected = (0..i64::from(count)).sum::<i64>();
        let emitted = precursor_unit(&fixture(count));
        for binary in compile(&emitted, expected, &format!("oracle{count}")) {
            for mode in [
                "oracle",
                "one-short",
                "cumulative-short",
                "claim",
                "vec-failure",
                "call-depth",
            ] {
                passes(&run(&binary, mode), mode);
            }
            fail_stops(&run(&binary, "oom-frame"), "fused frame allocation failed");
            fail_stops(
                &run(&binary, "oom-table"),
                "fused lease table allocation failed",
            );
            fail_stops(
                &run(&binary, "oom-views"),
                "fused view table allocation failed",
            );
            fail_stops(
                &run(&binary, "oom-backing"),
                "owned byte buffer allocation failed",
            );
            fail_stops(
                &run(&binary, "unsealed"),
                "selected fused-row entry without held authority",
            );
        }
    }
}

#[test]
fn precursor_suffix_failure_ends_live_loan_through_plan_exit_at_o0_o2() {
    let text = fixture(8).replace("let mut total = 0;", "let mut total = 9223372036854775807;");
    for binary in compile(&precursor_unit(&text), 0, "suffix") {
        passes(&run(&binary, "suffix-failure"), "suffix-failure");
    }
}

/// Actual immutable ORIGINAL AST -> complete selected factory -> concrete
/// native receiving -> O0/O2 runtime oracle. No rescaled/precursor authority.
#[test]
fn original_4096_rows_publish_8386560_with_one_12288_byte_backing_at_o0_o2() {
    let source = crate::parse(ORIGINAL, "original.spx").unwrap();
    let owner = hir::selected_fused_candidate::validate_selected(&source)
        .unwrap_or_else(|e| panic!("original selected factory refused: {e:?}"));
    assert_eq!(owner.capacity().rows, 4096);
    assert_eq!(owner.capacity().extent, 12288);
    let artifact = super::emit_validated(&owner).unwrap();
    assert_eq!(artifact.count(), 4096);
    assert_eq!(artifact.width(), 3);
    assert_exits_verbatim(
        function_body(artifact.source()),
        &owner.program().functions[0],
    );
    for binary in compile(artifact.source(), 8386560, "original4096") {
        for mode in [
            "oracle",
            "one-short",
            "cumulative-short",
            "claim",
            "vec-failure",
            "call-depth",
        ] {
            passes(&run(&binary, mode), mode);
        }
        fail_stops(&run(&binary, "oom-frame"), "fused frame allocation failed");
        fail_stops(
            &run(&binary, "oom-table"),
            "fused lease table allocation failed",
        );
        fail_stops(
            &run(&binary, "oom-views"),
            "fused view table allocation failed",
        );
        fail_stops(
            &run(&binary, "oom-backing"),
            "owned byte buffer allocation failed",
        );
    }
}
