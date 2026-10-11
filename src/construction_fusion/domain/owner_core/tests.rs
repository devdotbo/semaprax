use super::{Child, Core, Runtime};
use crate::construction_fusion::domain::{Issuer, ModuleIdentity, Refusal};
use std::{alloc::Layout, cell::RefCell, rc::Rc};
#[derive(Default)]
struct Account {
    remaining: usize,
    consumed: usize,
    failed: bool,
    events: u32,
    payload: usize,
    allocations: usize,
    releases: usize,
}
#[derive(Clone)]
struct Probe(Rc<RefCell<Account>>);
impl Runtime for Probe {
    type Owner = Box<[u8]>;
    fn reserve_metadata(&mut self, layout: Layout) -> Result<(), Refusal> {
        let mut a = self.0.borrow_mut();
        if a.failed || layout.size() > a.remaining {
            a.failed = true;
            return Err(Refusal::Metadata);
        }
        a.remaining -= layout.size();
        a.consumed += layout.size();
        Ok(())
    }
    fn charge_clone(&mut self, extent: usize) -> Result<(), Refusal> {
        let mut a = self.0.borrow_mut();
        if a.failed
            || a.events >= crate::byte_data_capacity::MAX_BYTES_COPY_SITES
            || a.payload.checked_add(extent).is_none_or(|n| {
                n > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES as usize
            })
        {
            a.failed = true;
            return Err(Refusal::Metadata);
        }
        a.events += 1;
        a.payload += extent;
        Ok(())
    }
    fn allocate(&mut self, extent: usize) -> Self::Owner {
        self.0.borrow_mut().allocations += 1;
        vec![0; extent].into_boxed_slice()
    }
    fn bytes(owner: &Self::Owner) -> &[u8] {
        owner
    }
    fn bytes_mut(owner: &mut Self::Owner) -> &mut [u8] {
        owner
    }
    fn release(&mut self, owner: Self::Owner) {
        drop(owner);
        self.0.borrow_mut().releases += 1;
    }
}
fn required(count: usize) -> usize {
    Core::<Probe>::metadata_layouts(count)
        .unwrap()
        .iter()
        .map(Layout::size)
        .sum()
}
fn probe(count: usize) -> Probe {
    Probe(Rc::new(RefCell::new(Account {
        remaining: required(count),
        ..Account::default()
    })))
}
fn duplicate<'a>(c: &Child<'a>) -> Child<'a> {
    Child {
        held: std::marker::PhantomData,
        issuer: c.issuer,
        module: c.module,
        domain: c.domain,
        slot: c.slot,
        nonce: c.nonce,
        generation: c.generation,
    }
}
// Creator function/parent-container scope is deliberately shorter than the
// invocation registry holding Core. Returning a child never destroys that Core.
fn leave_creator<'a>(core: &mut Core<'a, Probe>, mut parent: Vec<Child<'a>>) -> Child<'a> {
    let returned = parent.pop().unwrap();
    for child in &mut parent {
        core.drop_owner(child).unwrap();
    }
    returned
}
#[test]
fn preparation_preserves_original_and_precharges_every_actual_table_and_header() {
    let module = ModuleIdentity::new();
    let mut issuer = Issuer::new();
    let p = probe(3);
    p.0.borrow_mut().remaining -= 1;
    let owner: Box<[u8]> = vec![1, 2, 3, 4, 5, 6].into_boxed_slice();
    let address = owner.as_ptr();
    let (error, original) = match Core::partition(owner, 3, 2, &mut issuer, &module, p.clone()) {
        Err(e) => e,
        Ok(_) => panic!("one-short preparation published children"),
    };
    assert_eq!(error, Refusal::Metadata);
    assert_eq!(original.as_ptr(), address);
    assert_eq!(&*original, &[1, 2, 3, 4, 5, 6]);
    assert!(p.0.borrow().failed);
    assert_eq!(p.0.borrow().allocations, 0);
    assert_eq!(p.0.borrow().releases, 0);
    let p = probe(3);
    let (core, children) = Core::partition(original, 3, 2, &mut issuer, &module, p.clone())
        .ok()
        .unwrap();
    assert_eq!(core.slots.len(), 3);
    assert_eq!(children.len(), children.capacity());
    assert_eq!(p.0.borrow().consumed, required(3));
    assert_eq!(p.0.borrow().remaining, 0);
    assert_eq!(p.0.borrow().allocations, 0); // partition never copies the payload
    drop(core);
    assert_eq!(p.0.borrow().releases, 1);
}
#[test]
fn popped_child_store_clone_and_last_release_preserve_disjoint_backing() {
    let module = ModuleIdentity::new();
    let mut issuer = Issuer::new();
    let p = probe(3);
    let (mut core, children) = Core::partition(
        vec![1, 2, 3, 4, 5, 6].into_boxed_slice(),
        3,
        2,
        &mut issuer,
        &module,
        p.clone(),
    )
    .ok()
    .unwrap();
    let mut popped = leave_creator(&mut core, children);
    assert_eq!(p.0.borrow().releases, 0);
    let cloned = core.deep_clone(&popped).unwrap();
    assert_eq!(&*cloned, &[5, 6]);
    assert_eq!(p.0.borrow().events, 1);
    assert_eq!(p.0.borrow().payload, 2);
    let stale = duplicate(&popped);
    core.store(&mut popped, 0, 99).unwrap();
    let mut view = core.borrow(&popped, 0..2).unwrap();
    assert_eq!(core.read(&view, |b| b.to_vec()).unwrap(), [99, 6]);
    assert_eq!(&*cloned, &[5, 6]); // actual independent ordinary allocation
    core.end_borrow(&mut view).unwrap();
    assert!(matches!(core.borrow(&stale, 0..1), Err(Refusal::Identity)));
    assert_eq!(core.failure(), Some(Refusal::Identity));
    core.drop_owner(&mut popped).unwrap();
    assert_eq!(p.0.borrow().releases, 1);
    core.runtime.release(cloned);
    assert_eq!(p.0.borrow().releases, 2);
    drop(core);
    assert_eq!(p.0.borrow().releases, 2);
}
#[test]
fn borrowed_store_refuses_before_mutation_and_cleanup_preserves_first_failure() {
    let module = ModuleIdentity::new();
    let mut issuer = Issuer::new();
    let p = probe(2);
    let (mut core, mut children) = Core::partition(
        vec![1, 2, 3, 4].into_boxed_slice(),
        2,
        2,
        &mut issuer,
        &module,
        p.clone(),
    )
    .ok()
    .unwrap();
    let mut loan = core.borrow(&children[0], 0..2).unwrap();
    let mut right = core.borrow(&children[1], 0..2).unwrap();
    assert_eq!(
        core.read_pair(&loan, &right, <[u8]>::cmp).unwrap(),
        std::cmp::Ordering::Less
    );
    core.end_borrow(&mut right).unwrap();
    assert_eq!(core.store(&mut children[0], 0, 9), Err(Refusal::Borrowed));
    right.module ^= 1;
    let called = std::cell::Cell::new(false);
    assert_eq!(
        core.read_pair(&loan, &right, |_, _| called.set(true)),
        Err(Refusal::Borrowed)
    );
    assert!(!called.get()); // existing left failure precedes hostile right
    assert_eq!(Probe::bytes(core.backing.as_ref().unwrap()), &[1, 2, 3, 4]);
    assert_eq!(core.move_owner(&mut children[1]), Err(Refusal::Borrowed));
    core.end_borrow(&mut loan).unwrap();
    for c in &mut children {
        core.drop_owner(c).unwrap();
    }
    assert_eq!(core.failure(), Some(Refusal::Borrowed));
    assert_eq!(p.0.borrow().releases, 1);
}
#[test]
fn foreign_stale_overlapping_and_exhausted_receipts_never_resolve_backing() {
    {
        let module = ModuleIdentity::new();
        let mut issuer = Issuer::new();
        let p = probe(2);
        let (mut core, mut children) = Core::partition(
            vec![1, 2, 3, 4].into_boxed_slice(),
            2,
            2,
            &mut issuer,
            &module,
            p.clone(),
        )
        .ok()
        .unwrap();
        let mut left = core.borrow(&children[0], 0..2).unwrap();
        let mut right = core.borrow(&children[1], 0..2).unwrap();
        let module = right.module;
        right.module ^= 1;
        let called = std::cell::Cell::new(false);
        assert_eq!(
            core.read_pair(&left, &right, |_, _| called.set(true)),
            Err(Refusal::Identity)
        );
        assert!(!called.get());
        assert_eq!(core.failure(), Some(Refusal::Identity));
        right.module = module;
        core.end_borrow(&mut right).unwrap();
        core.end_borrow(&mut left).unwrap();
        for child in &mut children {
            core.drop_owner(child).unwrap();
        }
        assert_eq!(p.0.borrow().releases, 1);
    }

    for mutation in 0..6 {
        let module = ModuleIdentity::new();
        let mut issuer = Issuer::new();
        let p = probe(2);
        let (mut core, mut children) = Core::partition(
            vec![1, 2, 3, 4].into_boxed_slice(),
            2,
            2,
            &mut issuer,
            &module,
            p.clone(),
        )
        .ok()
        .unwrap();
        let mut forged = duplicate(&children[1]);
        match mutation {
            0 => forged.module ^= 1,
            1 => forged.domain += 1,
            2 => forged.slot = usize::MAX,
            3 => core.slots[1].offset = 0,
            4 => {
                core.issuer.get_mut().next = u64::MAX;
            }
            _ => {
                core.slots[1].generation = u64::MAX;
                forged.generation = u64::MAX;
            }
        }
        if mutation == 5 {
            assert_eq!(core.move_owner(&mut forged), Err(Refusal::Exhausted));
        } else {
            assert!(core.borrow(&forged, 0..2).is_err(), "mutation {mutation}");
        }
        assert!(core.failure().is_some());
        assert_eq!(p.0.borrow().allocations, 0);
        // Corrupt range-table control is abandoned, not repaired downstream.
        if mutation != 3 && mutation != 5 {
            for c in &mut children {
                core.drop_owner(c).unwrap();
            }
        }
        drop(core);
        assert_eq!(p.0.borrow().releases, 1);
    }
}
#[test]
fn clone_limits_remain_ordinary_and_partial_abandonment_releases_once() {
    for payload_short in [false, true] {
        let module = ModuleIdentity::new();
        let mut issuer = Issuer::new();
        let p = probe(2);
        {
            let mut a = p.0.borrow_mut();
            a.events = 31;
            a.payload = crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES as usize - 2;
            if payload_short {
                a.payload += 1;
            }
        }
        let (mut core, mut children) = Core::partition(
            vec![1, 2, 3, 4].into_boxed_slice(),
            2,
            2,
            &mut issuer,
            &module,
            p.clone(),
        )
        .ok()
        .unwrap();
        if !payload_short {
            let copy = core.deep_clone(&children[0]).unwrap();
            assert_eq!(p.0.borrow().events, 32);
            assert_eq!(p.0.borrow().payload, 2_097_152);
            core.runtime.release(copy);
        }
        assert!(core.deep_clone(&children[1]).is_err());
        let first = core.failure();
        core.drop_owner(&mut children[0]).unwrap();
        assert_eq!(core.failure(), first);
        drop(core); // remaining child/partial publication cannot leak backing
        assert_eq!(p.0.borrow().releases, if payload_short { 1 } else { 2 });
        assert_eq!(p.0.borrow().allocations, if payload_short { 0 } else { 1 });
    }
}
#[test]
fn zero_and_invalid_shapes_preserve_transaction_and_issuer_no_wrap() {
    let module = ModuleIdentity::new();
    for (count, width, extent, error) in [
        (1, 0, 0, Refusal::Range),
        (usize::MAX, 2, 0, Refusal::Range),
        (2, 2, 3, Refusal::Range),
        (8193, 1, 8193, Refusal::Metadata),
    ] {
        let mut issuer = Issuer::new();
        let p = probe(0);
        let owner = vec![8; extent].into_boxed_slice();
        let address = owner.as_ptr();
        let (found, original) =
            match Core::partition(owner, count, width, &mut issuer, &module, p.clone()) {
                Err(e) => e,
                Ok(_) => panic!("invalid shape published"),
            };
        assert_eq!(found, error);
        assert_eq!(address, original.as_ptr());
        assert_eq!(p.0.borrow().consumed, 0);
        assert_eq!(issuer.next, 1);
    }
    let mut issuer = Issuer::new();
    let p = probe(0);
    let (core, children) = Core::partition(
        Vec::new().into_boxed_slice(),
        0,
        1,
        &mut issuer,
        &module,
        p.clone(),
    )
    .ok()
    .unwrap();
    assert!(children.is_empty());
    assert_eq!(p.0.borrow().releases, 1);
    drop(core);
    assert_eq!(p.0.borrow().releases, 1);
}

#[test]
fn actual_requested_backing_is_precharged_without_child_payload_allocations() {
    for count in [0, 1, 4096] {
        let module = ModuleIdentity::new();
        let mut issuer = Issuer::new();
        let p = probe(count);
        let layouts = Core::<Probe>::metadata_layouts(count).unwrap();
        let tables = layouts[1].size() + layouts[2].size() + layouts[3].size();
        let payload = count * 3;
        let ((), observed) = crate::test_allocation_observer::measure(|| {
            // Original ordinary producer: one actual backing, debited to the
            // same account's existing event/payload fields before allocation.
            let mut runtime = p.clone();
            runtime.charge_clone(payload).unwrap();
            let owner = runtime.allocate(payload);
            let (mut core, mut children) =
                Core::partition(owner, count, 3, &mut issuer, &module, runtime)
                    .ok()
                    .unwrap();
            assert_eq!(children.len(), count);
            for child in &mut children {
                core.drop_owner(child).unwrap();
            }
            drop(children);
            drop(core);
        });
        assert!(!observed.invalid);
        assert_eq!(observed.reallocations, 0);
        assert_eq!(observed.allocations, if count == 0 { 1 } else { 4 });
        assert_eq!(observed.allocated, tables + payload);
        assert_eq!(observed.highwater, tables + payload);
        assert_eq!(observed.freed, observed.allocated);
        assert_eq!(observed.live, 0);
        let account = p.0.borrow();
        assert_eq!(account.consumed, required(count));
        assert_eq!(account.remaining, 0);
        assert_eq!(account.events, 1);
        assert_eq!(account.payload, payload);
        assert_eq!(account.allocations, 1);
        assert_eq!(account.releases, 1);
        drop(account);

        // All metadata reservations precede all table allocations. Returning
        // the original owner after a one-short debit allocates no replacement.
        let p = probe(count);
        p.0.borrow_mut().remaining -= 1;
        let original = vec![7u8; payload].into_boxed_slice();
        let address = original.as_ptr();
        let (result, refused) = crate::test_allocation_observer::measure(|| {
            Core::partition(original, count, 3, &mut issuer, &module, p.clone())
        });
        let (error, original) = match result {
            Err(pair) => pair,
            Ok(_) => panic!("one-short actual preparation allocated tables"),
        };
        assert_eq!(error, Refusal::Metadata);
        assert_eq!(original.as_ptr(), address);
        assert_eq!(refused, crate::test_allocation_observer::Metrics::default());
        assert!(p.0.borrow().failed);
        assert_eq!(p.0.borrow().allocations, 0);
        assert_eq!(p.0.borrow().releases, 0);
    }
}

#[test]
fn owned_holders_keep_receipts_current_across_context_moves_and_exact_precharge() {
    let count = 2usize;
    let holder_bytes = Layout::new::<Issuer>().size() + Layout::new::<ModuleIdentity>().size();
    let full = required(count) + holder_bytes;
    let account = Rc::new(RefCell::new(Account {
        remaining: full,
        ..Account::default()
    }));
    let runtime = Probe(account.clone());
    let ((), metrics) = crate::test_allocation_observer::measure(|| {
        let mut runtime = runtime;
        runtime.charge_clone(6).unwrap();
        let owner = runtime.allocate(6);
        let (core, children) = Core::partition_owned(
            owner,
            count,
            3,
            Issuer::new(),
            ModuleIdentity::new(),
            runtime,
        )
        .ok()
        .unwrap();
        // Moving the invocation context does not change either boxed identity.
        let mut registry = [Some(core)];
        let mut core = registry[0].take().unwrap();
        let mut returned = leave_creator(&mut core, children);
        core.store(&mut returned, 1, 9).unwrap();
        let mut loan = core.borrow(&returned, 0..3).unwrap();
        assert_eq!(
            core.read(&loan, |bytes| <[u8; 3]>::try_from(bytes).unwrap())
                .unwrap(),
            [0, 9, 0]
        );
        core.end_borrow(&mut loan).unwrap();
        core.drop_owner(&mut returned).unwrap();
        drop(core);
    });
    let tables: usize = Core::<Probe>::metadata_layouts(count).unwrap()[1..=3]
        .iter()
        .map(Layout::size)
        .sum();
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocations, 6); // original, three tables, two holders
    assert_eq!(metrics.reallocations, 0);
    assert_eq!(metrics.allocated, 6 + tables + holder_bytes);
    assert_eq!(metrics.highwater, metrics.allocated);
    assert_eq!(metrics.freed, metrics.allocated);
    assert_eq!(metrics.live, 0);
    assert_eq!(account.borrow().consumed, full);
    assert_eq!(account.borrow().remaining, 0);
    assert_eq!(account.borrow().releases, 1);
    // Observe preparation alone under one-short backing. Both boxes and all
    // tables remain unallocated, and the exact original owner is returned.
    let account = Rc::new(RefCell::new(Account {
        remaining: full - 1,
        ..Account::default()
    }));
    let runtime = Probe(account.clone());
    let original: Box<[u8]> = vec![0; 6].into_boxed_slice();
    let pointer = original.as_ptr();
    let (returned, metrics) = crate::test_allocation_observer::measure(|| {
        let (error, original) = Core::partition_owned(
            original,
            count,
            3,
            Issuer::new(),
            ModuleIdentity::new(),
            runtime,
        )
        .err()
        .unwrap();
        assert_eq!(error, Refusal::Metadata);
        assert_eq!(original.as_ptr(), pointer);
        original
    });
    assert_eq!(metrics, crate::test_allocation_observer::Metrics::default());
    assert!(account.borrow().failed);
    drop(returned);
}

#[test]
fn paired_reads_validate_both_loans_and_keep_foreign_refusal_sticky() {
    let module = ModuleIdentity::new();
    let mut issuer = Issuer::new();
    let p = probe(2);
    let (mut core, mut children) = Core::partition(
        vec![1, 2, 3, 4].into_boxed_slice(),
        2,
        2,
        &mut issuer,
        &module,
        p.clone(),
    )
    .ok()
    .unwrap();
    let mut left = core.borrow(&children[0], 0..2).unwrap();
    let mut right = core.borrow(&children[1], 0..2).unwrap();
    assert_eq!(
        core.read_pair(&left, &right, |a, b| [a[0], a[1], b[0], b[1]])
            .unwrap(),
        [1, 2, 3, 4]
    );
    assert_eq!(
        core.read_pair(&right, &left, |a, b| a.cmp(b)).unwrap(),
        std::cmp::Ordering::Greater
    );
    let mut foreign_issuer = Issuer::new();
    let (mut foreign, foreign_children) = Core::partition(
        vec![9, 9].into_boxed_slice(),
        1,
        2,
        &mut foreign_issuer,
        &module,
        probe(1),
    )
    .ok()
    .unwrap();
    let foreign_loan = foreign.borrow(&foreign_children[0], 0..2).unwrap();
    let calls = std::cell::Cell::new(0);
    assert_eq!(
        core.read_pair(&left, &foreign_loan, |_, _| calls.set(calls.get() + 1)),
        Err(Refusal::Identity)
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(core.failure(), Some(Refusal::Identity));
    assert_eq!(
        core.read_pair(&left, &right, |_, _| calls.set(calls.get() + 1)),
        Err(Refusal::Identity)
    );
    assert_eq!(calls.get(), 0);
    // Successful cleanup reports its own completion and retains selected failure.
    core.end_borrow(&mut left).unwrap();
    assert_eq!(core.failure(), Some(Refusal::Identity));
    core.end_borrow(&mut right).unwrap();
    assert_eq!(core.failure(), Some(Refusal::Identity));
    for child in &mut children {
        core.drop_owner(child).unwrap();
        assert_eq!(core.failure(), Some(Refusal::Identity));
    }
    assert_eq!(p.0.borrow().releases, 1);
}
