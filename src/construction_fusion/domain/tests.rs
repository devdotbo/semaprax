use super::*;
use std::{cell::Cell, rc::Rc};
#[derive(Clone)]
struct Probe {
    calls: Rc<Cell<usize>>,
    frees: Rc<Cell<usize>>,
}
impl Probe {
    fn new() -> Self {
        Self {
            calls: Rc::new(Cell::new(0)),
            frees: Rc::new(Cell::new(0)),
        }
    }
}
impl Backing for Probe {
    type Owner = usize;
    fn allocate(&mut self, extent: usize) -> usize {
        self.calls.set(self.calls.get() + 1);
        extent
    }
    fn release(&mut self, owner: usize) {
        assert!(owner > 0);
        self.frees.set(self.frees.get() + 1);
    }
}
fn forged<'a>(lease: &Lease<'a>) -> Lease<'a> {
    Lease {
        lifetime: std::marker::PhantomData,
        issuer: lease.issuer,
        domain: lease.domain,
        module: lease.module,
        slot: lease.slot,
        nonce: lease.nonce,
        generation: lease.generation,
        offset: lease.offset,
        len: lease.len,
    }
}
#[test]
fn lazy_backing_exact_metadata_and_last_child_release() {
    let (source, typed) = super::super::tests::inputs(8);
    let descriptor = super::super::derive(&source, &typed).unwrap();
    let required = Domain::<Probe>::metadata_bytes(8).unwrap();
    let probe = Probe::new();
    let module = ModuleIdentity::new();
    let mut issuer = Issuer::new();
    assert!(matches!(
        Domain::new(
            &descriptor,
            &mut issuer,
            &module,
            probe.clone(),
            required - 1
        ),
        Err(Refusal::Metadata)
    ));
    assert_eq!(probe.calls.get(), 0);
    let mut domain =
        Domain::new(&descriptor, &mut issuer, &module, probe.clone(), required).unwrap();
    domain.start().unwrap();
    assert!(domain.slots.is_empty() && domain.views.is_empty());
    assert_eq!(probe.calls.get(), 0);
    assert_eq!(domain.complete(), Err(Refusal::Unpublished));
    assert_eq!(domain.phase(), Phase::Building);
    assert!(domain.construction_pin);
    assert!(domain.slots.is_empty() && domain.views.is_empty());
    assert_eq!(probe.calls.get(), 0);
    assert_eq!(probe.frees.get(), 0);
    let mut leases = Vec::with_capacity(8);
    for row in 0..8 {
        let lease = domain.issue(row).unwrap();
        assert_eq!(lease.offset, row * 3);
        assert_eq!(lease.len, 3);
        domain.publish(&lease).unwrap();
        leases.push(lease);
    }
    assert_eq!(probe.calls.get(), 1);
    domain.complete().unwrap();
    let mut view = domain.borrow(&leases[7]).unwrap();
    assert_eq!(domain.read_range(&view, 0, 3), Ok((21, 3)));
    assert_eq!(
        domain.with_borrowed_range(&view, |owner, range| (*owner, range)),
        Ok((24, 21..24))
    );
    assert_eq!(domain.read_range(&view, 3, 1), Err(Refusal::Range));
    assert_eq!(domain.move_owner(&mut leases[7]), Err(Refusal::Borrowed));
    assert_eq!(domain.drop_owner(&mut leases[7]), Err(Refusal::Borrowed));
    domain.end_borrow(&mut view).unwrap();
    assert_eq!(domain.end_borrow(&mut view), Err(Refusal::Identity));
    assert_eq!(domain.read_range(&view, 0, 1), Err(Refusal::Identity));

    for lease in &mut leases[..7] {
        domain.drop_owner(lease).unwrap();
    }
    assert_eq!(probe.frees.get(), 0);
    domain.move_owner(&mut leases[7]).unwrap();
    domain.drop_owner(&mut leases[7]).unwrap();
    assert_eq!(probe.frees.get(), 1);
    assert_eq!(domain.phase(), Phase::Settled);
}
#[test]
fn abort_pin_unpublished_and_exact_generation_refuse_forged_owners() {
    let (source, typed) = super::super::tests::inputs(8);
    let descriptor = super::super::derive(&source, &typed).unwrap();
    let probe = Probe::new();
    let module = ModuleIdentity::new();
    let mut issuer = Issuer::new();
    let mut domain =
        Domain::new(&descriptor, &mut issuer, &module, probe.clone(), usize::MAX).unwrap();
    domain.start().unwrap();
    let mut lease = domain.issue(0).unwrap();
    assert!(matches!(domain.issue(1), Err(Refusal::Unpublished)));
    assert_eq!(domain.complete(), Err(Refusal::Unpublished));
    let mut range = forged(&lease);
    range.offset += 1;
    assert_eq!(domain.publish(&range), Err(Refusal::Range));
    let mut foreign = forged(&lease);
    foreign.issuer += 1;
    assert_eq!(domain.publish(&foreign), Err(Refusal::Identity));
    domain.abort().unwrap();
    assert_eq!(domain.settle_abort(), Err(Refusal::Phase));
    domain.drop_owner(&mut lease).unwrap();
    assert_eq!(probe.frees.get(), 0);
    assert_eq!(domain.drop_owner(&mut lease), Err(Refusal::Identity));
    domain.settle_abort().unwrap();
    assert_eq!(probe.frees.get(), 1);
}
#[test]
fn maximum_protocol_dimensions_are_reference_only_and_never_admit_original_source() {
    let (source, typed) = super::super::tests::inputs(8);
    let mut descriptor = super::super::derive(&source, &typed).unwrap();
    // Explicit synthetic dimensions exercise the protocol, NOT a checked or
    // source-bound 4096 HIR program. That original admission remains refused.
    descriptor.typed.iterations = 4096;
    descriptor.payload_bytes = 12288;
    descriptor.string_bytes = 12288;
    let module = ModuleIdentity::new();
    let mut issuer = Issuer::new();
    let probe = Probe::new();
    let mut domain = Domain::new(
        &descriptor,
        &mut issuer,
        &module,
        probe.clone(),
        Domain::<Probe>::metadata_bytes(4096).unwrap(),
    )
    .unwrap();
    domain.start().unwrap();
    let mut leases = Vec::with_capacity(4096);
    for row in 0..4096 {
        let lease = domain.issue(row).unwrap();
        domain.publish(&lease).unwrap();
        leases.push(lease);
    }
    domain.complete().unwrap();
    assert_eq!(leases[4095].offset, 12285);
    assert_eq!(probe.calls.get(), 1);
    let mut stale = forged(&leases[0]);
    domain.move_owner(&mut leases[0]).unwrap();
    assert_eq!(domain.drop_owner(&mut stale), Err(Refusal::Identity));
    for lease in &mut leases {
        domain.drop_owner(lease).unwrap();
    }
    assert_eq!(probe.frees.get(), 1);
    assert!(
        crate::check(super::super::tests::ORIGINAL, "fusion-max.spx")
            .unwrap_err()
            .iter()
            .any(|d| d.code == "SPX-T267")
    );
}
#[test]
fn issuer_exhaustion_never_wraps_or_allocates_backing() {
    let (source, typed) = super::super::tests::inputs(1);
    let descriptor = super::super::derive(&source, &typed).unwrap();
    let module = ModuleIdentity::new();
    let mut issuer = Issuer { next: u64::MAX - 1 };
    let probe = Probe::new();
    let mut domain =
        Domain::new(&descriptor, &mut issuer, &module, probe.clone(), usize::MAX).unwrap();
    domain.start().unwrap();
    assert!(matches!(domain.issue(0), Err(Refusal::Exhausted)));
    assert_eq!(probe.calls.get(), 0);
    domain.abort().unwrap();
    domain.settle_abort().unwrap();
    assert_eq!(probe.frees.get(), 0);
}

#[test]
fn one_owner_supports_independently_bounded_concurrent_views() {
    let (source, typed) = super::super::tests::inputs(1);
    let descriptor = super::super::derive(&source, &typed).unwrap();
    let module = ModuleIdentity::new();
    let mut issuer = Issuer::new();
    let required = Domain::<Probe>::metadata_bytes(1).unwrap();
    assert!(matches!(
        Domain::new(
            &descriptor,
            &mut issuer,
            &module,
            Probe::new(),
            required - 1
        ),
        Err(Refusal::Metadata)
    ));
    let mut domain =
        Domain::new(&descriptor, &mut issuer, &module, Probe::new(), required).unwrap();
    domain.start().unwrap();
    let mut owner = domain.issue(0).unwrap();
    domain.publish(&owner).unwrap();
    domain.complete().unwrap();
    let mut views: Vec<_> = (0..MAX_VIEWS)
        .map(|_| domain.borrow(&owner).unwrap())
        .collect();
    assert!(matches!(domain.borrow(&owner), Err(Refusal::Metadata)));
    assert_eq!(domain.drop_owner(&mut owner), Err(Refusal::Borrowed));
    for view in &mut views {
        domain.end_borrow(view).unwrap();
    }
    domain.drop_owner(&mut owner).unwrap();
    assert_eq!(domain.phase(), Phase::Settled);
}
