use std::collections::BTreeMap;
use std::mem::size_of;

use crate::hir::{DeclarationId, Place, PlaceProjection, ValueId};
use crate::loan_plan::{WorkCounter, MAX_LOAN_PLAN_WORK_V1};

fn fixture(count: usize) -> (BTreeMap<ValueId, Place>, Place) {
    let aliases = (0..count)
        .map(|index| {
            (
                ValueId::intrinsic_parameter("origin", index),
                Place {
                    root: ValueId::intrinsic_parameter("origin", index + 1),
                    projections: vec![PlaceProjection::VariantField {
                        case: DeclarationId::new(format!("case.é.{index}")),
                        field: DeclarationId::new(format!("field.\0.{index}")),
                    }],
                },
            )
        })
        .collect();
    (
        aliases,
        Place {
            root: ValueId::intrinsic_parameter("origin", 0),
            // This models the actual synthetic assignment-field receiver, rather
            // than assuming every initial place appears as a HIR Place node.
            projections: vec![
                PlaceProjection::Field(DeclarationId::new("assignment.field")),
                PlaceProjection::Field(DeclarationId::new("nested.field")),
            ],
        },
    )
}

#[test]
fn borrowed_alias_materialization_matches_original_order_and_linear_requested_backing() {
    for count in [0usize, 1, 8, 128] {
        let (aliases, initial) = fixture(count);
        let old_input = initial.clone();
        let mut old_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        let (old, old_observation) = crate::test_allocation_observer::measure(|| {
            super::original(&aliases, old_input, &mut old_work).unwrap()
        });
        let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        let (new, observed) = crate::test_allocation_observer::measure(|| {
            super::resolve(&aliases, initial, &mut work).unwrap()
        });
        assert_eq!(new, old);
        assert!(!observed.invalid);
        assert!(!old_observation.invalid);
        assert_eq!(old_work.used, count);
        assert_eq!(work.used, 3 * count);
        // Independently sum the single exact output vector, each cloned
        // alias ID once, and a conservative reference-only B-tree envelope.
        // No projection-bearing String/Vec clone is paid per prefix length.
        let text = (0..count)
            .map(|index| format!("case.é.{index}").len() + format!("field.\0.{index}").len())
            .sum::<usize>();
        let seen = if count == 0 {
            0
        } else {
            (count / 5 + 2) * (11 * size_of::<&ValueId>() + 16 * size_of::<usize>())
        };
        let output = if count == 0 {
            0
        } else {
            (count + 2) * size_of::<PlaceProjection>() + text
        };
        assert!(observed.allocated <= output + seen);
        assert!(observed.highwater <= output + seen);
        if count == 128 {
            assert!(observed.allocated < old_observation.allocated);
        }
    }
}

#[test]
fn alias_materialization_precharge_has_exact_short_and_sticky_work_and_cycle_refusals() {
    let (aliases, initial) = fixture(8);
    let text = (0..8)
        .map(|index| format!("case.é.{index}").len() + format!("field.\0.{index}").len())
        .sum::<usize>();
    let required = size_of::<Vec<PlaceProjection>>() + 10 * size_of::<PlaceProjection>() + text;
    let (result, overflowed, used) = crate::bounded_output::with_limit_usage(required, || {
        let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        super::resolve(&aliases, initial.clone(), &mut work)
    });
    assert!(!overflowed);
    assert_eq!(used, required);
    let exact = result.unwrap();
    let (result, overflowed, used) = crate::bounded_output::with_limit_usage(required - 1, || {
        let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        let refused = super::resolve(&aliases, initial.clone(), &mut work);
        // No-alias receives no fresh allocation but cannot reopen a refused
        // active phase. Preserve the same budget and check before activity.
        assert!(super::resolve(&BTreeMap::new(), initial.clone(), &mut work).is_err());
        refused
    });
    assert!(overflowed);
    assert_eq!(used, 0);
    assert_eq!(
        result.unwrap_err().message,
        "loan alias projection exceeds its active allocation budget"
    );
    // Construction Drop does not replenish any physical reservation.
    let ((), overflowed, used) = crate::bounded_output::with_limit_usage(required, || {
        let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
        drop(super::resolve(&aliases, initial.clone(), &mut work).unwrap());
        assert!(!crate::bounded_output::reserve_active_required(1));
    });
    assert!(overflowed);
    assert_eq!(used, required);

    for limit in [0usize, 7, 8, 15, 23] {
        let mut work = WorkCounter::new(limit);
        assert_eq!(
            super::resolve(&aliases, initial.clone(), &mut work)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
    let mut cyclic = aliases.clone();
    cyclic.insert(
        exact.root,
        Place {
            root: initial.root.clone(),
            projections: Vec::new(),
        },
    );
    let mut new_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
    let mut old_work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
    assert_eq!(
        super::resolve(&cyclic, initial.clone(), &mut new_work)
            .unwrap_err()
            .message,
        super::original(&cyclic, initial.clone(), &mut old_work)
            .unwrap_err()
            .message
    );
    assert_eq!(new_work.used, old_work.used + 8);
    let mut exact_work = WorkCounter::new(24);
    assert!(super::resolve(&aliases, initial.clone(), &mut exact_work).is_ok());
    assert_eq!(exact_work.used, 24);
}
