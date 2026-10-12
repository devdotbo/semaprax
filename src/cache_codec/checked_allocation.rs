//! Append-only v20 data carriers. Decoding is never allocation authority.
use super::{codec_enum, Codec, Decoder, Encoder, Result};
use crate::cleanup_plan::{AllocationCase, AllocationOperation, CleanupPlan, StatusLane,
    StatusProducer, CLEANUP_PLAN_SCHEMA_V20};

codec_enum!(AllocationCase { 0 => PayloadRefused, 1 => SizeOverflow });
codec_enum!(AllocationOperation {
    0 => StringLiteral, 1 => StringClone, 2 => StringConcat, 3 => StringFromChar,
    4 => StringFromI64, 5 => StringFromUsize, 6 => StringFromStr, 7 => StringFromUtf8,
    8 => StringSlice, 9 => StringTrim, 10 => LiteralFormat, 11 => BytesCopy,
    12 => BytesZeroed, 13 => VecBacking, 14 => VecElementClone, 15 => MapBacking,
    16 => MapKeyClone, 17 => SetBacking, 18 => SetKeyClone, 19 => BoxBacking,
    20 => RecordClone, 21 => ClosureBacking, 22 => ClosureCaptureClone
});
impl Codec for [AllocationCase; 2] {
    fn encode(&self, out: &mut Encoder) -> Result<()> {
        if *self != [AllocationCase::PayloadRefused, AllocationCase::SizeOverflow] {
            return Err(super::grammar("allocation normalized cases differ from v1"));
        }
        out.nested(|out| { self[0].encode(out)?; self[1].encode(out) })
    }
    fn decode(input: &mut Decoder<'_>) -> Result<Self> {
        input.nested(|input| {
            let cases = [AllocationCase::decode(input)?, AllocationCase::decode(input)?];
            if cases != [AllocationCase::PayloadRefused, AllocationCase::SizeOverflow] {
                return Err(super::grammar("allocation normalized cases differ from v1"));
            }
            Ok(cases)
        })
    }
}

/// Check the enclosing selective schema on both encode and decode, so append-
/// only enum tags cannot smuggle a new failure lane into an old plan contract.
pub(super) fn validate(plan: &CleanupPlan) -> Result<()> {
    for source in &plan.status_sources {
        match (&source.id.lane, &source.producer) {
            (StatusLane::AllocationFailure, StatusProducer::CheckedAllocation { normalized_cases, .. })
                if plan.schema == CLEANUP_PLAN_SCHEMA_V20
                    && *normalized_cases == [AllocationCase::PayloadRefused, AllocationCase::SizeOverflow] => {},
            (StatusLane::AllocationFailure, _) | (_, StatusProducer::CheckedAllocation { .. }) =>
                return Err(super::grammar("allocation source requires its exact v20 provenance")),
            _ => {},
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests;
