//! Compare direct wire-row sorting against the retained independent declaration-
//! order carrier pipeline. Both use the same checked compiler and frozen corpus;
//! this is differential behavior, not historical runtime/cost evidence.
use super::{compile_native, execute, hex, project, stage, CORPUS};

const ORIGINAL_MODEL: &str = include_str!("catalog_sort_adoption/model_original.txt");
const ORIGINAL_RESTOCK: &str = include_str!("catalog_sort_adoption/restock_original.txt");
const CURRENT_RESTOCK: &str =
    include_str!("../../../../../examples/catalog-scoped-record-successor/src/restock.spx");

#[test]
fn direct_wire_row_sort_retains_independent_original_pipeline_and_all_23_cases() {
    let root = crate::standard_library::temporary("catalog-direct-sort-reference");
    stage(&root);
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let program = snapshot.public_api_program();
        semaprax::hir::validate(program).map_err(|error| vec![error])?;
        let run = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "catalog.run")
            .unwrap();
        let mut pending = vec![&run.body];
        let mut sorts = Vec::new();
        while let Some(expression) = pending.pop() {
            if let semaprax::hir::ResolvedExprKind::Call { callee, .. } = &expression.kind {
                if callee.as_str().starts_with("core.vec.sort-by.v1:") {
                    sorts.push(callee.as_str());
                }
            }
            push_children(expression, &mut pending);
        }
        assert_eq!(sorts.len(), 1);
        let descriptor = sorts[0];
        let mut previous = 0;
        for field in ["department", "priority", "id", "stock", "mark"] {
            let identity = format!("catalog.output_item.{field}");
            // The layout also contains every field; select the last occurrence,
            // in the independently authenticated ordered key suffix.
            let offset = descriptor.rfind(&identity).unwrap();
            assert!(offset >= previous);
            previous = offset + identity.len();
        }
        assert!(!snapshot.semantic_graph().contains("catalog.sort_item"));
        assert_eq!(CURRENT_RESTOCK.matches("vec_push<Item>").count(), 1);
        assert_eq!(
            CURRENT_RESTOCK.matches("vec_into_iter<InputItem>").count(),
            1
        );
        assert!(!CURRENT_RESTOCK.contains("vec_into_iter<Item>"));
        Ok(())
    })
    .unwrap();

    // Original source is retained verbatim, not reconstructed from the new key
    // policy. Its field order independently encodes the original business order.
    let original = semaprax::parse(ORIGINAL_MODEL, "original-model.spx").unwrap();
    let row = original
        .types
        .iter()
        .find(|declaration| declaration.stable_id == "catalog.sort_item")
        .unwrap();
    let semaprax::ast::TypeDeclarationKind::Record { fields } = &row.kind else {
        panic!("original sort carrier is not a record")
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["department", "priority", "id", "stock", "mark"]
    );
    for (path, bytes) in [
        ("src/model.spx", ORIGINAL_MODEL),
        (
            "src/order.spx",
            super::super::string_slot_adoption::catalog_order(),
        ),
        ("src/restock.spx", ORIGINAL_RESTOCK),
    ] {
        std::fs::write(root.join(path), bytes).unwrap();
    }
    let source = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert!(snapshot.semantic_graph().contains("catalog.sort_item"));
        assert!(!snapshot.semantic_graph().contains("core.vec.sort-by.v1:"));
        semaprax::codegen::emit_hir_c_with_stdin_stream_collection_records(
            snapshot.public_api_program(),
            "catalog.command",
        )
        .map_err(|error| vec![error])
    })
    .unwrap();
    let corpus: serde_json::Value = serde_json::from_str(CORPUS).unwrap();
    let cases = corpus["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 23);
    for optimization in ["-O0", "-O2"] {
        let binary = compile_native(&source, &root, optimization);
        for case in cases {
            let output = execute(&binary, &hex(case["input_hex"].as_str().unwrap()));
            let name = case["name"].as_str().unwrap();
            assert_eq!(
                output.status.code(),
                Some(i32::try_from(case["status"].as_i64().unwrap()).unwrap()),
                "original {optimization}: {name}"
            );
            assert_eq!(
                output.stdout,
                hex(case["stdout_hex"].as_str().unwrap()),
                "original {optimization}: {name}"
            );
            assert_eq!(
                output.stderr,
                hex(case["stderr_hex"].as_str().unwrap()),
                "original {optimization}: {name}"
            );
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

// Independent exhaustive test traversal; compiler-private visitors remain private.
fn push_children<'a>(
    expression: &'a semaprax::hir::ResolvedExpr,
    pending: &mut Vec<&'a semaprax::hir::ResolvedExpr>,
) {
    match &expression.kind {
        semaprax::hir::ResolvedExprKind::Closure { captures, .. } => {
            pending.extend(captures.iter().rev().map(|capture| &capture.value))
        }
        semaprax::hir::ResolvedExprKind::VecConstructor {
            statements, tail, ..
        }
        | semaprax::hir::ResolvedExprKind::Block { statements, tail } => {
            pending.push(tail);
            for statement in statements.iter().rev() {
                for index in (0..statement.child_count()).rev() {
                    if let Some(child) = statement.child(index) {
                        pending.push(child);
                    }
                }
            }
        }
        semaprax::hir::ResolvedExprKind::FunctionReference { .. } => {}
        semaprax::hir::ResolvedExprKind::Invoke { callable, args } => {
            pending.extend(args.iter().rev());
            pending.push(callable);
        }
        semaprax::hir::ResolvedExprKind::Call { args, .. }
        | semaprax::hir::ResolvedExprKind::LiteralFormat { args, .. }
        | semaprax::hir::ResolvedExprKind::VecFieldRead { args, .. }
        | semaprax::hir::ResolvedExprKind::RowLoan { args, .. } => {
            pending.extend(args.iter().rev())
        }
        semaprax::hir::ResolvedExprKind::NativeRustImportCall(call) => {
            pending.extend(call.args.iter().rev())
        }
        semaprax::hir::ResolvedExprKind::HostCommandCall(call) => {
            pending.extend(call.args.iter().rev())
        }
        semaprax::hir::ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            pending.push(end);
            pending.push(start);
            pending.push(source);
        }
        semaprax::hir::ResolvedExprKind::LexicalExit { value, .. } => {
            if let Some(value) = value {
                pending.push(value);
            }
        }
        semaprax::hir::ResolvedExprKind::Unary { value, .. }
        | semaprax::hir::ResolvedExprKind::Try { operand: value, .. }
        | semaprax::hir::ResolvedExprKind::TryOption { operand: value, .. }
        | semaprax::hir::ResolvedExprKind::Project { base: value, .. }
        | semaprax::hir::ResolvedExprKind::Upcast { source: value }
        | semaprax::hir::ResolvedExprKind::Yield { request: value } => pending.push(value),
        semaprax::hir::ResolvedExprKind::Binary { left, right, .. } => {
            pending.push(right);
            pending.push(left);
        }
        semaprax::hir::ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            pending.push(else_branch);
            pending.push(then_branch);
            pending.push(condition);
        }
        semaprax::hir::ResolvedExprKind::ConstructRecord { fields, .. }
        | semaprax::hir::ResolvedExprKind::ConstructVariant { fields, .. } => {
            pending.extend(fields.iter().rev().map(|field| &field.value));
        }
        semaprax::hir::ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            for arm in arms.iter().rev() {
                pending.push(&arm.value);
                if let Some(guard) = &arm.guard {
                    pending.push(guard);
                }
            }
            pending.push(scrutinee);
        }
        semaprax::hir::ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            pending.extend(fields.iter().rev().map(|field| &field.value));
            pending.push(base);
        }
        semaprax::hir::ResolvedExprKind::Int(_)
        | semaprax::hir::ResolvedExprKind::Int32(_)
        | semaprax::hir::ResolvedExprKind::Char(_)
        | semaprax::hir::ResolvedExprKind::Uint8(_)
        | semaprax::hir::ResolvedExprKind::Usize(_)
        | semaprax::hir::ResolvedExprKind::ArrayU8(_)
        | semaprax::hir::ResolvedExprKind::RepeatArrayU8 { .. }
        | semaprax::hir::ResolvedExprKind::Float32(_)
        | semaprax::hir::ResolvedExprKind::Float64(_)
        | semaprax::hir::ResolvedExprKind::Bool(_)
        | semaprax::hir::ResolvedExprKind::String(_)
        | semaprax::hir::ResolvedExprKind::BorrowedUtf8Literal(_)
        | semaprax::hir::ResolvedExprKind::Place(_)
        | semaprax::hir::ResolvedExprKind::BorrowPlace { .. } => {}
    }
}
