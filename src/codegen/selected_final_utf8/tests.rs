use super::*;
const SOURCE: &str = r#"module selected.final;
@id("data.report") record Report { @id("data.raw") raw:string,@id("data.mode") mode:usize, }
@id("data.result") variant Outcome {
 @id("data.encoded") Encoded { @id("data.text") text:string, },
 @id("data.refused") Refused { @id("data.required") required:usize, },
}
@id("app.make") fn make(mode:usize)->Report {Report{raw:"{\"x\":\"é\\u0000\"}",mode:mode}}
@id("app.dispose") fn dispose(value:own Report)->i64 {0}
@id("app.encode") fn encode(value:borrow Report,limit:usize)->Outcome
ensures value.mode!=2usize
{
 let source=str_as_bytes(string_as_str(value.raw));let required=byte_len(source);
 if required>limit {Outcome::Refused{required:required}} else {
  let bytes=bytes_copy(source);
  let finished=if value.mode==1usize {bytes_set(bytes,0usize,255u8)}else{bytes};
  Outcome::Encoded{text:string_from_utf8(bytes_as_slice(finished))}
 }
}
@id("app.unrelated") fn unrelated()->string {"not selected"}
@id("app.main") fn main()->i64 {0}
"#;
fn program(source: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::check(source, "selected-final-utf8.spx").unwrap()).unwrap()
}
fn emit(p: &ResolvedProgram) -> Result<NativeFinalUtf8Artifact, Diagnostic> {
    emit_native_final_utf8(
        p,
        &DeclarationId::new("app.encode"),
        &DeclarationId::new("app.make"),
        &DeclarationId::new("app.dispose"),
    )
}
#[test]
fn certificate_replay_binds_checked_input_constructor_layout_and_preserves_default_bytes() {
    let p = program(SOURCE);
    let before = super::super::emit_hir_c(&p).unwrap();
    let artifact = emit(&p).unwrap();
    assert_eq!(artifact, emit(&p).unwrap());
    assert_eq!(
        artifact,
        replay_native_final_utf8(
            &p,
            &DeclarationId::new("app.encode"),
            &DeclarationId::new("app.make"),
            &DeclarationId::new("app.dispose"),
            artifact.certificate()
        )
        .unwrap()
    );
    assert_eq!(before, super::super::emit_hir_c(&p).unwrap());
    assert!(!artifact.source().contains("int main("));
    assert!(artifact.source().contains("semaprax_final_utf8_prepare_v1"));
    assert!(artifact
        .source()
        .contains("spx_final_utf8_construct(spx_ctx"));
    assert!(!artifact.source().contains("not selected"));
    let mut forged = artifact.certificate().to_vec();
    forged[0] ^= 1;
    assert!(replay_native_final_utf8(
        &p,
        &DeclarationId::new("app.encode"),
        &DeclarationId::new("app.make"),
        &DeclarationId::new("app.dispose"),
        &forged
    )
    .is_err());
    let changed = program(&SOURCE.replace("mode:mode", "mode:0usize"));
    assert!(replay_native_final_utf8(
        &changed,
        &DeclarationId::new("app.encode"),
        &DeclarationId::new("app.make"),
        &DeclarationId::new("app.dispose"),
        artifact.certificate()
    )
    .is_err());
    let metadata: serde_json::Value = serde_json::from_slice(artifact.certificate()).unwrap();
    assert_eq!(
        metadata["input_authority"],
        "checked-source-report-handle.v1"
    );
    assert_eq!(metadata["handle_capacity"], 8);
    assert_eq!(metadata["payload_limit"], 131072);
    assert_eq!(metadata["input_layout_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(metadata["result_layout_sha256"].as_str().unwrap().len(), 64);
}
#[test]
fn selected_closure_refuses_alias_hidden_allocations_and_wrong_provenance_roles() {
    let alias = SOURCE.replace(
        "Outcome::Encoded{text:string_from_utf8(bytes_as_slice(finished))}",
        "let text=string_from_utf8(bytes_as_slice(finished));Outcome::Encoded{text:text}",
    );
    assert_eq!(emit(&program(&alias)).unwrap_err().code, "SPX-B104");
    let literal = SOURCE.replace(
        "string_from_utf8(bytes_as_slice(finished))",
        "\"forbidden allocation\"",
    );
    assert!(emit(&program(&literal)).is_err());
    let wrong = SOURCE.replace(
        "fn dispose(value:own Report)->i64 {0}",
        "fn dispose(value:own Report)->i64 {1}",
    );
    assert!(emit(&program(&wrong)).is_err());
    let p = program(SOURCE);
    assert!(emit_native_final_utf8(
        &p,
        &DeclarationId::new("app.encode"),
        &DeclarationId::new("app.encode"),
        &DeclarationId::new("app.dispose")
    )
    .is_err());
    let mut stale = p.clone();
    stale
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "app.encode")
        .unwrap()
        .body
        .ty = crate::hir::ResolvedType::Bool;
    assert!(emit(&stale).is_err());
}
#[test]
fn selected_checked_source_adapter_faults_and_lease_controls_at_o0_o2() {
    use std::process::Command;
    let p = program(SOURCE);
    let artifact = emit(&p).unwrap();
    let root = std::env::temp_dir().join(format!(
        "selected-final-utf8-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("probe.c");
    std::fs::write(
        &path,
        format!(
            "{}\n{}\n{}",
            include_str!("probe_allocations.c"),
            artifact.source(),
            include_str!("probe.c")
        ),
    )
    .unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!(
            "probe{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let compiled = Command::new(std::env::var_os("CLANG").unwrap_or_else(|| "clang".into()))
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&path)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let result = Command::new(&binary).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"selected-final-utf8-v1: ok\n");
        assert!(result.stderr.is_empty());
    }
    std::fs::remove_dir_all(root).unwrap();
}
