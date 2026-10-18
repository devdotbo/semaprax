use super::*;
use crate::cleanup_plan::renewal_profile::child_refusal::Refusal;
use crate::cleanup_plan::replay::selected_context::child::Permit;
use std::mem::size_of;
type Caller<'a> = (
    &'a ResolvedFunction,
    &'a BTreeSet<StorageId>,
    &'a Leaves,
    &'a mut ReplayBudget,
    Result<(), Error>,
);
type Local<'a> = (
    &'a ResolvedFunction,
    &'a BTreeSet<StorageId>,
    &'a Leaves,
    &'a mut ReplayBudget,
    BTreeSet<LivenessFlagId>,
    &'a CleanupPlace,
    LivenessFlagId,
    std::vec::IntoIter<LivenessFlagId>,
    std::slice::Iter<'a, CleanupPlace>,
);
type PermitCall<'a> = (
    &'a ResolvedFunction,
    &'a mut ReplayBudget,
    Result<Permit<'a>, Error>,
);
type Control<'a> = (
    &'a Permit<'a>,
    Reason,
    &'a ResolvedFunction,
    Reason,
    &'a ExpressionId,
    ExpressionId,
    &'a ExpressionId,
    ExpressionId,
    semantic_refusal::Atom,
    semantic_refusal::Receipt,
    Error,
    Result<Permit<'a>, Error>,
);
type OriginalAcquire<'a> = (
    Permit<'a>,
    Refusal,
    Option<Refusal>,
    semantic_refusal::Receipt,
    Error,
    Option<Error>,
    Result<(), Refusal>,
    Result<(), Error>,
    Result<Permit<'a>, Error>,
    Result<selected_context::Context<'a>, Error>,
    Result<(), Error>,
);
fn entry(function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<(), Error> {
    let storage = BTreeSet::new();
    let leaves = Leaves::new();
    crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        Caller<'_>,
        5,
        "selected structural driver call receiving",
        validate_entry(function, &storage, &leaves, budget)
    )
}
#[test]
fn unknown_entry_exact_typed_frontier_and_one_short_keep_work_physical_and_no_allocation() {
    let mut program = program(SCALAR);
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "family.main")
        .unwrap();
    function.cleanup_plan.entry = BlockId(u32::MAX);
    let setup = ReplayBudget::with_physical(function, usize::MAX)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    let request = size_of::<Caller<'_>>()
        + size_of::<Local<'_>>()
        + 2 * size_of::<PermitCall<'_>>()
        + size_of::<Control<'_>>()
        + size_of::<OriginalAcquire<'_>>();
    assert_eq!(
        size_of::<Control<'_>>(),
        size_of::<selected_context::child::ControlReceiving<'_>>()
    );
    // 5 caller +9 local +3 permit caller +12 control +3 acquire caller +11 original.
    const WORK: usize = 43;
    for (bytes, work, semantic) in [
        (setup, WORK, false),
        (setup + request - 1, WORK, false),
        (setup + request, 0, false),
        (setup + request, WORK - 1, false),
        (setup + request, WORK, true),
    ] {
        let mut budget = ReplayBudget::with_physical(function, bytes).unwrap();
        budget.remaining = work;
        let (result, observed) = measure(|| entry(function, &mut budget));
        assert_eq!((observed.allocations, observed.reallocations), (0, 0));
        assert!(!observed.invalid);
        match result.unwrap_err() {
            Error::Structural(receipt) if semantic => {
                assert_eq!(budget.physical.as_ref().unwrap().charged(), setup + request);
                assert_eq!(budget.remaining, 0);
                assert_eq!(
                    receipt.source().as_str().as_ptr(),
                    function.body.id.as_str().as_ptr()
                );
                assert!(matches!(
                    receipt.borrowed_control_receipt().unwrap().1,
                    Reason::Finite(Kind::CleanupEntryBlockIsUnknown)
                ));
            }
            Error::Owned(_) if !semantic => {
                let ledger = budget.physical.as_deref_mut().unwrap();
                let first = ledger.reserve(0).unwrap_err();
                assert_eq!(ledger.reserve(1).unwrap_err(), first);
            }
            other => panic!("wrong first refusal semantic={semantic}: {other:?}"),
        }
    }
}
