use super::super::{LoanCapacity, Loans, Slot};
use super::Frame;
use crate::hir::{ResolvedFunction, ResolvedProgram};
use crate::interpreter::byte_partition::tests::{account, Probe};
use std::alloc::Layout;

fn fixture() -> ResolvedProgram {
    let source = r#"module partition.frames;
        @id("frames.helper") fn helper(value:own Bytes)->i64 {
            let view=bytes_as_slice(value); 0
        }
        @id("frames.borrowed") fn borrowed(value:borrow Bytes)->i64 {
            let view=bytes_as_slice(value); 0
        }
        @id("frames.scalar") fn scalar()->i64 {0}
        @id("frames.main") fn main()->i64 {
            let owned=bytes_zeroed(1usize); let view=bytes_as_slice(owned); 0
        }"#;
    crate::hir::resolve(&crate::check(source, "partition-frames.spx").unwrap()).unwrap()
}
fn function<'p>(program: &'p ResolvedProgram, name: &str) -> &'p ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.name == name)
        .unwrap()
}
fn start(receiver: &mut Loans<'_, Probe>) {
    let loan = &receiver.capacity.function().loan_plan.loans[0];
    while receiver.capacity.function().loan_plan.endpoints[receiver.point as usize].point
        != loan.start
    {
        let next = receiver
            .capacity
            .function()
            .loan_plan
            .edges
            .iter()
            .find(|edge| edge.from == receiver.point)
            .unwrap()
            .to;
        receiver.advance(next).unwrap();
    }
}
#[test]
fn caller_views_and_returned_children_survive_and_paid_frames_reuse() {
    let program = fixture();
    let main = function(&program, "main");
    let helper = function(&program, "helper");
    let probe = account(usize::MAX);
    let mut receiver = Loans::new(
        LoanCapacity::checked(&program, main).unwrap(),
        probe.clone(),
    )
    .unwrap();
    let mut owners = receiver
        .partition(vec![7].into_boxed_slice(), 1, 1)
        .unwrap();
    start(&mut receiver);
    let caller = receiver
        .borrow(&owners[0], main.loan_plan.loans[0].id)
        .unwrap();
    receiver
        .enter_frame(LoanCapacity::checked(&program, helper).unwrap())
        .unwrap();
    assert_eq!(receiver.read(caller, |bytes| bytes[0]).unwrap(), 7);
    let mut returned = receiver
        .partition(vec![9].into_boxed_slice(), 1, 1)
        .unwrap();
    start(&mut receiver);
    let callee = receiver
        .borrow(&returned[0], helper.loan_plan.loans[0].id)
        .unwrap();
    assert_ne!(caller.frame, callee.frame);
    assert_eq!(receiver.read(callee, |bytes| bytes[0]).unwrap(), 9);
    receiver.leave_frame().unwrap();
    assert_eq!(receiver.read(caller, |bytes| bytes[0]).unwrap(), 7);
    receiver.store(&mut returned[0], 0, &[11]).unwrap();
    let before = probe.charged();
    receiver
        .enter_frame(LoanCapacity::checked(&program, helper).unwrap())
        .unwrap();
    assert_eq!(probe.charged(), before);
    start(&mut receiver);
    let renewed = receiver
        .borrow(&returned[0], helper.loan_plan.loans[0].id)
        .unwrap();
    assert_ne!(callee.frame, renewed.frame);
    assert_eq!(receiver.read(renewed, |bytes| bytes[0]).unwrap(), 11);
    // Old callee descriptors cannot alias a reused slot array.
    assert!(receiver.read(callee, |_| ()).is_err());
    assert!(receiver.read(caller, |_| ()).is_err());
    receiver.leave_frame().unwrap();
    receiver.abandon_views().unwrap();
    receiver.settle(&mut returned[0]).unwrap();
    receiver.settle(&mut owners[0]).unwrap();
}
#[test]
fn entry_has_exact_one_short_shared_funding_and_scalar_frames_have_no_slots() {
    let program = fixture();
    let main = function(&program, "main");
    let helper = function(&program, "helper");
    let scalar = function(&program, "scalar");
    assert!(scalar.loan_plan.loans.is_empty());
    let probe = account(usize::MAX);
    let mut receiver = Loans::new(
        LoanCapacity::checked(&program, main).unwrap(),
        probe.clone(),
    )
    .unwrap();
    receiver
        .enter_frame(LoanCapacity::checked(&program, helper).unwrap())
        .unwrap();
    let exact = probe.charged();
    // Independent complete layout sum, not a second invocation of the
    // implementation's metadata-layout or measured-charge helper.
    let independent = Layout::new::<Loans<'_, Probe>>().size()
        + Layout::new::<Vec<Slot>>().size()
        + Layout::array::<Slot>(main.loan_plan.loans.len())
            .unwrap()
            .size()
        + Layout::new::<Vec<Frame<'_>>>().size()
        + Layout::array::<Frame<'_>>(1).unwrap().size()
        + Layout::new::<Vec<Slot>>().size()
        + Layout::array::<Slot>(helper.loan_plan.loans.len())
            .unwrap()
            .size();
    assert_eq!(exact, independent);
    receiver.leave_frame().unwrap();
    let before = probe.charged();
    for _ in 0..32 {
        receiver
            .enter_frame(LoanCapacity::checked(&program, scalar).unwrap())
            .unwrap();
        assert!(receiver.slots.is_empty());
        receiver.leave_frame().unwrap();
    }
    assert_eq!(probe.charged(), before);
    let paid = account(exact);
    let mut complete =
        Loans::new(LoanCapacity::checked(&program, main).unwrap(), paid.clone()).unwrap();
    complete
        .enter_frame(LoanCapacity::checked(&program, helper).unwrap())
        .unwrap();
    assert_eq!(paid.charged(), exact);
    complete.leave_frame().unwrap();
    let short = account(exact - 1);
    let mut refused = Loans::new(
        LoanCapacity::checked(&program, main).unwrap(),
        short.clone(),
    )
    .unwrap();
    let pointer = refused.slots.as_ptr();
    assert!(refused
        .enter_frame(LoanCapacity::checked(&program, helper).unwrap())
        .is_err());
    assert_eq!(refused.slots.as_ptr(), pointer);
    assert!(refused.frames.is_empty());
    let spent = short.charged();
    assert!(refused
        .enter_frame(LoanCapacity::checked(&program, scalar).unwrap())
        .is_err());
    assert_eq!(short.charged(), spent);
}

#[test]
fn inherited_parameter_view_has_one_caller_finalizer_across_exact_caller_end() {
    let program = fixture();
    let main = function(&program, "main");
    let helper = function(&program, "borrowed");
    assert!(helper.loan_plan.loans.is_empty());
    assert!(helper.loan_plan.endpoints.is_empty());
    let probe = account(usize::MAX);
    let mut receiver = Loans::new(
        LoanCapacity::checked(&program, main).unwrap(),
        probe.clone(),
    )
    .unwrap();
    let mut owners = receiver
        .partition(vec![7].into_boxed_slice(), 1, 1)
        .unwrap();
    start(&mut receiver);
    let loan = &main.loan_plan.loans[0];
    let caller = receiver.borrow(&owners[0], loan.id).unwrap();
    receiver
        .enter_frame(LoanCapacity::checked(&program, helper).unwrap())
        .unwrap();
    assert!(receiver.slots.is_empty());
    let before = probe.charged();
    let local = receiver
        .inherit_parameter(&helper.params[0], caller)
        .unwrap();
    assert_eq!(local, caller);
    assert_eq!(probe.charged(), before);
    assert_eq!(receiver.read(local, |bytes| bytes[0]).unwrap(), 7);
    receiver.leave_frame().unwrap();
    // Returning never ends the caller's physical loan or mints a callee loan.
    assert_eq!(receiver.read(caller, |bytes| bytes[0]).unwrap(), 7);
    loop {
        let (index, edge) = main
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
    assert!(receiver
        .read(local, |_| panic!("stale inherited callback"))
        .is_err());
    assert!(receiver.read(caller, |_| ()).is_err());
    receiver.abandon_views().unwrap();
    receiver.settle(&mut owners[0]).unwrap();
    let mut fresh = Loans::new(
        LoanCapacity::checked(&program, main).unwrap(),
        account(usize::MAX),
    )
    .unwrap();
    let mut owners = fresh.partition(vec![7].into_boxed_slice(), 1, 1).unwrap();
    start(&mut fresh);
    let caller = fresh.borrow(&owners[0], loan.id).unwrap();
    fresh
        .enter_frame(LoanCapacity::checked(&program, helper).unwrap())
        .unwrap();
    let copied = helper.params[0].clone();
    assert!(fresh.inherit_parameter(&copied, caller).is_err());
    assert!(fresh.inherit_parameter(&helper.params[0], caller).is_err());
    fresh.leave_frame().unwrap();
    fresh.abandon_views().unwrap();
    fresh.settle(&mut owners[0]).unwrap();
}
