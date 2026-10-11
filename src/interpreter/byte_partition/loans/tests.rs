use super::{LoanCapacity, Loans};
use crate::construction_fusion::domain::{owner_core::Core, Issuer, ModuleIdentity};
use crate::interpreter::byte_partition::{tests::account, InvocationRuntime};

fn fixture(count: usize) -> crate::hir::ResolvedProgram {
    let mut source = String::from(
        "module partition.loans; @id(\"loan.main\") fn main()->i64 { let owned=bytes_zeroed(1usize);\n",
    );
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
    let source = crate::parse(
        "module partition.seal; @id(\"seal.main\") fn main()->i64 {let original=bytes_zeroed(1usize);let parts=bytes_partition(original,1usize,1usize);if vec_len<Bytes>(parts)==1usize {7}else{0}}",
        "partition-seal.spx",
    ).unwrap();
    let sealed =
        crate::hir::byte_partition::ValidatedPartitionProgram::compile_source(&source, &mut {
            usize::MAX
        })
        .unwrap();
    let current = &sealed.program().functions[0];
    let mut remaining = usize::MAX;
    let capacity = LoanCapacity::from_source(&sealed, current, &mut remaining).unwrap();
    assert!(std::ptr::eq(capacity.program(), sealed.program()));
    let used = usize::MAX - remaining;
    let mut exact = used;
    LoanCapacity::from_source(&sealed, current, &mut exact).unwrap();
    assert_eq!(exact, 0);
    let mut short = used - 1;
    assert!(LoanCapacity::from_source(&sealed, current, &mut short).is_err());
    assert_eq!(short, 0);
    let copied = current.clone();
    let mut foreign_work = usize::MAX;
    assert!(LoanCapacity::from_source(&sealed, &copied, &mut foreign_work).is_err());
    assert_eq!(foreign_work, 0);
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

#[test]
fn finite_adapter_borrows_the_actual_logical_counters_without_reset_or_refund() {
    use crate::interpreter::byte_partition::funding::{upper, Account};
    let program = fixture(1);
    let function = &program.functions[0];
    let authority = LoanCapacity::checked(&program, function).unwrap();
    let required = upper(&authority).unwrap();
    let mut events = 1;
    let mut payload = 1;
    let owner = vec![7].into_boxed_slice();
    {
        // The caller explicitly funds the forecast. `upper` does not allocate
        // or grant a source/root/whole-backing receipt.
        let account = Account::funded(&mut events, &mut payload, required).unwrap();
        let mut receiver = Loans::new(authority, account.handle()).unwrap();
        let mut owners = receiver.partition(owner, 1, 1).unwrap();
        let public = receiver.copy_out_and_settle(&mut owners[0]).unwrap();
        drop(receiver);
        assert_eq!(public.as_ref(), &[7]);
        drop(public);
    }
    assert_eq!(events, 2);
    assert_eq!(payload, 2);
    let before = (events, payload);
    assert!(Account::funded(&mut events, &mut payload, 0).is_err());
    assert_eq!((events, payload), before);
    {
        use crate::construction_fusion::domain::owner_core::Runtime;
        let account = Account::funded(&mut events, &mut payload, required).unwrap();
        let mut first = account.handle();
        let mut second = first.held_account();
        for _ in 2..crate::byte_data_capacity::MAX_BYTES_COPY_SITES {
            first.charge_clone(1).unwrap();
        }
        assert!(second.charge_clone(1).is_err());
        assert!(first
            .reserve_metadata(std::alloc::Layout::new::<u8>())
            .is_err());
    }
    assert_eq!(events, crate::byte_data_capacity::MAX_BYTES_COPY_SITES);
    assert_eq!(payload, u64::from(events));
}

#[test]
fn affine_value_owner_cannot_copy_and_view_alias_never_owns_the_loan() {
    use crate::interpreter::OwnedBytesValue;
    let program = fixture(1);
    let function = &program.functions[0];
    let authority = LoanCapacity::checked(&program, function).unwrap();
    let mut receiver = Loans::new(authority, account(usize::MAX)).unwrap();
    let mut owners = receiver
        .partition(vec![7].into_boxed_slice(), 1, 1)
        .unwrap();
    let mut value = OwnedBytesValue::partition(1, owners.pop().unwrap());
    assert!(value.borrow_alias().is_err());
    let loan = &function.loan_plan.loans[0];
    while function.loan_plan.endpoints[receiver.point as usize].point != loan.start {
        let edge = function
            .loan_plan
            .edges
            .iter()
            .find(|edge| edge.from == receiver.point)
            .unwrap();
        receiver.advance(edge.to).unwrap();
    }
    let borrowed = value
        .borrow_partition_bytes(&mut receiver, loan.id)
        .unwrap();
    let borrowed_alias = borrowed.borrow_alias().unwrap();
    assert_eq!(
        borrowed_alias
            .read_partition_bytes(&mut receiver, |bytes| bytes[0])
            .unwrap(),
        7
    );
    assert!(borrowed_alias
        .borrow_alias()
        .unwrap()
        .move_partition(&mut receiver)
        .is_err());
    let view = borrowed
        .partition_view_as_slice(&mut receiver, loan.origin.root.clone())
        .unwrap();
    let alias = view.clone();
    let range = alias.range(0, 1);
    assert_eq!(
        view.read_partition(&mut receiver, |bytes| bytes[0])
            .unwrap(),
        7
    );
    assert_eq!(
        range
            .read_partition(&mut receiver, |bytes| bytes[0])
            .unwrap(),
        7
    );
    drop(alias); // no Loan clone and no physical end_borrow
    assert_eq!(
        view.read_partition(&mut receiver, |bytes| bytes[0])
            .unwrap(),
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
    value.move_partition(&mut receiver).unwrap();
    value.store_partition(&mut receiver, 0, &[9]).unwrap();
    assert!(view.read_partition(&mut receiver, |_| ()).is_err());
    assert!(borrowed
        .read_partition_bytes(&mut receiver, |_| ())
        .is_err());
    value.settle_partition(&mut receiver).unwrap();
}
