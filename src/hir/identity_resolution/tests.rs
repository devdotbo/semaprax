use super::*;
use crate::test_allocation_observer::measure;

const SOURCE: &str = r#"module identity.paid;
@id("range.type") record Range {
    @id("range.lo") lo:i64,
    @id("range.hi") hi:i64,
} requires lo <= hi
@id("app.main") fn main()->i64 {
    let row = Range { lo:1, hi:2 };
    row.hi
}
"#;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Stop(usize);
fn backing(text: &str) -> usize {
    let word = std::mem::size_of::<usize>();
    (2 * word + text.len()).div_ceil(word) * word
}

#[test]
fn both_invariant_passes_keep_exact_identity_refusal_and_partial_index_drop() {
    let program = crate::check(SOURCE, "identity-paid.spx").unwrap();
    // Independent source identities include the actual compiler-generated
    // invariant and check functions, with the original source order.
    const FIRST: &[&str] = &["range.type", "core.option", "core.result", "app.main", "range.type#invariant", "range.lo", "range.hi", "core.option.none", "core.option.some", "core.option.some.value", "core.result.ok", "core.result.ok.value", "core.result.err", "core.result.err.error"];
    const SECOND: &[&str] = &["range.type", "core.option", "core.result", "app.main", "range.type#invariant", "range.type#check", "range.lo", "range.hi", "core.option.none", "core.option.some", "core.option.some.value", "core.result.ok", "core.result.ok.value", "core.result.err", "core.result.err.error"];
    let requests: Vec<_> = FIRST.iter().chain(SECOND).map(|text| backing(text)).collect();
    let old = super::super::resolve_verified(&program, None).unwrap();
    let mut seen = 0;
    let current = resolve(&program, None, |bytes| {
        assert_eq!(bytes, requests[seen]);
        seen += 1;
        Ok::<_, Stop>(())
    }).unwrap();
    assert_eq!(seen, 29);
    assert_eq!(current.0, old.0);
    assert_eq!(current.1.reused(), old.1.reused());
    for stop in [0, 13, 14, 28] {
        let mut seen = 0;
        let (error, observed) = measure(|| {
            resolve(&program, None, |bytes| {
                assert_eq!(bytes, requests[seen]);
                let at = seen;
                seen += 1;
                if at == stop { Err(Stop(at)) } else { Ok(()) }
            }).err().unwrap()
        });
        assert!(matches!(error, Failure::Identity(Stop(at)) if at == stop));
        assert_eq!(seen, stop + 1);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        assert!(!observed.invalid);
    }
    // The original source-verifier error wins before any identity request.
    let mut malformed = program.clone();
    malformed.functions[0].stable_id.clear();
    let old = super::super::resolve_with_function_reuse(&malformed, None, None, None, None).err().unwrap();
    let current = resolve(&malformed, None, |_| -> Result<(), Stop> { panic!("request after verifier refusal") }).err().unwrap();
    let Failure::Source(current) = current else { panic!("source error remapped") };
    assert_eq!(current, old);
}

#[test]
fn actual_identity_caller_setup_refuses_without_diagnostic_or_report_allocation() {
    let program = crate::check(SOURCE, "identity-paid.spx").unwrap();
    let ((error, observed), failed, used) = crate::bounded_output::with_limit_usage(0, || {
        measure(|| resolve_with_report(&program, None).err().unwrap())
    });
    assert!(matches!(error, CallerFailure::Setup(_)));
    assert!(failed);
    assert_eq!(used, 0);
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.live, 0);
}
