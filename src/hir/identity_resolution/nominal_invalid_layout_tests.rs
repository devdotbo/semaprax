//! Additive full-constructor control for the semantic T217 publication boundary.
use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Stop(usize);

#[test]
fn nominal_invalid_recursive_layout_keeps_old_report_and_funds_every_refusal_and_drop() {
    const PATH: &str = "nominal-invalid-layout.spx";
    const SOURCE: &str = r#"module nominal.invalid_layout;
@id("recursive.type") record Recursive {
    @id("recursive.next") next:Recursive,
}
@id("app.main") fn main()->i64 {0}
"#;
    // Parsing deliberately reaches the actual index layout gate. Source
    // verification would publish this diagnostic before the constructor owner.
    let program = crate::parse(SOURCE, PATH).unwrap();
    let prelude = crate::prelude::declarations_for_program(&program);
    let old = DeclarationIndex::nominal_original_with_prelude(&program, prelude).unwrap_err();
    let expected = Diagnostic::error(
        "SPX-T217",
        "record declarations contain an illegal by-value recursive layout",
        crate::ast::Span::default(),
    )
    .at_path(PATH);
    assert_eq!(old, expected, "complete independent old six-field receipt");

    let mut requests = Vec::new();
    let (result, overflowed, exact, first) = with_limit_usage_refusal(usize::MAX, || {
        DeclarationIndex::from_verified_with_nominal_reservation(&program, prelude, |n| {
            requests.push(n);
            if reserve_active_required(n) {
                Ok(())
            } else {
                Err(Stop(7))
            }
        })
    });
    assert!(!overflowed);
    assert_eq!(first, None);
    assert!(exact > 0);
    assert!(!requests.is_empty());
    let IdentityIndexError::Semantic(current) = result.unwrap_err() else {
        panic!("unlimited owner unexpectedly refused")
    };
    assert_eq!(current, old);
    drop(current);

    // Every callback includes the actual final diagnostic message/path
    // acquisition; no successful semantic report can hide a refused producer.
    for stop in 0..requests.len() {
        let mut seen = 0;
        let mut at_refusal = 0;
        let (result, allocation) = measure(|| {
            DeclarationIndex::from_verified_with_nominal_reservation(&program, prelude, |n| {
                assert_eq!(n, requests[seen]);
                let at = seen;
                seen += 1;
                if at == stop {
                    at_refusal = current_metrics().allocated;
                    Err(Stop(at))
                } else {
                    Ok(())
                }
            })
        });
        assert!(matches!(result, Err(IdentityIndexError::Identity(Stop(at))) if at == stop));
        assert_eq!(seen, stop + 1, "no callback after selected raw error");
        assert!(!allocation.invalid);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert_eq!(
            allocation.allocated, at_refusal,
            "no allocation after refusal, including partial Drop"
        );
    }

    for cap in [0, exact - 1, exact] {
        let ((result, allocation), overflowed, used, first) = with_limit_usage_refusal(cap, || {
            measure(|| {
                DeclarationIndex::from_verified_with_nominal_reservation(&program, prelude, |n| {
                    if reserve_active_required(n) {
                        Ok(())
                    } else {
                        Err(Stop(7))
                    }
                })
            })
        });
        assert!(!allocation.invalid);
        if cap == exact {
            assert!(!overflowed);
            assert_eq!(first, None);
            assert_eq!(used, exact);
            assert!(
                allocation.allocated <= used,
                "actual backing={allocation:?}; paid={used}"
            );
            let IdentityIndexError::Semantic(current) = result.unwrap_err() else {
                panic!("exact owner unexpectedly refused")
            };
            assert_eq!(current, old);
            let ((), disposal) = measure(|| drop(current));
            assert!(!disposal.invalid);
            assert_eq!(
                disposal.allocated, 0,
                "semantic report final Drop allocates nothing"
            );
        } else {
            assert!(matches!(result, Err(IdentityIndexError::Identity(Stop(7)))));
            assert!(overflowed);
            assert!(first.is_some());
            assert_eq!(allocation.live, 0);
            assert_eq!(allocation.allocated, allocation.freed);
            if cap == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
    // Keep report allocation and its complete final Drop in one registry.
    // A separate Drop-only observation cannot establish those original frees.
    let (((), allocation), overflowed, used, first) = with_limit_usage_refusal(exact, || {
        measure(|| {
            let result =
                DeclarationIndex::from_verified_with_nominal_reservation(&program, prelude, |n| {
                    if reserve_active_required(n) {
                        Ok(())
                    } else {
                        Err(Stop(7))
                    }
                });
            let IdentityIndexError::Semantic(current) = result.unwrap_err() else {
                panic!("exact complete owner unexpectedly refused")
            };
            assert_eq!(current, old);
            drop(current);
        })
    });
    assert!(!overflowed);
    assert_eq!(first, None);
    assert_eq!(used, exact);
    assert!(!allocation.invalid);
    assert!(allocation.allocated <= used);
    assert_eq!(allocation.live, 0);
    assert_eq!(allocation.allocated, allocation.freed);

    let ((result, allocation), overflowed, used, first) = with_limit_usage_refusal(0, || {
        assert!(!reserve_active_required(1));
        measure(|| {
            DeclarationIndex::from_verified_with_nominal_reservation(&program, prelude, |n| {
                if reserve_active_required(n) {
                    Ok(())
                } else {
                    Err(Stop(9))
                }
            })
        })
    });
    assert!(matches!(result, Err(IdentityIndexError::Identity(Stop(9)))));
    assert!(overflowed);
    assert_eq!(used, 0);
    assert_eq!(first.unwrap().requested, 1);
    assert!(!allocation.invalid);
    assert_eq!(allocation.allocated, 0);
    assert_eq!(allocation.live, 0);
}
