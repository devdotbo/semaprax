//! One account for every Bytes backing on the private selected route.
//! Ordinary None execution keeps its existing allocation and status policy.
use super::funding::{AllocationError, BorrowedAccount};
use crate::interpreter::{
    construction_fusion::{Backing, WholeBacking},
    BorrowedSliceValue, Evaluator, Flow, OwnedBytesValue,
};
use std::sync::Arc;

fn administrative(_: AllocationError) -> Flow {
    Flow::Guard("partition invocation backing account refused")
}
fn logical_copy(error: AllocationError) -> Flow {
    match error {
        AllocationError::Logical => Flow::Guard("owned byte allocation exceeds verified capacity"),
        error => administrative(error),
    }
}
fn whole(account: BorrowedAccount<'_>, (allocation, bytes): (u32, Box<[u8]>)) -> OwnedBytesValue {
    OwnedBytesValue {
        allocation,
        bytes: Backing::Whole(WholeBacking {
            account: account.identity(),
            bytes,
        }),
    }
}
impl Evaluator<'_> {
    pub(in crate::interpreter) fn byte_allocation_count(&self) -> u32 {
        self.partition
            .as_ref()
            .map_or(self.next_byte_allocation, |invocation| {
                invocation.account.counters().0
            })
    }
    pub(in crate::interpreter) fn preflight_accounted_store(
        &mut self,
        value: &OwnedBytesValue,
    ) -> Result<(), Flow> {
        let invocation = self
            .partition
            .as_ref()
            .ok_or(Flow::Guard("missing partition invocation account"))?;
        let account = invocation.account;
        if !account.active() {
            return Err(Flow::Guard(
                "partition invocation account is already refused",
            ));
        }
        match &value.bytes {
            Backing::Whole(whole)
                if whole.account == account.identity()
                    && value.allocation > 0
                    && value.allocation <= account.counters().0 =>
            {
                Ok(())
            }
            _ => {
                account.poison();
                Err(Flow::Guard(
                    "partition store requires its current whole owner receiver",
                ))
            }
        }
    }
    pub(in crate::interpreter) fn zeroed_bytes(
        &mut self,
        length: u64,
    ) -> Result<OwnedBytesValue, Flow> {
        let Some(invocation) = &self.partition else {
            return super::super::owned_buffer::zeroed(
                length,
                &mut self.next_byte_allocation,
                &mut self.allocated_byte_payload,
            );
        };
        if length > crate::byte_ops::MAX_BUFFER_CAPACITY_BYTES {
            return Err(Flow::Guard(
                "owned byte buffer capacity exceeds the verified profile limit",
            ));
        }
        let length = usize::try_from(length)
            .map_err(|_| Flow::Guard("owned byte buffer capacity does not fit usize"))?;
        invocation
            .account
            .allocate_zeroed(length)
            .map(|value| whole(invocation.account, value))
            .map_err(logical_copy)
    }
    pub(in crate::interpreter) fn copy_bytes(
        &mut self,
        value: &BorrowedSliceValue,
    ) -> Result<OwnedBytesValue, Flow> {
        if let Some(invocation) = &mut self.partition {
            let account = invocation.account;
            return if value.partition.is_some() {
                value.read_partition(&mut invocation.loans, |bytes| {
                    account
                        .copy(bytes)
                        .map(|value| whole(account, value))
                        .map_err(logical_copy)
                })?
            } else {
                // A selected invocation cannot import an old fused lease.
                if value.lease.is_some() {
                    return Err(Flow::Guard("foreign fused view in partition invocation"));
                }
                account
                    .copy(value.bytes())
                    .map(|value| whole(account, value))
                    .map_err(logical_copy)
            };
        }
        let length = u64::try_from(value.read(&mut self.fused, <[u8]>::len)?)
            .map_err(|_| Flow::Guard("owned byte payload length does not fit u64"))?;
        if length > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES {
            return Err(Flow::Guard(
                "owned byte payload exceeds the verified profile limit",
            ));
        }
        let next_count = self
            .next_byte_allocation
            .checked_add(1)
            .ok_or(Flow::Guard("owned byte allocation count overflowed"))?;
        if next_count > crate::byte_data_capacity::MAX_BYTES_COPY_SITES {
            return Err(Flow::Guard(
                "owned byte allocation count exceeds verified capacity",
            ));
        }
        let next_payload = self
            .allocated_byte_payload
            .checked_add(length)
            .ok_or(Flow::Guard("owned byte payload accounting overflowed"))?;
        if next_payload > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES {
            return Err(Flow::Guard("owned byte payload exceeds verified capacity"));
        }
        self.next_byte_allocation = next_count;
        self.allocated_byte_payload = next_payload;
        Ok(OwnedBytesValue {
            allocation: next_count,
            bytes: value
                .read(&mut self.fused, |bytes| Arc::<[u8]>::from(bytes))?
                .into(),
        })
    }
    pub(in crate::interpreter) fn clone_accounted_bytes(
        &mut self,
        value: &OwnedBytesValue,
    ) -> Result<OwnedBytesValue, Flow> {
        let invocation = self
            .partition
            .as_mut()
            .ok_or(Flow::Guard("missing partition invocation account"))?;
        if !invocation.account.active() {
            return Err(Flow::Guard(
                "partition invocation account is already refused",
            ));
        }
        if value.allocation == 0 || value.allocation > invocation.account.counters().0 {
            invocation.account.poison();
            return Err(Flow::Guard(
                "partition clone has invalid allocation identity",
            ));
        }
        let account: BorrowedAccount<'_> = invocation.account;
        let copy = |bytes: &[u8]| {
            account
                .copy(bytes)
                .map(|value| whole(account, value))
                .map_err(|error| match error {
                    AllocationError::Logical => {
                        Flow::Failure(super::super::owned_vec::normalize_vec(
                            crate::vec_ops::ALLOCATION_FAILURE_CODE,
                        ))
                    }
                    error => administrative(error),
                })
        };
        match &value.bytes {
            Backing::Whole(whole) if whole.account == account.identity() => copy(&whole.bytes),
            Backing::Whole(_) => {
                account.poison();
                Err(Flow::Guard(
                    "whole Bytes belongs to another invocation account",
                ))
            }
            Backing::Partition(owner) => {
                let bytes = invocation.loans.deep_clone(owner).map_err(|_| {
                    if account.logical_refused() {
                        Flow::Failure(super::super::owned_vec::normalize_vec(
                            crate::vec_ops::ALLOCATION_FAILURE_CODE,
                        ))
                    } else {
                        Flow::Guard("partition clone owner authority refused")
                    }
                })?;
                Ok(whole(account, (account.counters().0, bytes)))
            }
            // Borrow descriptors and foreign legacy carriers are not owned leaves.
            _ => {
                account.poison();
                Err(Flow::Guard(
                    "partition clone requires its current affine owner",
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests;
