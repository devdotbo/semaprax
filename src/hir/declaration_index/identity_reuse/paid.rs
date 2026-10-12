//! Actual initial identity requests only; other index backing and original
//! owned semantic diagnostics retain their existing unfunded compatibility.
use super::*;
use crate::cleanup_plan::ReplayPhysicalReservation;
use crate::hir::declaration_index::identity_payer::IdentityIndexError;

const INITIAL: [&str; 17] = [
    "token.type",
    "row.type",
    "base.type",
    "child.type",
    "choice.type",
    "token.drop",
    "app.main",
    "row.x",
    "base.x",
    "child.y",
    "base.x",
    "base.read",
    "child.y",
    "child.read",
    "choice.left",
    "choice.left.x",
    "choice.right",
];
fn backing(text: &str) -> usize {
    let word = std::mem::size_of::<usize>();
    (2 * word + text.len()).div_ceil(word) * word
}
#[test]
fn initial_identity_reservations_preserve_full_saved_index_and_literal_payload_drop() {
    let program = crate::check(SOURCE, "identity-reuse.spx").unwrap();
    let expected = DeclarationIndex::from_verified_with_prelude(&program, &[]).unwrap();
    let bytes = INITIAL.iter().map(|text| backing(text)).sum::<usize>();
    let mut physical = ReplayPhysicalReservation::new(bytes);
    let mut count = 0;
    let (index, observed) = measure(|| {
        DeclarationIndex::from_verified_with_identity_reservation(&program, &[], |requested| {
            assert_eq!(requested, backing(INITIAL[count]));
            count += 1;
            physical.reserve(requested)
        })
        .unwrap()
    });
    assert_eq!(index, expected);
    assert_eq!(count, 17);
    assert_eq!(physical.charged(), bytes);
    drop(index);
    // Full index allocations are compared through Drop, not called paid by
    // the identity-only ledger. The 17 removed source String clones are the
    // exact difference; Arc/text/map/field allocations remain observed.
    let (_, old) =
        measure(|| drop(DeclarationIndex::from_verified_with_prelude(&program, &[]).unwrap()));
    let mut physical = ReplayPhysicalReservation::new(bytes);
    let (_, new) = measure(|| {
        drop(
            DeclarationIndex::from_verified_with_identity_reservation(&program, &[], |requested| {
                physical.reserve(requested)
            })
            .unwrap(),
        )
    });
    assert_eq!(old.allocations, new.allocations + 17);
    assert_eq!(old.reallocations, new.reallocations);
    assert_eq!(
        old.allocated,
        new.allocated + INITIAL.iter().map(|text| text.len()).sum::<usize>()
    );
    for metrics in [old, new] {
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
    // The successful constructor owns the backing measured above; it was
    // dropped outside that first scope, so no all-settled assertion is made.
    assert!(!observed.invalid);
    assert!(observed.live > 0);
}
#[test]
fn initial_identity_refusal_returns_original_copy_and_stops_before_next_request() {
    let program = crate::check(SOURCE, "identity-reuse.spx").unwrap();
    for accepted in [0, 8, 16] {
        let prefix = INITIAL[..accepted]
            .iter()
            .map(|text| backing(text))
            .sum::<usize>();
        let mut physical = ReplayPhysicalReservation::new(prefix + backing(INITIAL[accepted]) - 1);
        let mut requests = 0;
        let (refusal, observed) = measure(|| {
            let error =
                DeclarationIndex::from_verified_with_identity_reservation(&program, &[], |bytes| {
                    assert_eq!(bytes, backing(INITIAL[requests]));
                    requests += 1;
                    physical.reserve(bytes)
                })
                .err()
                .unwrap();
            let IdentityIndexError::Identity(first) = error else {
                panic!("identity refusal was rendered")
            };
            assert_eq!(physical.reserve(0), Err(first));
            assert_eq!(physical.reserve(1), Err(first));
            first
        });
        assert_eq!(requests, accepted + 1);
        assert_eq!(physical.charged(), prefix);
        assert_eq!(physical.reserve(0), Err(refusal));
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    // Invalid text is not granted verifier authority by a reservation. The
    // direct compatibility constructor still agrees on its original behavior.
    for text in ["", "nul\0identity", "Røw.雪", "choice.type"] {
        let mut changed = program.clone();
        changed.types[1].stable_id = text.to_owned();
        let old = DeclarationIndex::from_verified_with_prelude(&changed, &[]);
        let mut physical = ReplayPhysicalReservation::new(usize::MAX);
        let paid =
            DeclarationIndex::from_verified_with_identity_reservation(&changed, &[], |bytes| {
                physical.reserve(bytes)
            });
        match (old, paid) {
            (Ok(old), Ok(paid)) => assert_eq!(old, paid),
            (Err(old), Err(IdentityIndexError::Semantic(paid))) => assert_eq!(old, paid),
            _ => panic!("paid identity changed legacy meaning"),
        }
        assert_eq!(
            crate::verify::verify(&changed),
            crate::verify::verify(&changed.clone())
        );
    }
}
