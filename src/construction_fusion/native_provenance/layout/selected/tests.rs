//! Authored owning layout and separate reference-protocol definitions.
//! Neither arithmetic nor a reference receipt grants execution authority.
use super::funding::{
    self, Account, Boundary, HeldInvocation, HeldIssuer, Kind, Ledger, Ordinary, SettledRows,
};
use super::retention::Operation as Op;
use super::*;

/// The native lowering's census for the original fixture: one staged row,
/// three row Vec headers and one view (`placement_items() == [1, 3, 1]`).
const FIXTURE: Placement = Placement {
    staged: 1,
    rows: 3,
    views: 1,
};
/// Push, then the source owner events (move, borrow, sort), then the passive
/// settlement marker. Carrier-model data only, never cleanup permission.
const CLOSED: [Op; 5] = [
    Op::Push,
    Op::VecMove,
    Op::RowsBorrow,
    Op::VecSort,
    Op::PassiveVecSettlement,
];
const ORIGINAL: &str = include_str!("../../../../../tests/fixtures/record-traversal-maximum.spx");

fn request(rows: u32, width: u64, placement: Placement, operations: &[Op]) -> Request {
    Request::for_test(rows, width, placement, operations).unwrap()
}

fn layout(rows: u32) -> SelectedNativeLayout {
    derive(&request(rows, 3, FIXTURE, &CLOSED)).unwrap()
}

#[test]
fn rows_33_and_4096_have_exact_checked_native64_layouts() {
    assert_eq!(FIXTURE.bytes(), Some(40 + 3 * 56 + 48));
    for (rows, width, metadata, storage) in [
        (33u32, 3u64, 240 + 33 * 24 + 64 * 24 + 256, 33 * 32),
        (4096, 3, 240 + 4096 * 24 + 64 * 24 + 256, 4096 * 32),
        (4096, 32, 240 + 4096 * 24 + 64 * 24 + 256, 131_072),
    ] {
        let layout = derive(&request(rows, width, FIXTURE, &CLOSED)).unwrap();
        assert_eq!(layout.metadata(), metadata);
        assert_eq!(
            layout.metadata(),
            layout.header() + layout.slot_table() + layout.view_table() + layout.placement()
        );
        assert_eq!((layout.header(), layout.view_table()), (240, 64 * 24));
        assert_eq!(layout.slot_table(), rows as usize * 24);
        assert_eq!(layout.row_storage(), storage);
        assert_eq!(layout.extent(), (u64::from(rows) * width) as usize);
        // The semantic owned charge is an envelope fact, never row funding.
        assert_eq!(layout.semantic_owned(), u64::from(rows) * 32);
        assert_eq!(layout.semantic_scalar(), u64::from(rows) * 8);
        assert_eq!(layout.max_slot_generation(), 2);
        assert_eq!(layout.final_vec_generation(), 1 + u64::from(rows) + 2);
        assert_eq!(layout.nonces_through_completion(), 1 + 2 * u64::from(rows));
        assert_eq!(layout.destination(), &test_destination());
    }
    assert_eq!(metadata_bytes(4096, 0), Some(240 + 4096 * 24 + 64 * 24));
    assert_eq!(
        (
            SelectedNativeLayout::owner_carrier(),
            SelectedNativeLayout::stored_owner(),
            SelectedNativeLayout::view_carrier(),
            SelectedNativeLayout::row_stride()
        ),
        (24, 16, 48, 32)
    );
}

#[test]
fn rows_width_extent_placement_and_overflow_refuse() {
    for (rows, width, refusal) in [
        (32, 3, SelectedRefusal::Rows),
        (4097, 3, SelectedRefusal::Rows),
        (33, 0, SelectedRefusal::Width),
        (4096, 33, SelectedRefusal::Extent),
        (33, u64::MAX, SelectedRefusal::Extent),
    ] {
        assert_eq!(
            derive(&request(rows, width, FIXTURE, &CLOSED)),
            Err(refusal)
        );
    }
    assert!(derive(&request(4096, 32, FIXTURE, &CLOSED)).is_ok());
    for placement in [
        Placement {
            staged: usize::MAX,
            ..FIXTURE
        },
        Placement {
            rows: usize::MAX / 56 + 1,
            ..FIXTURE
        },
        Placement {
            views: MAX_PLACEMENT / 48 + 1,
            ..Placement::default()
        },
    ] {
        assert_eq!(
            derive(&request(64, 3, placement, &CLOSED)),
            Err(SelectedRefusal::Placement)
        );
    }
    let ceiling = Placement {
        views: MAX_PLACEMENT / 48,
        ..Placement::default()
    };
    assert!(derive(&request(64, 3, ceiling, &CLOSED)).is_ok());
    assert_eq!(metadata_bytes(0, 0), None);
    assert_eq!(metadata_bytes(4097, 0), None);
    assert_eq!(metadata_bytes(33, MAX_PLACEMENT + 1), None);
    // Independent native64 ABI: current domain header, exact Slot and Borrow
    // tables, then the remaining inline-frame placement envelope. The owning
    // cross-source C compiler gate independently checks every size/offset.
    assert_eq!(MAX_PLACEMENT, 65_536 - 240);
    assert_eq!(
        metadata_bytes(4096, MAX_PLACEMENT),
        Some(240 + 4096 * 24 + 64 * 24 + MAX_PLACEMENT)
    );
}

#[test]
fn row_storage_is_reserved_at_the_destination_on_the_same_physical_ledger() {
    let layout = layout(4096);
    let plan = layout.plan();
    assert_eq!(
        plan.map(|allocation| (allocation.kind, allocation.account, allocation.boundary)),
        [
            (
                Kind::RowStorage,
                Account::PhysicalLedger,
                Boundary::DestinationAllocation
            ),
            (
                Kind::SlotTable,
                Account::PhysicalLedger,
                Boundary::FirstBytesOperand
            ),
            (
                Kind::ViewTable,
                Account::PhysicalLedger,
                Boundary::FirstBytesOperand
            ),
            (
                Kind::Backing,
                Account::OrdinaryEvents,
                Boundary::FirstBytesOperand
            ),
        ]
    );
    assert_eq!(
        plan.map(|allocation| allocation.bytes),
        [131_072, 98_304, 1_536, 12_288]
    );
    let total = layout.row_storage() + layout.metadata();
    // One short at the destination: the Vec boundary itself refuses. No nonce,
    // receipt, metadata or ordinary debit follows; the refusal is sticky.
    let mut issuer = HeldIssuer::new();
    let mut ledger = Ledger::new(layout.row_storage() - 1);
    let mut ordinary = Ordinary::default();
    let mut held = HeldInvocation {
        issuer: &mut issuer,
        ledger: &mut ledger,
        ordinary: &mut ordinary,
    };
    assert_eq!(
        funding::reserve_rows(&layout, &mut held, layout.destination()).err(),
        Some(SelectedRefusal::RowsUnfunded)
    );
    assert_eq!((held.ledger.debited(), held.ledger.failed()), (0, true));
    assert_eq!(held.ordinary.events(), 0);
    assert_eq!(
        funding::reserve_rows(&layout, &mut held, layout.destination()).err(),
        Some(SelectedRefusal::RowsUnfunded)
    );
    // A different destination identity refuses before any debit.
    let mut issuer = HeldIssuer::new();
    let mut ledger = Ledger::new(total);
    let mut ordinary = Ordinary::default();
    let mut held = HeldInvocation {
        issuer: &mut issuer,
        ledger: &mut ledger,
        ordinary: &mut ordinary,
    };
    let main =
        crate::hir::FunctionExecutionId::Monomorphic(crate::hir::DeclarationId::new("app.main"));
    let other = crate::hir::ExpressionId::new(&main, "body.s1.value");
    assert_eq!(
        funding::reserve_rows(&layout, &mut held, &other).err(),
        Some(SelectedRefusal::Destination)
    );
    assert_eq!(held.ledger.debited(), 0);
    // Exact total: rows at the destination, then metadata on the same ledger.
    let receipt = funding::reserve_rows(&layout, &mut held, layout.destination()).unwrap();
    assert_eq!(receipt.bytes(), layout.row_storage());
    assert_eq!(held.ledger.debited(), layout.row_storage());
    assert_eq!((held.ordinary.events(), held.ordinary.cumulative()), (0, 0));
    let activation = funding::activate(&layout, &mut held, &receipt).unwrap();
    assert!(activation.invocation > receipt.nonce());
    assert_eq!(activation.row_storage, layout.row_storage());
    assert_eq!(activation.metadata, layout.metadata());
    assert_eq!(activation.first_bytes, [plan[1], plan[2], plan[3]]);
    assert_eq!(held.ledger.debited(), total);
    assert_eq!(
        (held.ordinary.events(), held.ordinary.cumulative()),
        (1, 12_288)
    );
    assert_eq!(funding::finish(&mut held), Some(activation.invocation));
    // Retiring the receipt refunds nothing.
    assert_eq!(
        funding::settle_row_receipt(receipt),
        SettledRows {
            bytes: layout.row_storage()
        }
    );
    assert_eq!(held.ledger.debited(), total);
    // One short at the first Bytes operand: rows stay debited, metadata
    // refuses, nothing is claimed and a retry stays refused.
    let mut issuer = HeldIssuer::new();
    let mut ledger = Ledger::new(total - 1);
    let mut ordinary = Ordinary::default();
    let mut held = HeldInvocation {
        issuer: &mut issuer,
        ledger: &mut ledger,
        ordinary: &mut ordinary,
    };
    let receipt = funding::reserve_rows(&layout, &mut held, layout.destination()).unwrap();
    for _ in 0..2 {
        assert_eq!(
            funding::activate(&layout, &mut held, &receipt).err(),
            Some(SelectedRefusal::MetadataUnfunded)
        );
        assert!(held.issuer.active().is_none());
        assert_eq!(
            (held.ledger.debited(), held.ledger.failed()),
            (layout.row_storage(), true)
        );
    }
    assert_eq!(
        funding::settle_row_receipt(receipt).bytes,
        layout.row_storage()
    );
    assert_eq!(held.ledger.debited(), layout.row_storage());
}

#[test]
fn second_invocation_debits_the_same_ledger_without_reset() {
    let layout = layout(4096);
    let total = layout.row_storage() + layout.metadata();
    let mut issuer = HeldIssuer::new();
    let mut ledger = Ledger::new(total * 2);
    let mut ordinary = Ordinary::default();
    let mut held = HeldInvocation {
        issuer: &mut issuer,
        ledger: &mut ledger,
        ordinary: &mut ordinary,
    };
    let first_rows = funding::reserve_rows(&layout, &mut held, layout.destination()).unwrap();
    let first = funding::activate(&layout, &mut held, &first_rows).unwrap();
    // Overlap on the held issuer refuses without a debit.
    assert_eq!(
        funding::activate(&layout, &mut held, &first_rows).err(),
        Some(SelectedRefusal::IssuerBusy)
    );
    assert_eq!(held.ledger.debited(), total);
    assert_eq!(funding::finish(&mut held), Some(first.invocation));
    // A finished invocation cannot reuse its receipt to skip a second row
    // reservation, even while the original receipt is still held.
    assert_eq!(
        funding::activate(&layout, &mut held, &first_rows).err(),
        Some(SelectedRefusal::RowReceipt)
    );
    assert_eq!(held.ledger.debited(), total);
    assert_eq!(held.issuer.active(), None);
    funding::settle_row_receipt(first_rows);
    let second_rows = funding::reserve_rows(&layout, &mut held, layout.destination()).unwrap();
    assert!(second_rows.nonce() > first.invocation);
    assert_eq!(held.ledger.debited(), total + layout.row_storage());
    let second = funding::activate(&layout, &mut held, &second_rows).unwrap();
    assert!(second.invocation > second_rows.nonce());
    assert_eq!(held.ledger.debited(), total * 2);
    assert_eq!(funding::finish(&mut held), Some(second.invocation));
    funding::settle_row_receipt(second_rows);
    assert_eq!(held.ledger.debited(), total * 2);
    // A third invocation is refused at its destination; nothing resets.
    assert_eq!(
        funding::reserve_rows(&layout, &mut held, layout.destination()).err(),
        Some(SelectedRefusal::RowsUnfunded)
    );
    assert_eq!(
        (held.ledger.debited(), held.ledger.failed()),
        (total * 2, true)
    );
}

#[test]
fn receipts_bind_account_issuer_destination_and_geometry() {
    let large = layout(4096);
    let small = layout(64);
    let mut issuer = HeldIssuer::new();
    let mut ledger = Ledger::new(usize::MAX);
    let mut ordinary = Ordinary::default();
    let mut held = HeldInvocation {
        issuer: &mut issuer,
        ledger: &mut ledger,
        ordinary: &mut ordinary,
    };
    let receipt = funding::reserve_rows(&large, &mut held, large.destination()).unwrap();
    // Another geometry, ledger or issuer cannot accept it.
    assert_eq!(
        funding::activate(&small, &mut held, &receipt).err(),
        Some(SelectedRefusal::RowReceipt)
    );
    let mut other_ledger = Ledger::new(usize::MAX);
    let mut other_ordinary = Ordinary::default();
    let mut foreign = HeldInvocation {
        issuer: &mut *held.issuer,
        ledger: &mut other_ledger,
        ordinary: &mut other_ordinary,
    };
    assert_eq!(
        funding::activate(&large, &mut foreign, &receipt).err(),
        Some(SelectedRefusal::RowReceipt)
    );
    let mut other_issuer = HeldIssuer::new();
    let mut foreign = HeldInvocation {
        issuer: &mut other_issuer,
        ledger: &mut *held.ledger,
        ordinary: &mut *held.ordinary,
    };
    assert_eq!(
        funding::activate(&large, &mut foreign, &receipt).err(),
        Some(SelectedRefusal::RowReceipt)
    );
    assert_eq!(held.ledger.debited(), large.row_storage());
    // Exhausted ordinary budgets neither block nor fund the row reservation.
    let mut issuer = HeldIssuer::new();
    let mut ledger = Ledger::new(large.row_storage() + large.metadata());
    let mut ordinary = Ordinary::spent_for_test(32, 2_097_152);
    let mut held = HeldInvocation {
        issuer: &mut issuer,
        ledger: &mut ledger,
        ordinary: &mut ordinary,
    };
    let receipt = funding::reserve_rows(&large, &mut held, large.destination()).unwrap();
    assert_eq!(held.ordinary.events(), 32);
    assert_eq!(
        funding::activate(&large, &mut held, &receipt).err(),
        Some(SelectedRefusal::OrdinaryUnfunded)
    );
    assert!(funding::finish(&mut held).is_some());
}

#[test]
fn ordinary_counters_and_nonce_headroom_refuse_without_wrapping() {
    funding::identity_exhaustion_refuses_without_wrap();
    let layout = layout(4096);
    let total = layout.row_storage() + layout.metadata();
    let mut ledger = Ledger::new(total);
    let mut ordinary = Ordinary::spent_for_test(0, 2_097_152 - 12_287);
    let mut issuer = HeldIssuer::new();
    let mut held = HeldInvocation {
        issuer: &mut issuer,
        ledger: &mut ledger,
        ordinary: &mut ordinary,
    };
    let receipt = funding::reserve_rows(&layout, &mut held, layout.destination()).unwrap();
    assert_eq!(
        funding::activate(&layout, &mut held, &receipt).err(),
        Some(SelectedRefusal::OrdinaryUnfunded)
    );
    assert_eq!(held.ledger.debited(), total);
    for next in [0, u64::MAX] {
        let mut ledger = Ledger::new(total);
        let mut ordinary = Ordinary::default();
        let mut issuer = HeldIssuer::exhausted_for_test(next);
        let mut held = HeldInvocation {
            issuer: &mut issuer,
            ledger: &mut ledger,
            ordinary: &mut ordinary,
        };
        assert_eq!(
            funding::reserve_rows(&layout, &mut held, layout.destination()).err(),
            Some(SelectedRefusal::NonceHeadroom)
        );
        assert_eq!(held.ledger.debited(), 0);
    }
    // Rows reserve, but activation lacks headroom for every issue nonce.
    let mut ledger = Ledger::new(total);
    let mut ordinary = Ordinary::default();
    let mut issuer = HeldIssuer::exhausted_for_test(u64::MAX - 4096 - 2);
    let mut held = HeldInvocation {
        issuer: &mut issuer,
        ledger: &mut ledger,
        ordinary: &mut ordinary,
    };
    let receipt = funding::reserve_rows(&layout, &mut held, layout.destination()).unwrap();
    assert_eq!(
        funding::activate(&layout, &mut held, &receipt).err(),
        Some(SelectedRefusal::NonceHeadroom)
    );
    assert_eq!(held.ledger.debited(), layout.row_storage());
}

#[test]
fn owner_context_retention_refuses_unprovable_profiles() {
    let pop_store = [
        Op::Push,
        Op::RowsPop,
        Op::RowsStore,
        Op::PassiveVecSettlement,
    ];
    let layout = derive(&request(64, 3, FIXTURE, &pop_store)).unwrap();
    assert_eq!(layout.max_slot_generation(), 4);
    let settle = Op::PassiveVecSettlement;
    for (operations, refusal) in [
        (
            &[Op::Push, Op::PublicCrossing, settle][..],
            SelectedRefusal::Carrier("private carrier crossing or stored escape"),
        ),
        (
            &[Op::Push, Op::StoredEscape, settle][..],
            SelectedRefusal::Carrier("private carrier crossing or stored escape"),
        ),
        (
            &[Op::Push, Op::RowsStore, settle][..],
            SelectedRefusal::Carrier("operation does not receive its held carrier"),
        ),
        (
            &[Op::Push, Op::ChargedClone, settle][..],
            SelectedRefusal::Carrier("operation does not receive its held carrier"),
        ),
        (
            &[Op::VecMove, settle][..],
            SelectedRefusal::Profile("profile does not begin at the push commit"),
        ),
        (
            &[Op::Push, Op::VecMove][..],
            SelectedRefusal::Profile("profile never settles the row Vec"),
        ),
        (
            &[Op::Push, settle, Op::RowsLen][..],
            SelectedRefusal::Profile("operation after Vec settlement"),
        ),
    ] {
        assert_eq!(derive(&request(64, 3, FIXTURE, operations)), Err(refusal));
    }
    assert!(Request::for_test(64, 3, FIXTURE, &[Op::RowsLen; MAX_OPERATIONS + 1]).is_none());
}

#[test]
fn receiver_reads_the_validated_owner_never_the_legacy_descriptor() {
    let source = crate::parse(ORIGINAL, "selected-layout.spx").unwrap();
    // The private factory resolves and validates the original 4096-row source.
    let owner = crate::hir::selected_fused_candidate::validate_selected(&source)
        .unwrap_or_else(|refusal| panic!("validated owner refused: {refusal:?}"));
    let request = Request::from_validated(&owner, FIXTURE).unwrap();
    assert_eq!(request.operations().collect::<Vec<_>>(), CLOSED);
    let layout = derive(&request).unwrap();
    assert_eq!((layout.rows(), layout.width()), (4096, 3));
    assert_eq!(layout.metadata(), 240 + 4096 * 24 + 64 * 24 + 256);
    assert_eq!(layout.row_storage(), 131_072);
    owner
        .with_joined(|joined| {
            assert_eq!(layout.destination(), &joined.selection().allocation().id);
            // The proof-only passive receiver agrees but is never authority.
            let passive = Request::passive_from_joined(joined, FIXTURE).unwrap();
            assert_eq!(derive(&passive).unwrap(), layout);
        })
        .unwrap();
    // The legacy precursor contract is separate and unchanged.
    let legacy = super::super::for_count(4096, 64).unwrap();
    assert_eq!((legacy.row, legacy.context), (40, 165_504));
    assert_ne!(legacy.row, SelectedNativeLayout::row_stride());
}
