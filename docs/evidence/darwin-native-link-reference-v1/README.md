# Darwin native tool reference

[Public SDK job 116714584363](https://github.com/wavect/semaprax/actions/runs/38819916075/job/116714584363)
failed two calculator consumers with I231. The builder deliberately keeps that
diagnostic static and suppresses child stderr. The new blocking macOS SDK step
runs a small independent Rust/C reference with the configured compiler images,
empty child environments, and the current prepared Rust staticlib and Clang
link arguments. It prints ordinary tool diagnostics before the full SDK gate.

The reference calls an exported Rust function using the unwind runtime, links
the static library from C and executes the result. Its local Rust 1.88 run
passed with a sampled aggregate child-tree peak of 97,232 KiB under a 200 MiB
stop guard. This does not reproduce or resolve the hosted calculator failure.

A separate local component probe compiled the complete current Unix quarantine
and its original common carriers, excluding unrelated doctor routes. It used
the actual discovery, direct-compiler and sysroot checks, prepared C and Rust
compilation, captured object backing, prepared link, executable admission and
execution on the same small input. It passed with sampled compiler/runtime
peaks of 195,872 and 84,848 KiB under the same stop guard. It does not qualify
the full SDK, held production inputs or the hosted macOS runner.

The ordinary reference is diagnostic evidence only. The existing exact archive
admission and complete package, consumer, callback and backend-equivalence
gates remain blocking and unchanged. The production tool environment, process
settlement, diagnostics and publication authority are unchanged.
