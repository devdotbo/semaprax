//! Private receiving state. Preparation is not an evaluator admission grant.
//! The installer still has to bind actual executed roots and canonical edges.
use super::{authority::LoanCapacity, funding::BorrowedAccount, loans::Loans};
use crate::construction_fusion::domain::owner_core::Runtime;
use crate::{
    construction_fusion::domain::Refusal,
    hir::{self, ResolvedFunction},
};
use std::alloc::Layout;
pub(in crate::interpreter) struct Invocation<'a> {
    pub(in crate::interpreter) account: BorrowedAccount<'a>,
    pub(in crate::interpreter) loans: Loans<'a, BorrowedAccount<'a>>,
    entry: &'a ResolvedFunction,
    simultaneous_views: usize,
}
impl<'a> Invocation<'a> {
    pub(in crate::interpreter) fn prepare(
        owner: &'a hir::byte_partition::ValidatedPartitionProgram<'_>,
        entry: &'a ResolvedFunction,
        work: &mut usize,
        mut account: BorrowedAccount<'a>,
    ) -> Result<Self, Refusal> {
        // Consume this bound once at the exact invocation entry, never per helper.
        let simultaneous_views = owner
            .simultaneous_view_bound(entry, work)
            .map_err(|_| Refusal::Identity)?;
        let capacity = LoanCapacity::from_source(owner, entry, work)?;
        // Loans already pays its own retained header. Fund only the actual
        // enclosing entry/bound/handle remainder, without a second Loans debit.
        let header = std::mem::size_of::<Self>()
            .checked_sub(std::mem::size_of::<Loans<'a, BorrowedAccount<'a>>>())
            .ok_or(Refusal::Metadata)?;
        account.reserve_metadata(
            Layout::from_size_align(header, std::mem::align_of::<Self>())
                .map_err(|_| Refusal::Metadata)?,
        )?;
        let loans = Loans::new(capacity, account)?;
        Ok(Self {
            account,
            loans,
            entry,
            simultaneous_views,
        })
    }
    pub(in crate::interpreter) fn entry(&self) -> &ResolvedFunction {
        self.entry
    }
    pub(in crate::interpreter) fn simultaneous_views(&self) -> usize {
        self.simultaneous_views
    }
}
