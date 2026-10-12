//! Actual interpreter backing for immutable borrowed `Slice<u8>` traversal.
//!
//! The same owned root is traversed, or only measured, at two lengths. A view
//! or buffer copy would raise the traversal's peak by the root length; the
//! traversal's peak excess over its control must instead be identical at both
//! lengths. Every runtime allocation, including the owned root, is released by
//! the end of evaluation: the root drops once and nothing outlives it.
use std::path::Path;

use super::closures::checked_functions;
use super::prepared::PreparedCancellation;
use super::{admitted_resolved_functions, Evaluator, FunctionLookup, Value};
use crate::test_allocation_observer::{measure, Metrics};

fn source(length: usize, traverse: bool) -> String {
    let walk = if traverse {
        "for byte in view { total = total + i64_from_u8(byte); 0 }"
    } else {
        "let length = byte_len(view);\n    if length == 0usize { total = 1; 0 } else { 0 };"
    };
    format!(
        "module app.byte_traversal_peak;\n\n@id(\"app.main\")\nfn main() -> i64 {{\n    let data = [7u8; {length}];\n    let owned = bytes_copy(array_as_slice(data));\n    let view = bytes_as_slice(owned);\n    let mut total = 0;\n    {walk}\n    total\n}}\n"
    )
}

fn run(length: usize, traverse: bool) -> (i64, Metrics) {
    let ast = crate::parse(
        &source(length, traverse),
        Path::new("byte-traversal-peak.spx"),
    )
    .unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    let admitted = admitted_resolved_functions(&program);
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(super::EVALUATION_STACK_BYTES)
            .spawn_scoped(scope, || {
                let closure_functions = checked_functions(&program).unwrap();
                let (value, metrics) = measure(|| {
                    let mut evaluator = Evaluator::new_prepared(
                        FunctionLookup::Borrowed(&admitted),
                        closure_functions,
                        &program.declarations,
                        10_000_000,
                        0,
                        PreparedCancellation::Never,
                    );
                    let value = evaluator.call_frame(entry, Vec::new(), 0);
                    drop(evaluator);
                    value
                });
                let Ok(Value::Int(value)) = value else {
                    panic!("byte traversal peak program did not return i64")
                };
                (value, metrics)
            })
            .unwrap()
            .join()
            .unwrap()
    })
}

#[test]
fn byte_traversal_copies_no_buffer_and_drops_its_owned_root() {
    let mut excess = Vec::new();
    for length in [256usize, 16_384] {
        let (walked, traversal) = run(length, true);
        let (measured, control) = run(length, false);
        assert_eq!(walked, 7 * length as i64);
        assert_eq!(measured, 0);
        for metrics in [traversal, control] {
            assert!(!metrics.invalid);
            assert_eq!(
                metrics.live, 0,
                "{length}: runtime backing outlived evaluation"
            );
            assert_eq!(metrics.freed, metrics.allocated);
            // The array and its owned copy are each live at the peak.
            assert!(metrics.highwater >= 2 * length);
        }
        excess.push(traversal.highwater as i128 - control.highwater as i128);
    }
    assert_eq!(
        excess[0], excess[1],
        "traversal peak grows with the root length"
    );
    assert!(excess[1] < 16_384);
}
