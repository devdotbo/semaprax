use super::*;
const SOURCE: &str = r#"module checked.native;
@id("checked.native.text") fn text() -> string { string_concat("a", "β") }
@id("checked.native.main") fn main() -> i64 {
    let text = text();
    string_len(text)
}
"#;
#[test]
fn exact_source_native_receiver_uses_checked_helpers_and_closed_default_routes() {
    let source = crate::parse(SOURCE, "checked-native.spx").unwrap();
    let owner = crate::cleanup_plan::checked_allocation::prepare(&source,
        &DeclarationId::new("checked.native.main")).unwrap().receive().unwrap();
    let projection = Projection::receive(&owner).unwrap();
    let first = projection.emit().unwrap();
    assert_eq!(first, projection.emit().unwrap());
    assert!(first.contains("spx_checked_allocation_invoke_v1("));
    assert!(first.contains("release_state = ctx->allocator_state"));
    assert!(first.contains("header->release(header->release_state, header)"));
    assert!(first.contains("spx_checked_native_concat_v1(spx_ctx,"));
    assert!(first.contains("checked concat commit epochs"));
    assert!(!first.contains("int main(void)"));
    assert!(super::super::emit_hir_c_with_labels(owner.program(), &HashMap::new(),
        NativeOutputProfile::Legacy, None).is_err());
    assert!(super::super::emit_hir_c_with_labels(owner.program(), &HashMap::new(),
        NativeOutputProfile::CheckedAllocation, None).is_err());
}

#[test]
fn generated_native_null_sweep_settles_committed_inputs_and_reenters_at_o0_o2() {
    let source = crate::parse(SOURCE, "checked-native.spx").unwrap();
    let owner = crate::cleanup_plan::checked_allocation::prepare(&source,
        &DeclarationId::new("checked.native.main")).unwrap().receive().unwrap();
    let emitted = Projection::receive(&owner).unwrap().emit().unwrap();
    let root = std::env::temp_dir().join(format!("checked-native-allocation-{}-{}",
        std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir(&root).unwrap();
    let input = root.join("probe.c");
    std::fs::write(&input, format!("{emitted}\n{}", include_str!("probe.c"))).unwrap();
    let compiler = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("probe{optimization}{}", std::env::consts::EXE_SUFFIX));
        let compiled = std::process::Command::new(&compiler)
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&input).arg("-o").arg(&binary).output().unwrap();
        assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
        let ran = std::process::Command::new(&binary).output().unwrap();
        assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stderr));
        assert_eq!(ran.stdout, b"checked-native-allocation: ok\n");
        assert!(ran.stderr.is_empty());
    }
    std::fs::remove_dir_all(root).unwrap();
}
