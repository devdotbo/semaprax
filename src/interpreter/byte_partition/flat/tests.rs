use super::{PartitionIterator, PartitionVec};
use crate::interpreter::byte_partition::{authority::LoanCapacity, loans::Loans, tests::account};
use std::alloc::Layout;

fn fixture() -> crate::hir::ResolvedProgram {
    let checked = crate::check(
        "module partition.flat; @id(\"flat.main\") fn main()->i64 {let owned=bytes_zeroed(1usize);let view=bytes_as_slice(owned);0}",
        "partition-flat.spx",
    ).unwrap();
    crate::hir::resolve(&checked).unwrap()
}
#[test]
fn paid_array_moves_into_iterator_and_returned_child_survives_container_cleanup() {
    let program = fixture();
    let authority = LoanCapacity::checked(&program, &program.functions[0]).unwrap();
    let probe = account(usize::MAX);
    let mut receiver = Loans::new(authority, probe.clone()).unwrap();
    let mut vector = PartitionVec::partition(&mut receiver, vec![1, 2, 3].into_boxed_slice(), 3, 1)
        .ok()
        .unwrap();
    assert_eq!((vector.len(), vector.capacity()), (3, 3));
    let mut returned = vector.take_last(&mut receiver).unwrap().unwrap();
    let before = probe.charged();
    let mut iterator = vector.into_iter(&mut receiver).ok().unwrap();
    assert_eq!(
        probe.charged() - before,
        Layout::new::<PartitionIterator>().size()
    );
    assert_eq!(iterator.remaining(), 2);
    let mut first = iterator.next(&mut receiver).unwrap().unwrap();
    iterator.settle(&mut receiver).unwrap();
    assert_eq!(iterator.remaining(), 0);
    assert!(iterator.next(&mut receiver).unwrap().is_none());
    receiver.store(&mut returned, 0, &[9]).unwrap();
    receiver.store(&mut first, 0, &[7]).unwrap();
    assert_eq!(&*receiver.copy_out_and_settle(&mut returned).unwrap(), &[9]);
    assert_eq!(&*receiver.copy_out_and_settle(&mut first).unwrap(), &[7]);
}
#[test]
fn live_child_loan_refuses_clear_before_any_leaf_and_cleanup_remains_available() {
    let program = fixture();
    let function = &program.functions[0];
    let authority = LoanCapacity::checked(&program, function).unwrap();
    let mut receiver = Loans::new(authority, account(usize::MAX)).unwrap();
    let mut vector = PartitionVec::partition(&mut receiver, vec![1, 2, 3].into_boxed_slice(), 3, 1)
        .ok()
        .unwrap();
    let loan = &function.loan_plan.loans[0];
    while function.loan_plan.endpoints[receiver.endpoint() as usize].point != loan.start {
        let edge = function
            .loan_plan
            .edges
            .iter()
            .find(|edge| edge.from == receiver.endpoint())
            .unwrap();
        receiver.advance(edge.to).unwrap();
    }
    let view = receiver.borrow(vector.owner(1).unwrap(), loan.id).unwrap();
    assert_eq!(receiver.read(view, |bytes| bytes[0]).unwrap(), 2);
    assert!(vector.clear(&mut receiver).is_err());
    assert_eq!(vector.len(), 3);
    assert!(receiver.extent(vector.owner(0).unwrap()).is_err());
    receiver.abandon_views().unwrap();
    vector.settle(&mut receiver).unwrap();
    assert_eq!(vector.len(), 0);
}
