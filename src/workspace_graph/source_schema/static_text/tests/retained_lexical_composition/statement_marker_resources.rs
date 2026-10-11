//! Actual receiving work/backing, separate from parser limits and cache bytes.
use super::*;

fn observed(
    retained: &WorkspaceGraphBuild,
    members: &[&hir::ResolvedFunction],
    work: usize,
    bytes: usize,
) -> (
    (bool, (usize, usize), bool),
    crate::test_allocation_observer::Metrics,
) {
    crate::test_allocation_observer::measure(|| {
        let (results, metrics, finished) = replay(retained, members, work, bytes).unwrap();
        let successful = results.iter().all(Result::is_ok);
        // Include diagnostics, preparation, index/session and Drop; retain no
        // heap-owned observation/report after the measured closure returns.
        drop(results);
        (successful, metrics, finished)
    })
}

#[test]
fn scalar_discard_and_terminated_guards_share_actual_replay_work_and_backing_limits() {
    // Retain the original append-only compose body verbatim. Whole String
    // replacement is intentionally outside this witness and its pending seam.
    let compose = STRING_SOURCE
        .split("@id(\"static.replace\")")
        .next()
        .unwrap();
    let observe = compose
        .strip_prefix("module static.writer;\n")
        .unwrap()
        .replace("static.compose", "static.observe")
        .replace("fn compose", "fn observe");
    let source =
        format!("{compose}{observe} @id(\"app.main\") fn main()->i64 {{compose(4)+observe(4)}}");
    assert_eq!(source.matches("at=at+1;").count(), 2);
    let selected = source.replace("at=at+1;", "at=at+1; 5; false && 1/0>0;");
    // An independently authored ordinary-let/unterminated-guard control.
    let control = source
        .replace("at=at+1;", "at=at+1; let probe=5; let lazy=false && 1/0>0;")
        .replace("{continue;};", "{continue;}")
        .replace("str_len_bytes(key);};", "str_len_bytes(key);}");
    for source in [&selected, &control] {
        let ordinary = direct(source);
        let retained = build(source);
        admitted(&retained);
        plan_parity(writer(&retained), &ordinary);
        runtime_parity(&retained, &ordinary, Some(10));
        let members = lexical_members(writer(&retained));
        assert_eq!(members.len(), 2);
        for member in &members {
            assert!(member.cleanup_plan.lexical_control.is_some());
        }
        let ((successful, (remaining, charged), finished), allocation) =
            observed(&retained, &members, WIDE_WORK, WIDE_BYTES);
        assert!(successful && finished);
        let spent = WIDE_WORK - remaining;
        assert!(spent > 0 && charged > 0);
        assert!(!allocation.invalid);
        assert_eq!(allocation.live, 0);
        assert!(allocation.allocated <= charged);
        assert!(allocation.highwater <= charged);
        let ((successful, exact, finished), allocation) =
            observed(&retained, &members, spent, charged);
        assert!(successful && finished);
        assert_eq!(exact, (0, charged));
        assert!(!allocation.invalid);
        assert_eq!(allocation.live, 0);
        assert!(allocation.allocated <= charged);
        for (work, bytes) in [(spent - 1, charged), (spent, charged - 1)] {
            let ((successful, (_, debit), finished), allocation) =
                observed(&retained, &members, work, bytes);
            assert!(!successful && !finished);
            assert!(debit <= bytes);
            assert!(!allocation.invalid);
            assert_eq!(allocation.live, 0);
            assert!(allocation.allocated <= debit);
            assert!(allocation.highwater <= debit);
            // replay() also asserts H006 and unchanged metrics on retry after
            // refusal, using this same Preparation/Session, without a reset.
        }
    }
}
