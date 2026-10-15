//! The provider inventory is constructed before execution, inside the same
//! observer as evaluator acquisition and final Drop. It is a prefunded safe
//! payload/control boundary, not a claim that System Arc allocation is nullable.
use super::*;
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
const SOURCE: &str = r#"module checked.native;
@id("checked.native.text") fn text() -> string { string_concat("a", "β") }
@id("checked.native.main") fn main() -> i64 {
    let text = text();
    string_len(text)
}
"#;
#[derive(Debug)]
struct Inventory {
    available: Mutex<VecDeque<Arc<[u8]>>>,
    requests: AtomicUsize,
    refuse_at: usize,
}
impl Inventory {
    fn acquire(parts: &[&str], refuse_at: usize) -> Arc<Self> {
        Arc::new(Self {
            available: Mutex::new(
                parts
                    .iter()
                    .map(|text| Arc::from(text.as_bytes()))
                    .collect(),
            ),
            requests: AtomicUsize::new(0),
            refuse_at,
        })
    }
}
impl Utf8Provider for Inventory {
    fn allocate(&self, pieces: &[&str]) -> Result<Arc<[u8]>, AllocationCase> {
        let ordinal = self.requests.fetch_add(1, Ordering::SeqCst);
        if ordinal == self.refuse_at {
            return Err(AllocationCase::PayloadRefused);
        }
        let bytes = self
            .available
            .lock()
            .unwrap()
            .pop_front()
            .expect("independent inventory census");
        let mut offset = 0;
        for piece in pieces {
            assert_eq!(&bytes[offset..offset + piece.len()], piece.as_bytes());
            offset += piece.len();
        }
        assert_eq!(offset, bytes.len());
        Ok(bytes)
    }
}
fn received<'a>(
    source: &'a crate::ast::Program,
    string_result: bool,
) -> crate::cleanup_plan::checked_allocation::Validated<'a> {
    let entry = DeclarationId::new(if string_result {
        "checked.native.publish"
    } else {
        "checked.native.main"
    });
    if string_result {
        crate::cleanup_plan::checked_allocation::prepare_string_result(source, &entry).unwrap()
    } else {
        crate::cleanup_plan::checked_allocation::prepare(source, &entry).unwrap()
    }
    .receive()
    .unwrap()
}
// The legacy public scanner intentionally excludes String-returning helpers.
// This saved ordinary engine receives the same independently replayed full HIR,
// without installing a checked provider. Its original System String operations,
// default owning clones, fuel, contracts and unwind remain the oracle.
fn ordinary(
    owner: &crate::cleanup_plan::checked_allocation::Validated<'_>,
    fuel: usize,
) -> Result<Value, Flow> {
    owner.replay().unwrap();
    let program = owner.program();
    let entry = program
        .functions
        .iter()
        .find(|f| f.id == *owner.entry())
        .unwrap();
    let functions = program
        .functions
        .iter()
        .map(|f| (f.id.as_str(), f))
        .collect();
    let mut evaluator = Evaluator::new_prepared(
        FunctionLookup::Borrowed(&functions),
        BTreeMap::new(),
        &program.declarations,
        fuel,
        0,
        PreparedCancellation::Never,
    );
    assert!(evaluator.checked_allocation.is_none());
    evaluator.call_frame(entry, Vec::new(), 0)
}

#[test]
fn actual_interpreter_provider_refusal_sweep_preserves_default_clone_and_whole_drop() {
    let source = crate::parse(SOURCE, "checked-interpreter.spx").unwrap();
    let owner = received(&source, false);
    let projection = Projection::receive(&owner).unwrap();
    // Saved ordinary execution independently uses System String and the same
    // full source/entry. No checked payload carrier is installed in that route.
    let legacy =
        super::super::evaluate_resolved_zero_arg_i64(owner.program(), "checked.native.main", 1000)
            .unwrap_err();
    assert!(legacy[0].message.contains("unsupported_callee"));
    assert!(matches!(ordinary(&owner, 1000), Ok(Value::Int(3))));
    for refused in 0..=4 {
        for _ in 0..3 {
            let ((), observed) = crate::test_allocation_observer::measure(|| {
                // Two literals, concat, then the ORIGINAL owning Place clone.
                let provider = Inventory::acquire(&["a", "β", "aβ", "aβ"], refused);
                let outcome = projection.run(provider.clone(), 1000);
                if refused == 4 {
                    assert!(matches!(outcome, Ok(Value::Int(3))));
                } else {
                    let Err(Flow::Failure(status)) = &outcome else {
                        panic!("physical refusal was hidden");
                    };
                    assert_eq!(
                        status.domain_id(),
                        crate::cleanup_plan::ALLOCATION_DOMAIN_V1
                    );
                    assert_eq!(status.code(), 1);
                }
                assert_eq!(
                    provider.requests.load(Ordering::SeqCst),
                    if refused == 4 { 4 } else { refused + 1 }
                );
                drop(outcome);
                drop(provider);
            });
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}
#[test]
fn escaped_interpreter_utf8_retains_actual_provider_and_pre_post_failure_settles() {
    for (body, pieces, expected, failure) in [
        (
            "{ string_concat(\"a\", \"β\") }",
            &["a", "β", "aβ"][..],
            "aβ",
            false,
        ),
        ("{ \"\" }", &[""][..], "", false),
        (
            "requires false { string_concat(\"a\", \"β\") }",
            &[][..],
            "",
            true,
        ),
        (
            "ensures false { string_concat(\"a\", \"β\") }",
            &["a", "β", "aβ"][..],
            "",
            true,
        ),
        (
            "ensures string_len(result) == 3 { string_concat(\"a\", \"β\") }",
            &["a", "β", "aβ", "aβ"][..],
            "aβ",
            false,
        ),
    ] {
        let text = format!("module checked.native; @id(\"checked.native.publish\") fn publish() -> string {body} @id(\"checked.native.main\") fn main() -> i64 {{ 0 }}");
        let source = crate::parse(&text, "checked-interpreter-result.spx").unwrap();
        let owner = received(&source, true);
        let projection = Projection::receive(&owner).unwrap();
        let ((), observed) = crate::test_allocation_observer::measure(|| {
            let provider = Inventory::acquire(pieces, usize::MAX);
            let lifetime = Arc::downgrade(&provider);
            let outcome = projection.run(provider.clone(), 1000);
            assert_eq!(provider.requests.load(Ordering::SeqCst), pieces.len());
            drop(provider);
            if failure {
                assert!(matches!(outcome, Err(Flow::Failure(_))));
                drop(outcome);
            } else {
                let Ok(Value::CheckedString(value)) = outcome else {
                    panic!("result was not its physical owner");
                };
                assert_eq!(value.as_str(), expected);
                let view = value.borrow();
                drop(value);
                assert!(lifetime.upgrade().is_some());
                assert_eq!(view.as_ref(), expected.as_bytes());
                drop(view);
            }
            assert!(lifetime.upgrade().is_none());
            drop(lifetime);
        });
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
#[test]
fn provider_receiving_rejects_shared_wrong_size_and_wrong_bytes_and_keeps_first_refusal() {
    let source = crate::parse(SOURCE, "checked-interpreter-provider.spx").unwrap();
    let owner = received(&source, false);
    let ((), observed) = crate::test_allocation_observer::measure(|| {
        let provider = Inventory::acquire(&[], 0);
        let mut invocation = Invocation::new(owner.program(), provider.clone());
        let before = crate::test_allocation_observer::current_metrics().allocated;
        let first = invocation.materialize(&["a"]);
        assert!(matches!(&first, Err(Flow::Failure(status)) if status.code() == 1));
        assert!(matches!(
            invocation.materialize(&["β"]),
            Err(Flow::Guard(_))
        ));
        assert_eq!(invocation.physical, Some(AllocationCase::PayloadRefused));
        assert_eq!(provider.requests.load(Ordering::SeqCst), 1);
        assert_eq!(
            crate::test_allocation_observer::current_metrics().allocated,
            before
        );
        drop(first);
        drop(invocation);
        drop(provider);
    });
    assert!(!observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    #[derive(Debug)]
    struct Bad {
        backing: Arc<[u8]>,
        shared: bool,
    }
    impl Utf8Provider for Bad {
        fn allocate(&self, _: &[&str]) -> Result<Arc<[u8]>, AllocationCase> {
            if self.shared {
                Ok(self.backing.clone())
            } else {
                Ok(Arc::from(self.backing.as_ref()))
            }
        }
    }
    // Hostile providers are deliberately outside the prefunded protocol; the
    // receiver must reject them before exposing an owner, even when UTF8 valid.
    for (bytes, shared) in [(&b"a"[..], true), (&b""[..], false), (&b"b"[..], false)] {
        let mut invocation = Invocation::new(
            owner.program(),
            Arc::new(Bad {
                backing: Arc::from(bytes),
                shared,
            }),
        );
        assert!(matches!(
            invocation.materialize(&["a"]),
            Err(Flow::Guard(_))
        ));
    }
}

#[test]
fn checked_interpreter_fuel_and_late_argument_failure_preserve_original_order_and_settlement() {
    let source = crate::parse(SOURCE, "checked-interpreter-fuel.spx").unwrap();
    let owner = received(&source, false);
    let projection = Projection::receive(&owner).unwrap();
    let mut exhausted = false;
    let mut completed = false;
    let public_zero =
        super::super::evaluate_resolved_zero_arg_i64(owner.program(), "checked.native.main", 0)
            .unwrap_err();
    assert_eq!(public_zero[0].code, "SPX-F101");
    for fuel in 0..64 {
        let old = ordinary(&owner, fuel);
        let ((), observed) = crate::test_allocation_observer::measure(|| {
            let provider = Inventory::acquire(&["a", "β", "aβ", "aβ"], usize::MAX);
            let outcome = projection.run(provider.clone(), fuel);
            match old {
                Ok(Value::Int(3)) => {
                    completed = true;
                    assert!(matches!(outcome, Ok(Value::Int(3))));
                }
                Err(Flow::Exhausted) => {
                    exhausted = true;
                    assert!(matches!(outcome, Err(Flow::Exhausted)));
                }
                _ => panic!("saved ordinary fuel oracle did not follow its original contract"),
            }
            assert!(provider.requests.load(Ordering::SeqCst) <= 4);
            drop(outcome);
            drop(provider);
        });
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    assert!(exhausted && completed);
    const LATE: &str = r#"module checked.native;
@id("checked.native.outer") fn outer(text: string, number: i64) -> i64 { string_len(text) + number }
@id("checked.native.fail") fn fail(number: i64) -> i64 { 9 / number }
@id("checked.native.main") fn main() -> i64 {
    let text = "a";
    outer(string_take(text), fail(0))
}
"#;
    let source = crate::parse(LATE, "checked-interpreter-late.spx").unwrap();
    let owner = received(&source, false);
    let projection = Projection::receive(&owner).unwrap();
    let old = ordinary(&owner, 1000);
    let Err(Flow::Failure(expected)) = old else {
        panic!("saved late failure refused differently");
    };
    for refusal in [0, usize::MAX] {
        let ((), observed) = crate::test_allocation_observer::measure(|| {
            let provider = Inventory::acquire(&["a"], refusal);
            let outcome = projection.run(provider.clone(), 1000);
            let Err(Flow::Failure(status)) = &outcome else {
                panic!("late failure did not select a language status");
            };
            if refusal == 0 {
                assert_eq!(
                    status.domain_id(),
                    crate::cleanup_plan::ALLOCATION_DOMAIN_V1
                );
            } else {
                assert_eq!(status, &expected);
            }
            // The outer callee and its owning-read clone never execute.
            assert_eq!(provider.requests.load(Ordering::SeqCst), 1);
            drop(outcome);
            drop(provider);
        });
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}

#[test]
fn checked_contract_result_alias_requires_current_function_provider_and_protected_result() {
    const CONTRACT: &str = r#"module checked.native;
@id("checked.native.publish") fn publish() -> string ensures string_len(result) == 1 { "a" }
@id("checked.native.main") fn main() -> i64 { 0 }
"#;
    let source = crate::parse(CONTRACT, "checked-interpreter-contract.spx").unwrap();
    let owner = received(&source, true);
    let function = owner
        .program()
        .functions
        .iter()
        .find(|f| f.id.as_str() == "checked.native.publish")
        .unwrap();
    let foreign_function = function.clone();
    let provider = Inventory::acquire(&["a"], usize::MAX);
    let mut invocation = Invocation::new(owner.program(), provider);
    let value = invocation.materialize(&["a"]).unwrap();
    assert!(matches!(
        invocation.contract_result(&foreign_function, &value),
        Err(Flow::Guard(_))
    ));
    let foreign_provider = Inventory::acquire(&["a"], usize::MAX);
    let foreign_invocation = Invocation::new(owner.program(), foreign_provider);
    assert!(matches!(
        foreign_invocation.contract_result(function, &value),
        Err(Flow::Guard(_))
    ));
    let alias = invocation.contract_result(function, &value).unwrap();
    assert_eq!(alias.as_str(), "a");
    assert!(Arc::ptr_eq(&alias.backing.bytes, &value.backing.bytes));
    // Ordinary contract ownership forbids explicit consumption of result; the
    // checked factory must not substitute an alias to admit that source.
    let hostile = CONTRACT.replace("string_len(result)", "string_len(string_take(result))");
    let hostile = crate::parse(&hostile, "checked-interpreter-contract-hostile.spx").unwrap();
    assert!(
        crate::cleanup_plan::checked_allocation::prepare_string_result(
            &hostile,
            &DeclarationId::new("checked.native.publish")
        )
        .is_err()
    );
}

#[test]
fn checked_scalar_text_constructors_use_actual_provider_without_decimal_string_temporaries() {
    for (expression, expected, pieces) in [
        (
            "string_from_i64(-9223372036854775808)",
            "-9223372036854775808",
            &["-9223372036854775808"][..],
        ),
        ("string_from_i64(0)", "0", &["0"][..]),
        (
            "string_from_usize(18446744073709551615usize)",
            "18446744073709551615",
            &["18446744073709551615"][..],
        ),
        ("string_from_char('β')", "β", &["β"][..]),
        ("string_trim(\"  β\t\")", "β", &["  β\t", "β"][..]),
        ("string_slice(\"aβz\", 1, 3)", "β", &["aβz", "β"][..]),
    ] {
        let text = format!("module checked.native; @id(\"checked.native.publish\") fn publish()->string {{ {expression} }} @id(\"checked.native.main\") fn main()->i64 {{0}}");
        let source = crate::parse(&text, "checked-interpreter-constructor.spx").unwrap();
        let owner = received(&source, true);
        let projection = Projection::receive(&owner).unwrap();
        let functions: BTreeMap<_, _> = owner
            .program()
            .functions
            .iter()
            .map(|f| (f.id.as_str(), f))
            .collect();
        let entry = functions["checked.native.publish"];
        let mut old = Evaluator::new_prepared(
            FunctionLookup::Borrowed(&functions),
            BTreeMap::new(),
            &owner.program().declarations,
            1000,
            0,
            PreparedCancellation::Never,
        );
        let Ok(Value::String(reference)) = old.call_frame(entry, Vec::new(), 0) else {
            panic!("original constructor refused");
        };
        assert_eq!(reference, expected);
        for refused in 0..=pieces.len() {
            let ((), observed) = crate::test_allocation_observer::measure(|| {
                let provider = Inventory::acquire(pieces, refused);
                let outcome = projection.run(provider.clone(), 1000);
                if refused == pieces.len() {
                    let Ok(Value::CheckedString(value)) = &outcome else {
                        panic!("constructor did not publish checked owner");
                    };
                    assert_eq!(value.as_str(), expected);
                } else {
                    assert!(
                        matches!(&outcome, Err(Flow::Failure(status)) if status.domain_id() == crate::cleanup_plan::ALLOCATION_DOMAIN_V1)
                    );
                }
                assert_eq!(
                    provider.requests.load(Ordering::SeqCst),
                    if refused == pieces.len() {
                        pieces.len()
                    } else {
                        refused + 1
                    }
                );
                drop(outcome);
                drop(provider);
            });
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}
#[test]
fn checked_slice_semantic_refusal_precedes_fresh_payload_and_matches_original_full_status() {
    for (offsets, code) in [("-1, 2", 1), ("1, 2", 2)] {
        let text = format!("module checked.native; @id(\"checked.native.publish\") fn publish()->string {{ string_slice(\"β\", {offsets}) }} @id(\"checked.native.main\") fn main()->i64 {{0}}");
        let source = crate::parse(&text, "checked-interpreter-slice.spx").unwrap();
        let owner = received(&source, true);
        let projection = Projection::receive(&owner).unwrap();
        let functions: BTreeMap<_, _> = owner
            .program()
            .functions
            .iter()
            .map(|f| (f.id.as_str(), f))
            .collect();
        let mut old = Evaluator::new_prepared(
            FunctionLookup::Borrowed(&functions),
            BTreeMap::new(),
            &owner.program().declarations,
            1000,
            0,
            PreparedCancellation::Never,
        );
        let Err(Flow::Failure(expected)) =
            old.call_frame(functions["checked.native.publish"], Vec::new(), 0)
        else {
            panic!("original text failure did not select status");
        };
        assert_eq!(expected.code(), code);
        let ((), observed) = crate::test_allocation_observer::measure(|| {
            let provider = Inventory::acquire(&["β"], usize::MAX);
            let outcome = projection.run(provider.clone(), 1000);
            assert!(matches!(&outcome, Err(Flow::Failure(status)) if status == &expected));
            assert_eq!(provider.requests.load(Ordering::SeqCst), 1);
            drop(outcome);
            drop(provider);
        });
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
