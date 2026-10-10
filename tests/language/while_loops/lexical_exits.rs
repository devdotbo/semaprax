//! Scalar labels include ordinary nested loops and exact lexical targets.
use super::*;
const SOURCE: &str = r#"module lexical.scalar;
@id("app.main") fn main()->i64 ensures result==24 {
 let mut row=0;let mut total=0;
 while row<4 {
  row=row+1;
  let mut column=0;
  while column<4 {
   column=column+1;
   if column==2 {continue;};
   if column==4 {break;};
   total=total+row+column;
   0
  }
  let mut inert=0;
  while inert<1 {inert=inert+1;0}
  if row==3 {return total;};
  0
 }
 return -1;
}
"#;

#[test]
fn nested_targets_and_legacy_loop_labels_preserve_scalar_returns() {
    // Three rows, columns 1 and 3: (1+1)+(1+3)+(2+1)+(2+3)+(3+1)+(3+3)=24.
    let source = SOURCE;
    let sequence = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root =
        std::env::temp_dir().join(format!("lexical-scalar-{}-{sequence}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("main.spx");
    std::fs::write(&path, &source).unwrap();
    let envelope = envelope_for(&path, "app.main", &[], DEFAULT_MAX_STEPS);
    let parsed: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    assert_eq!(parsed["payload"]["outcome"]["kind"], "returned");
    assert_eq!(parsed["payload"]["outcome"]["value"], "24");
    let ast = semaprax::check(&source, &path).unwrap();
    let canonical = format::canonical(&ast);
    let reparsed = parse(&canonical, &path).unwrap();
    assert_eq!(canonical, format::canonical(&reparsed));
    let bytes = semaprax::wasm::emit_module(&ast).unwrap();
    assert_eq!(bytes, semaprax::wasm::emit_module(&reparsed).unwrap());
    std::fs::write(root.join("main.wasm"), bytes).unwrap();
    std::fs::write(root.join("probe.mjs"), NODE_LOOP_RUNNER).unwrap();
    let observed = Command::new("node")
        .arg(root.join("probe.mjs"))
        .arg(root.join("main.wasm"))
        .arg("24")
        .output()
        .unwrap();
    assert!(
        observed.status.success(),
        "{}",
        String::from_utf8_lossy(&observed.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&observed.stdout).trim(),
        "while-wasm-ok"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn forged_exit_certificate_refuses_before_backend_execution() {
    let ast = semaprax::check(SOURCE, "lexical-hostile.spx").unwrap();
    let mut program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let control = function
        .cleanup_plan
        .lexical_control
        .as_deref_mut()
        .unwrap();
    let certificate = control
        .exits
        .iter_mut()
        .find(|exit| exit.kind == semaprax::ast::LexicalExitKind::Continue)
        .unwrap();
    certificate.kind = semaprax::ast::LexicalExitKind::Break;
    assert_eq!(hir::validate(&program).unwrap_err().code, "SPX-H006");
    assert!(semaprax::wasm::emit_resolved_module(&program).is_err());
    assert!(interpreter::evaluate_resolved_owned_data(&program, "app.main", &[], 10_000).is_err());
}
