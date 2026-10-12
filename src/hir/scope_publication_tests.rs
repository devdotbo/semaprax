//! Unchanged publication storage only; initial and changed snapshots remain owned.
use super::*;
use crate::test_allocation_observer::{current_metrics, measure, Metrics};
#[path = "scope_publication_tests/old.rs"]
mod old;

fn scope(count: usize) -> BTreeMap<ValueId, ValidationBinding> {
    (0..count)
        .map(|index| {
            (
                ValueId::new(format!("publication.binding.{index}")),
                ValidationBinding {
                    contract_local: false,
                    mutable: false,
                    ty: ResolvedType::Nominal {
                        declaration: DeclarationId::new(format!("publication.record.{index}")),
                        arguments: Vec::new(),
                    },
                    ownership: OwnershipMode::Own,
                    availability: Availability::Available,
                    moved_places: BTreeMap::new(),
                    definitely_partial: BTreeSet::new(),
                    active_loans: BTreeSet::new(),
                },
            )
        })
        .collect()
}

fn observed(count: usize, repeats: usize, old: bool) -> Metrics {
    let ((), metrics) = measure(|| {
        // Both complete observations include original scopes, initial snapshot,
        // all publication attempts and every owned Drop. Observer metadata only
        // is excluded. There is no builder or physical-payment claim here.
        let current = scope(count);
        let mut target = BTreeMap::new();
        if old {
            let mut publication = old::Publication {
                target: &mut target,
                published: current.clone(),
                enabled: true,
            };
            for _ in 0..repeats {
                publication.publish(&current);
            }
            drop(publication);
        } else {
            let mut publication = ValidationScopePublication {
                target: &mut target,
                published: current.clone(),
                enabled: true,
            };
            let before = current_metrics();
            for _ in 0..repeats {
                publication.publish(&current);
            }
            assert_eq!(current_metrics(), before);
            drop(publication);
        }
        assert_eq!(target, current);
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    metrics
}

#[test]
fn exact_unchanged_publication_avoids_recloning_wide_scopes_and_preserves_every_state_field() {
    for count in [1, 64] {
        let baseline = observed(count, 0, false);
        let actual = observed(count, 128, false);
        let legacy = observed(count, 128, true);
        assert_eq!(actual, baseline);
        // Each old publication clones each literal nominal declaration String.
        // This independent lower census does not estimate BTree node layouts.
        assert!(legacy.allocations >= baseline.allocations + count * 128);
        assert!(legacy.allocated > actual.allocated);
    }
    let mut current = scope(1);
    let key = current.keys().next().unwrap().clone();
    let mut actual = BTreeMap::new();
    let mut expected = BTreeMap::new();
    {
        let mut publication = ValidationScopePublication {
            target: &mut actual,
            published: current.clone(),
            enabled: true,
        };
        let mut reference = old::Publication {
            target: &mut expected,
            published: current.clone(),
            enabled: true,
        };
        for mutation in 0..9 {
            let binding = current.get_mut(&key).unwrap();
            match mutation {
                0 => binding.contract_local = true,
                1 => binding.mutable = true,
                2 => binding.ty = ResolvedType::I64,
                3 => binding.ownership = OwnershipMode::Borrow,
                4 => binding.availability = Availability::Moved,
                5 => {
                    binding.moved_places.insert(
                        vec![PlaceProjection::Field(DeclarationId::new("moved.field"))],
                        Availability::MaybeMoved,
                    );
                }
                6 => {
                    binding
                        .definitely_partial
                        .insert(vec![PlaceProjection::Field(DeclarationId::new(
                            "partial.field",
                        ))]);
                }
                7 => {
                    binding.active_loans.insert(LoanId(7));
                }
                _ => {
                    current.insert(
                        ValueId::new("new.binding".into()),
                        scope(1).into_values().next().unwrap(),
                    );
                }
            }
            publication.publish(&current);
            reference.publish(&current);
            assert_eq!(publication.published, reference.published);
            assert_eq!(publication.published, current);
        }
        publication.enabled = false;
        reference.enabled = false;
        current.clear();
        publication.publish(&current);
        reference.publish(&current);
        assert_eq!(publication.published, reference.published);
        assert!(!publication.published.is_empty());
        publication.enabled = true;
        reference.enabled = true;
        publication.publish(&current);
        reference.publish(&current);
        assert!(publication.published.is_empty());
        assert_eq!(publication.published, reference.published);
    }
    assert_eq!(actual, expected);
}
