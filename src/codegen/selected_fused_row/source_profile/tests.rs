//! Actual native entry controls; authored for the consolidated owning run.
use super::emit_source_profile;
use crate::hir::selected_fused_candidate::validate_selected;
use sha2::{Digest, Sha256};
use std::process::Command;
const ORIGINAL: &str = include_str!("../../../../tests/fixtures/record-traversal-maximum.spx");
const PREFIX: &str = include_str!("../probe_prefix.c");

fn source_digest(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn source(text: &str, limit: usize) -> String {
    let ast = crate::parse(text, "fused-source-profile.spx").unwrap();
    let owner = validate_selected(&ast).unwrap();
    let hash = source_digest(text);
    emit_source_profile(&owner, limit, &hash).unwrap()
}
fn binaries(unit: &str, label: &str) -> Vec<std::path::PathBuf> {
    let root = std::env::temp_dir().join(format!(
        "fused-source-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let file = root.join("profile.c");
    std::fs::write(&file, unit).unwrap();
    ["-O0", "-O2"]
        .into_iter()
        .map(|optimization| {
            let binary = root.join(format!(
                "profile{optimization}{}",
                std::env::consts::EXE_SUFFIX
            ));
            let built = Command::new(std::env::var_os("CLANG").unwrap_or_else(|| "clang".into()))
                .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
                .arg(&file)
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(
                built.status.success(),
                "{}",
                String::from_utf8_lossy(&built.stderr)
            );
            binary
        })
        .collect()
}
#[test]
fn original_native_source_profile_is_deterministic_and_default_remains_closed() {
    let unit = source(ORIGINAL, 1024 * 1024);
    assert_eq!(unit, source(ORIGINAL, 1024 * 1024));
    assert!(crate::check(ORIGINAL, "ordinary.spx")
        .unwrap_err()
        .iter()
        .any(|d| d.code == "SPX-T267"));
    let legacy_text = ORIGINAL.replace("4096", "32");
    let legacy = crate::check(&legacy_text, "ordinary-32.spx").unwrap();
    assert!(
        validate_selected(&legacy).is_err(),
        "explicit profile cannot fall back to Legacy32"
    );
    let ast = crate::parse(ORIGINAL, "authority.spx").unwrap();
    let owner = validate_selected(&ast).unwrap();
    for (limit, hash) in [
        (0, "a".repeat(64)),
        (1, "a".repeat(63)),
        (1, "G".repeat(64)),
    ] {
        assert_eq!(
            emit_source_profile(&owner, limit, &hash).unwrap_err().code,
            "SPX-B104"
        );
    }
    for binary in binaries(&unit, "original") {
        let output = Command::new(&binary).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["result"], "8386560");
        assert_eq!(value["outcome"], "returned");
        assert_eq!(value["profile"], "fused-construction-v1");
        assert_eq!(value["source_sha256"], source_digest(ORIGINAL));
    }

    let maximum = ORIGINAL.replace("{ total } else { 0 }", "{ 9223372036854775807 } else { 0 }");
    assert_ne!(maximum, ORIGINAL);
    for binary in binaries(&source(&maximum, 1024 * 1024), "i64-maximum") {
        let output = Command::new(&binary).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["outcome"], "returned");
        assert_eq!(value["result"], "9223372036854775807");
        assert_eq!(value["source_sha256"], source_digest(maximum));
    }

    for binary in binaries(&source(ORIGINAL, 1), "refused") {
        let output = Command::new(&binary).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["outcome"], "administrative_refusal");
        assert_eq!(value["managed_reserved"], 0);
        assert!(value.get("result").is_none() && value.get("status").is_none());
    }
}
#[test]
fn native_source_account_exact_short_reentry_and_settlement_use_one_parent() {
    let unit = source(ORIGINAL, 1024 * 1024);
    let driver = r#"
int main(void) {
    spx_profile_account paid={0},short_account={0};
    assert(spx_profile_account_init(&paid,SPX_PROFILE_REQUIRED*2u));
    assert(!spx_profile_account_init(&paid,SPX_PROFILE_REQUIRED*3u));
    struct spx_status_entry entries[1];struct spx_context context;
    for (unsigned i=0;i<2;i++) {
        memset(entries,0,sizeof(entries));
        assert(spx_context_init(&context,1,entries,1,NULL,NULL,NULL));
        census_reset();profile_malloc_bytes=0;int64_t result=-99;spx_status_token token=SPX_STATUS_SUCCESS;
        assert(spx_profile_invoke(&paid,&context,&result,&token)==SPX_FUSED_PUBLISHED);
        assert(result==8386560 && token==SPX_STATUS_SUCCESS && !census.live);
        assert(paid.debited==(i+1u)*SPX_PROFILE_REQUIRED && !paid.issuer.active);
        assert(paid.structure.debited==(i+1u)*SPX_FUSED_SELECTED_FUNDING);
        assert(paid.payload.events==i+1u && paid.payload.cumulative==(i+1u)*SPX_PROFILE_PAYLOAD);
        assert(census.mallocs==4096u && census.calls==5u);
        size_t requested=profile_malloc_bytes;
        for (size_t n=0;n<census.calls;n++) requested+=census.bytes[n];
        assert(profile_malloc_bytes==SPX_PROFILE_STRINGS && requested==SPX_PROFILE_REQUIRED);
    }
    census_reset();int64_t result=-99;spx_status_token token=SPX_STATUS_SUCCESS;
    assert(spx_profile_invoke(&paid,&context,&result,&token)==SPX_FUSED_REFUSED);
    assert(paid.failed && result==-99 && token==SPX_STATUS_SUCCESS && !census.live && !census.calls && !census.mallocs);
    assert(spx_profile_account_init(&short_account,SPX_PROFILE_REQUIRED-1u));
    assert(spx_profile_invoke(&short_account,&context,&result,&token)==SPX_FUSED_REFUSED);
    assert(short_account.failed && !short_account.debited && result==-99 && !census.calls);
    assert(spx_profile_invoke(&short_account,&context,&result,&token)==SPX_FUSED_REFUSED);
    assert(!short_account.debited && !census.calls && !census.mallocs);
    return 0;
}
"#;
    let observer = r#"
static size_t profile_malloc_bytes;
static void *profile_counted_malloc(size_t size) {
    assert(size<=SIZE_MAX-profile_malloc_bytes);profile_malloc_bytes+=size;
    return probe_malloc(size);
}
#undef malloc
#define malloc profile_counted_malloc
"#;
    for binary in binaries(&format!("#define PROBE_KEEP_FAIL_STOP 1\n{PREFIX}\n{observer}\n#define main profile_unused_main\n{unit}\n#undef main\n{driver}"),"account") {
        let output=Command::new(&binary).output().unwrap();
        assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
    }
}
#[test]
fn native_source_failure_preserves_status_and_never_publishes_poison() {
    let text = ORIGINAL
        .replace("4096", "33")
        .replace("let mut total = 0;", "let mut total = 9223372036854775807;");
    let unit = source(&text, 1024 * 1024);
    let driver = r#"
int main(void) {
    spx_profile_account paid={0};assert(spx_profile_account_init(&paid,SPX_PROFILE_REQUIRED*3u));
    for (unsigned i=0;i<3;i++) {
        struct spx_status_entry entries[1]={0};struct spx_context context;
        assert(spx_context_init(&context,1,entries,1,NULL,NULL,NULL));
        census_reset();int64_t result=-99;spx_status_token token=SPX_STATUS_SUCCESS;
        assert(spx_profile_invoke(&paid,&context,&result,&token)==SPX_FUSED_LANGUAGE_FAILURE);
        const struct spx_normalized_status *status=spx_status_resolve(&context,token);
        assert(status && !strcmp(status->domain_id,"semaprax.arithmetic.v1") && status->code==1);
        assert(result==-99 && !census.live && !paid.issuer.active && !paid.failed);
        assert(paid.debited==(i+1u)*SPX_PROFILE_REQUIRED);
    }
    return profile_unused_main();
}
"#;
    for binary in binaries(&format!("#define PROBE_KEEP_FAIL_STOP 1\n{PREFIX}\n#define main profile_unused_main\n{unit}\n#undef main\n{driver}"),"failure") {
        let output=Command::new(&binary).output().unwrap();
        assert_eq!(output.status.code(),Some(1),"{}",String::from_utf8_lossy(&output.stderr));
        let value:serde_json::Value=serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["outcome"],"language_failure");
        assert_eq!(value["status"]["domain"],"semaprax.arithmetic.v1");
        assert_eq!(value["status"]["code"],1);
        assert!(value.get("result").is_none());
    }
}
