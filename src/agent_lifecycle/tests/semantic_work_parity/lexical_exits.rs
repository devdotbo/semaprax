//! Real metered lexical contours; authored coverage, not a prior-run receipt.
use super::*;

const LEXICAL_BODY: &str = r#"
    let seed = [83u8, 87u8];
    let scratch = bytes_copy(array_as_slice(seed));
    let mut index = 0;
    let rounds = task.budget;
    while index < rounds {
        let inner = bytes_copy(array_as_slice(seed));
        index = index + 1;
        if index == 1 { continue; };
        if index == 3 { break; };
    }
    return Summary { tag: bytes_copy(array_as_slice(seed)), total: index, count: index };
"#;

fn lexical_source() -> String {
    let start = SOURCE.find("    let seed = [83u8, 87u8];").unwrap();
    let end = SOURCE[start..].find("\n}\n\n@id(\"app.main\")").unwrap() + start;
    format!("{}    lexical(task){}\n@id(\"test.stage_semantic_work.lexical\")\nfn lexical(task: own Task) -> Summary {{{LEXICAL_BODY}}}\n", &SOURCE[..start], &SOURCE[end..])
}

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn lexical_entry_and_iteration_fuel_exhaustion_preserve_exact_cleanup_and_reentry() {
    let source = lexical_source();
    let checked = crate::check(
        &source,
        std::path::Path::new("stage-lexical-semantic-work.spx"),
    )
    .expect("lexical semantic work fixture checks");
    let program = hir::resolve(&checked).expect("lexical semantic work fixture resolves");
    hir::validate(&program).expect("lexical semantic work fixture validates");
    let prepared = prepared(&program);
    let host = native_stage_host().expect("lexical fuel parity requires held clang");
    // Each case starts with the same ordinary meter invocation. No helper or
    // corpus path refunds/reinitializes its shared fuel. Repeated successes
    // after exhaustion exercise fresh-invocation settlement and reentry.
    for (limit, fuel, exhausted, finalizers) in [
        (1, 1, true, 1),
        (2, 2, true, 2),
        (3, 3, true, 3),
        (5, 5, false, 5),
        (4, 4, true, 4),
        (1000, 5, false, 5),
    ] {
        let args = arguments(4, 1, 100);
        let expected = metered(
            authorization::StageBackend::Interpreter,
            &program,
            &prepared,
            &args,
            limit,
        );
        assert_eq!(work(&expected).fuel_used, fuel);
        assert_eq!(work(&expected).exhausted, exhausted);
        if exhausted {
            assert_eq!(expected.outcome, RetainedCallOutcome::FuelExhausted);
        } else {
            let RetainedCallOutcome::Returned(RetainedValue::Record(summary)) = &expected.outcome
            else {
                panic!("lexical success did not return its checked Summary");
            };
            assert_eq!(summary.record.as_str(), "test.stage_semantic_work.Summary");
            let fields = summary
                .fields
                .iter()
                .map(|field| (field.field.as_str(), &field.value))
                .collect::<std::collections::BTreeMap<_, _>>();
            assert_eq!(summary.fields.len(), 3);
            assert_eq!(fields.len(), 3);
            assert_eq!(
                fields.get("test.stage_semantic_work.Summary.tag"),
                Some(&&RetainedValue::Bytes(vec![83, 87]))
            );
            assert_eq!(
                fields.get("test.stage_semantic_work.Summary.total"),
                Some(&&RetainedValue::I64(3))
            );
            assert_eq!(
                fields.get("test.stage_semantic_work.Summary.count"),
                Some(&&RetainedValue::I64(3))
            );
        }
        let mut first = None;
        for (leg, backend) in legs_on(&host, &source) {
            let actual = metered(backend, &program, &prepared, &args, limit);
            assert_eq!(
                comparable(&actual),
                comparable(&expected),
                "limit {limit}: {leg}"
            );
            let events = work(&actual)
                .finalizer_events
                .as_ref()
                .expect("compiled backend reports performed finalizers");
            assert_eq!(
                events.len(),
                finalizers,
                "limit {limit}: {leg} exact performed inventory"
            );
            assert!(events
                .iter()
                .all(|event| event.function.as_str() == "test.stage_semantic_work.lexical"));
            if let Some(first) = &first {
                assert_eq!(
                    events, first,
                    "limit {limit}: {leg} canonical runtime order"
                );
            } else {
                first = Some(events.clone());
            }
        }
    }
}
