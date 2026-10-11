//! Exact private Value receivers. Their caller must hold the validated current
//! partition program plus actual owner-root/edge binding; no default installer.
use super::{flat::PartitionVec, loans::Loans, InvocationRuntime};
use crate::interpreter::{construction_fusion::Backing, Flow, Value};

fn refused(_: crate::construction_fusion::domain::Refusal) -> Flow {
    Flow::Guard("ordinary partition receiving authority refused")
}
/// Called at the canonical whole-Vec/iterator or individual Bytes finalizer.
/// It does not recursively guess aggregate field order or settle a borrowed ID.
pub(super) fn settle<R: InvocationRuntime>(
    value: &mut Value,
    receiver: &mut Loans<'_, R>,
) -> Result<(), Flow> {
    match value {
        Value::Bytes(bytes) if matches!(bytes.bytes, Backing::Partition(_)) => {
            bytes.settle_partition(receiver)
        }
        Value::PartitionVec(vector) => vector.settle(receiver).map_err(refused),
        Value::PartitionIter(iterator) => iterator.settle(receiver).map_err(refused),
        _ => Err(Flow::Guard(
            "partition finalizer requires its exact owned carrier",
        )),
    }
}
/// Move the already paid affine container. No legacy Arc alias is substituted.
pub(super) fn into_iterator<R: InvocationRuntime>(
    value: Value,
    receiver: &mut Loans<'_, R>,
) -> Result<Value, (Flow, Value)> {
    let Value::PartitionVec(vector) = value else {
        return Err((
            Flow::Guard("partition iterator requires its affine Vec"),
            value,
        ));
    };
    match vector.into_iter(receiver) {
        Ok(iterator) => Ok(Value::PartitionIter(iterator)),
        Err((error, vector)) => Err((refused(error), Value::PartitionVec(vector))),
    }
}
/// Numeric classification, funding and complete child publication precede the
/// Value constructor. Language IterStep publication is not performed here.
pub(super) fn partition<R: InvocationRuntime>(
    receiver: &mut Loans<'_, R>,
    owner: R::Owner,
    count: u64,
    width: u64,
) -> Result<Value, (crate::construction_fusion::partition::Preparation, R::Owner)> {
    PartitionVec::partition(receiver, owner, count, width).map(Value::PartitionVec)
}
