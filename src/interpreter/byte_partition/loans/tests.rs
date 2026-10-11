use super::{LoanCapacity, Loans};
use crate::construction_fusion::domain::{owner_core::Core, Issuer, ModuleIdentity};
use crate::interpreter::byte_partition::{tests::account, InvocationRuntime};

fn fixture(count: usize) -> crate::hir::ResolvedProgram {
    let mut source = String::from("module partition.loans; @id(\"loan.main\") fn main()->i64 { let owned=bytes_zeroed(1usize);\n");
    for index in 0..count {
        source.push_str(&format!("let view{index}=bytes_as_slice(owned);\n"));
    }
    source.push_str("0 }");
    let checked = crate::check(&source, "partition-loans.spx").unwrap();
    crate::hir::resolve(&checked).unwrap()
}
#[test]
fn ordinary_capacity_keeps_256_and_private_capacity_stays_64() {
    let program = fixture(256);
    let function = &program.functions[0];
    assert_eq!(function.loan_plan.loans.len(), 256);
    let authority = LoanCapacity::checked(&program, function).unwrap();
    let probe = account(usize::MAX);
    let (mut core, mut children) = Core::partition_owned_ordinary(
        vec![0].into_boxed_slice(),
        1,
        1,
        Issuer::new(),
        ModuleIdentity::new(),
        probe.held_account(),
        &authority,
    )
    .unwrap();
    let mut views = Vec::new();
    for _ in 0..256 {
        views.push(core.borrow(&children[0], 0..1).unwrap());
    }
    for view in &views {
        assert_eq!(core.read(view, |bytes| bytes[0]).unwrap(), 0);
    }
    for view in &mut views {
        core.end_borrow(view).unwrap();
    }
    core.drop_owner(&mut children[0]).unwrap();
    let (mut private, mut children) = Core::partition_owned(
        vec![0].into_boxed_slice(),
        1,
        1,
        Issuer::new(),
        ModuleIdentity::new(),
        probe.held_account(),
    )
    .unwrap();
    let mut views = Vec::new();
    for _ in 0..64 {
        views.push(private.borrow(&children[0], 0..1).unwrap());
    }
    assert!(private.borrow(&children[0], 0..1).is_err());
    for view in &mut views {
        private.end_borrow(view).unwrap();
    }
    private.drop_owner(&mut children[0]).unwrap();
    let detached = function.clone();
    assert!(LoanCapacity::checked(&program, &detached).is_err());
}
#[test]
fn exact_end_edge_retires_one_loan_and_copied_descriptor_stays_stale() {
    let program = fixture(1);
    let function = &program.functions[0];
    let authority = LoanCapacity::checked(&program, function).unwrap();
    let probe = account(usize::MAX);
    let mut receiver = Loans::new(authority, probe.clone()).unwrap();
    let mut owners = receiver
        .partition(vec![7].into_boxed_slice(), 1, 1)
        .unwrap();
    let loan = &function.loan_plan.loans[0];
    // Follow the actual straight-line CFG; never use node-union kills.
    while function.loan_plan.endpoints[receiver.point as usize].point != loan.start {
        let edge = function
            .loan_plan
            .edges
            .iter()
            .find(|edge| edge.from == receiver.point)
            .unwrap();
        receiver.advance(edge.to).unwrap();
    }
    let view = receiver.borrow(&owners[0], loan.id).unwrap();
    let alias = view;
    assert_eq!(receiver.read(view, |bytes| bytes[0]).unwrap(), 7);
    assert_eq!(
        receiver.read_range(alias, 0, 1, |bytes| bytes[0]).unwrap(),
        7
    );
    loop {
        let (index, edge) = function
            .loan_plan
            .edges
            .iter()
            .enumerate()
            .find(|(_, edge)| edge.from == receiver.point)
            .unwrap();
        receiver.advance(edge.to).unwrap();
        if loan.end_edges.contains(&(index as u16)) {
            break;
        }
    }
    receiver.store(&mut owners[0], 0, &[9]).unwrap();
    assert!(receiver.read(alias, |_| ()).is_err());
    assert!(receiver.store(&mut owners[0], 0, &[10]).is_err());
    receiver.abandon_views().unwrap();
    receiver.settle(&mut owners[0]).unwrap();
}

#[test]
fn loan_table_and_core_backing_are_prepaid_through_drop_and_one_short() {
    let program = fixture(1);
    let function = &program.functions[0];
    let probe = account(usize::MAX);
    let owner = vec![7].into_boxed_slice();
    let authority = LoanCapacity::checked(&program, function).unwrap();
    let ((), observed) = crate::test_allocation_observer::measure(|| {
        let mut receiver = Loans::new(authority, probe.clone()).unwrap();
        let mut owners = receiver.partition(owner, 1, 1).unwrap();
        receiver.settle(&mut owners[0]).unwrap();
    });
    let charged = probe.charged();
    assert!(!observed.invalid);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(observed.allocations, 9);
    assert!(observed.allocated <= charged);
    assert!(observed.highwater <= charged);
    assert_eq!(observed.live, 0);
    for limit in [charged, charged - 1] {
        let probe = account(limit);
        let authority = LoanCapacity::checked(&program, function).unwrap();
        let owner = vec![7].into_boxed_slice();
        let pointer = owner.as_ptr();
        let mut receiver = Loans::new(authority, probe.clone()).unwrap();
        match receiver.partition(owner, 1, 1) {
            Ok(mut owners) => {
                assert_eq!(limit, charged);
                receiver.settle(&mut owners[0]).unwrap();
            }
            Err((error, owner)) => {
                assert_eq!(limit, charged - 1);
                assert!(matches!(
                    error,
                    crate::construction_fusion::partition::Preparation::Authority(_)
                ));
                assert_eq!(owner.as_ptr(), pointer);
                let spent = probe.charged();
                assert!(receiver.partition(owner, 1, 1).is_err());
                assert_eq!(probe.charged(), spent);
            }
        }
    }
}
