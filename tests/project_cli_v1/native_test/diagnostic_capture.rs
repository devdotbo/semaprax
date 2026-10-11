//! Explicit test-only capture; no production scratch or deadline changes.
use super::{NativeFixture, Ordering, SERIAL};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const MAX_ARTIFACTS: usize = 64;
const MAX_BYTES: u64 = 64 * 1024 * 1024;

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn file_hash(path: &Path) -> String {
    use std::io::Read as _;
    let mut file = fs::File::open(path).unwrap();
    let mut digest = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        let count = file.read(&mut bytes).unwrap();
        if count == 0 {
            break;
        }
        digest.update(&bytes[..count]);
    }
    format!("{:x}", digest.finalize())
}
fn compiler() -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").expect("compiler PATH"))
        .map(|directory| directory.join("clang"))
        .find(|path| path.is_file())
        .expect("diagnostic capture requires existing clang")
        .canonicalize()
        .unwrap()
}
fn write_new(path: &Path, bytes: &[u8]) {
    use std::io::Write as _;
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .unwrap()
        .write_all(bytes)
        .unwrap();
}

pub(super) fn run(fixture: &NativeFixture, arguments: &[&str], directory: &Path) -> Output {
    // The explicit directory is diagnostic output authority, never source or
    // invocation authority. Every attempt owns a fresh child and cannot replace
    // an earlier receipt. The default helper does not enter this function.
    fs::create_dir_all(directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let destination = directory.join(format!(
        "{}-attempt-{}",
        fixture.root.file_name().unwrap().to_string_lossy(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&destination).unwrap();
    let captures = destination.join("captures");
    fs::create_dir(&captures).unwrap();
    let wrapper = destination.join("compiler-wrapper");
    fs::create_dir(&wrapper).unwrap();
    let actual_compiler = compiler();
    let script = WRAPPER
        .replace(
            "__COMPILER__",
            &serde_json::to_string(&actual_compiler).unwrap(),
        )
        .replace("__CAPTURES__", &serde_json::to_string(&captures).unwrap());
    let script_path = wrapper.join("clang");
    write_new(&script_path, script.as_bytes());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let mut paths = vec![wrapper];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let source_names = ["semaprax.toml", "app.spx", "tests.spx"];
    let sources: Vec<Value> = source_names
        .iter()
        .map(|name| {
            let bytes = fs::read(fixture.root.join(name)).unwrap();
            write_new(&destination.join(name), &bytes);
            json!({"path":name,"bytes":bytes.len(),"sha256":hash(&bytes)})
        })
        .collect();
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(arguments)
        .current_dir(&fixture.root)
        .env("TMPDIR", &fixture.scratch)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .output()
        .unwrap();
    // Copying occurs before program entry, entirely outside the CLI deadline.
    // The wrapper changes the compiler environment: no speed/loader inference
    // or comparison is justified by a captured run.
    let envelope: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|_| json!({"capture_error":"no JSON case report"}));
    let cases = envelope["cases"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut retained = Vec::new();
    for (ordinal, case) in cases.iter().enumerate() {
        let capture = captures.join(format!("{ordinal:03}"));
        let timeout = case["outcome"].as_str().is_some_and(|outcome| {
            outcome.starts_with("native test exceeded ") && outcome.ends_with(" ms")
        });
        if timeout && capture.exists() {
            let receipt: Value = fs::read(capture.join("compiler.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_else(|| json!({"capture_error":"bounded compiler receipt unavailable"}));
            retained.push(json!({"ordinal":ordinal,"case":case,"compiler":receipt}));
        } else if capture.exists() {
            fs::remove_dir_all(capture).unwrap();
        }
    }
    for ordinal in cases.len()..MAX_ARTIFACTS {
        let capture = captures.join(format!("{ordinal:03}"));
        if capture.exists() {
            fs::remove_dir_all(capture).unwrap();
        }
    }
    assert!(retained.len() <= MAX_ARTIFACTS);
    let artifact_bytes: u64 = fs::read_dir(&captures)
        .unwrap()
        .map(|entry| {
            fs::read_dir(entry.unwrap().path())
                .unwrap()
                .map(|file| file.unwrap().metadata().unwrap().len())
                .sum::<u64>()
        })
        .sum();
    assert!(artifact_bytes <= MAX_BYTES);
    let executable_hash = file_hash(Path::new(env!("CARGO_BIN_EXE_semaprax")));
    let manifest = json!({
        "schema":"semaprax.native-test-timeout-capture.v1",
        "scope":"test-harness diagnostic only; compiler PATH wrapper and copies outside deadline; no performance inference",
        "command":{"executable":env!("CARGO_BIN_EXE_semaprax"),"executable_sha256":executable_hash,"arguments":arguments,"cwd":fixture.root,"tmpdir":fixture.scratch},
        "sources":sources,"stdout_sha256":hash(&output.stdout),"stderr_sha256":hash(&output.stderr),
        "status":output.status.code(),"case_capture_limit":MAX_ARTIFACTS,"artifact_file_limit":MAX_ARTIFACTS * 3,"artifact_byte_limit":MAX_BYTES,
        "compiler_wrapper_sha256":hash(script.as_bytes()),"compiler_sha256":file_hash(&actual_compiler),
        "artifact_bytes":artifact_bytes,"retained":retained
    });
    write_new(&destination.join("stdout.json"), &output.stdout);
    write_new(&destination.join("stderr.txt"), &output.stderr);
    write_new(
        &destination.join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest).unwrap(),
    );
    for entry in fs::read_dir(&captures).unwrap() {
        for file in fs::read_dir(entry.unwrap().path()).unwrap() {
            let file = file.unwrap();
            let mut permissions = file.metadata().unwrap().permissions();
            permissions.set_readonly(true);
            fs::set_permissions(file.path(), permissions).unwrap();
        }
    }
    for file in fs::read_dir(&destination).unwrap() {
        let file = file.unwrap();
        if file.file_type().unwrap().is_file() {
            let mut permissions = file.metadata().unwrap().permissions();
            permissions.set_readonly(true);
            fs::set_permissions(file.path(), permissions).unwrap();
        }
    }
    output
}

// Copies are bounded before reading; a capture failure never substitutes the
// real compiler's status. There is one compilation and one program invocation.
const WRAPPER: &str = r#"#!/usr/bin/env python3
import hashlib, json, os, pathlib, shutil, stat, subprocess, sys
compiler = __COMPILER__
root = pathlib.Path(__CAPTURES__)
args = sys.argv[1:]
cap = None
def copy(source, target):
    with source.open('rb') as reader, target.open('xb') as writer:
        shutil.copyfileobj(reader, writer, length=65536)
def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as reader:
        while True:
            chunk = reader.read(65536)
            if not chunk:
                return result.hexdigest()
            result.update(chunk)
try:
    used = sum(p.stat().st_size for p in root.rglob('*') if p.is_file())
    ordinal = len(list(root.iterdir()))
    if ordinal < 64:
        cap = root / f'{ordinal:03}'
        cap.mkdir()
        source = next(pathlib.Path(a) for a in args if a.endswith('.c'))
        size = source.stat().st_size
        if used + size <= 67108864:
            copy(source, cap / 'source.c')
except Exception:
    cap = None
result = subprocess.run([compiler, *args])
try:
    if cap is not None:
        target = pathlib.Path(args[args.index('-o') + 1])
        used = sum(p.stat().st_size for p in root.rglob('*') if p.is_file())
        if result.returncode == 0 and used + target.stat().st_size <= 67108864:
            copy(target, cap / 'program')
            (cap / 'program').chmod(stat.S_IMODE(target.stat().st_mode))
        files = [{ 'path':p.name, 'bytes':p.stat().st_size, 'sha256':digest(p) }
                 for p in sorted(cap.iterdir()) if p.is_file()]
        receipt = json.dumps({'compiler':compiler, 'arguments':args, 'exit_code':result.returncode, 'files':files},sort_keys=True).encode()
        used = sum(p.stat().st_size for p in root.rglob('*') if p.is_file())
        if used + len(receipt) <= 67108864:
            (cap / 'compiler.json').write_bytes(receipt)
except Exception:
    pass
if result.returncode < 0:
    os.kill(os.getpid(), -result.returncode)
sys.exit(result.returncode)
"#;
