//! Private storage precursor: no Value carrier, evaluator selection or admission.
//! A domain owns one payload; child owners and read views contain no Arc, Box,
//! payload clone or copy-on-write path. Source authentication and complete
//! interpreter/Native/Wasm execution still have to join before activation.
use crate::construction_fusion::domain::Backing;

#[derive(Default)]
struct AllocationState {
    events: u32,
    payload_bytes: u64,
    allocations: usize,
    releases: usize,
}
struct InterpreterBacking<'a> {
    state: &'a mut AllocationState,
}
impl Backing for InterpreterBacking<'_> {
    type Owner = Box<[u8]>;
    fn allocate(&mut self, extent: usize) -> Self::Owner {
        let extent_u64 = u64::try_from(extent).expect("fusion extent invariant");
        assert!(extent_u64 <= crate::byte_ops::MAX_BUFFER_CAPACITY_BYTES);
        let events = self.state.events.checked_add(1).expect("event overflow");
        let payload = self
            .state
            .payload_bytes
            .checked_add(extent_u64)
            .expect("payload overflow");
        assert!(events <= crate::byte_data_capacity::MAX_BYTES_COPY_SITES);
        assert!(payload <= crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES);
        self.state.events = events;
        self.state.payload_bytes = payload;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(extent)
            .expect("physical Bytes allocation invariant");
        assert_eq!(
            bytes.capacity(),
            extent,
            "payload physical inventory invariant"
        );
        bytes.resize(extent, 0u8);
        self.state.allocations += 1;
        // Exact retained capacity makes this a transfer of the existing backing.
        bytes.into_boxed_slice()
    }
    fn release(&mut self, owner: Self::Owner) {
        self.state.releases += 1;
        drop(owner);
    }
}
fn inputs(count: u32) -> (crate::ast::Program, crate::hir::ResolvedProgram) {
    let text = include_str!("../../tests/fixtures/record-traversal-maximum.spx")
        .replace("4096", &count.to_string());
    let source = crate::check(&text, "interpreter-fusion-precursor.spx").unwrap();
    let typed = crate::hir::resolve(&source).unwrap();
    (source, typed)
}
#[test]
fn private_payload_has_one_backing_and_nonowning_exact_child_views() {
    use crate::construction_fusion::{
        derive,
        domain::{Domain, Issuer, ModuleIdentity},
    };
    // Ordinary checked <=32 source; this is not an admitted 4096-row execution.
    let (source, typed) = inputs(8);
    let descriptor = derive(&source, &typed).unwrap();
    let mut issuer = Issuer::new();
    let module = ModuleIdentity::new();
    let mut state = AllocationState::default();
    {
        let mut domain = Domain::new(
            &descriptor,
            &mut issuer,
            &module,
            InterpreterBacking { state: &mut state },
            usize::MAX,
        )
        .unwrap();
        domain.start().unwrap();
        let mut owners = Vec::with_capacity(8);
        for ordinal in 0..8 {
            let owner = domain.issue(ordinal).unwrap();
            domain.publish(&owner).unwrap();
            owners.push(owner);
        }
        domain.complete().unwrap();
        for owner in &owners {
            let mut view = domain.borrow(owner).unwrap();
            domain
                .with_borrowed_range(&view, |backing, range| {
                    assert_eq!(backing.len(), 24);
                    assert_eq!(range.len(), 3);
                    assert_eq!(&backing[range], &[0u8; 3]);
                })
                .unwrap();
            assert!(domain.read_range(&view, 3, 1).is_err());
            domain.end_borrow(&mut view).unwrap();
        }
        for mut owner in owners {
            domain.drop_owner(&mut owner).unwrap();
        }
    }
    assert_eq!(state.allocations, 1);
    assert_eq!(state.releases, 1);
    assert_eq!(state.events, 1);
    assert_eq!(state.payload_bytes, 24);
}
#[test]
fn private_partial_cleanup_releases_one_backing_without_refunding_charges() {
    use crate::construction_fusion::{
        derive,
        domain::{Domain, Issuer, ModuleIdentity},
    };
    let (source, typed) = inputs(8);
    let descriptor = derive(&source, &typed).unwrap();
    for prefix in 0..=8 {
        let mut issuer = Issuer::new();
        let module = ModuleIdentity::new();
        let mut state = AllocationState::default();
        {
            let mut domain = Domain::new(
                &descriptor,
                &mut issuer,
                &module,
                InterpreterBacking { state: &mut state },
                usize::MAX,
            )
            .unwrap();
            domain.start().unwrap();
            let mut owners = Vec::with_capacity(prefix);
            for ordinal in 0..prefix {
                let owner = domain.issue(ordinal).unwrap();
                if ordinal + 1 < prefix {
                    domain.publish(&owner).unwrap();
                }
                owners.push(owner);
            }
            domain.abort().unwrap();
            for mut owner in owners {
                domain.drop_owner(&mut owner).unwrap();
            }
            domain.settle_abort().unwrap();
        }
        let allocations = usize::from(prefix != 0);
        assert_eq!(state.allocations, allocations);
        assert_eq!(state.releases, allocations);
        assert_eq!(state.events, allocations as u32);
        assert_eq!(state.payload_bytes, if prefix == 0 { 0 } else { 24 });
    }
}
