//! Allocation-free refusal envelope. The exact failed ledger remains affine.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SetupReason {
    Reservation,
    CloneCursor,
    TextOverflow,
    TextReservation,
    TextCapacity,
}
impl SetupReason {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::Reservation => "cleanup replay physical reservation exceeds the bound",
            Self::CloneCursor => "cleanup replay clone cursor reservation refused",
            Self::TextOverflow => "cleanup replay refusal layout overflowed",
            Self::TextReservation => "cleanup replay refusal text reservation refused",
            Self::TextCapacity => "cleanup replay refusal allocator capacity differs",
        }
    }
}
pub(crate) struct SetupFailure {
    physical: PhysicalReservation,
    reason: SetupReason,
}
impl SetupFailure {
    pub(crate) fn into_parts(self) -> (PhysicalReservation, SetupReason) {
        (self.physical, self.reason)
    }
}
impl ReplayBudget {
    pub(in crate::cleanup_plan::replay) fn try_from_reservation(
        function: &ResolvedFunction,
        mut physical: PhysicalReservation,
    ) -> Result<Self, SetupFailure> {
        macro_rules! refused {
            ($reason:expr) => {{
                physical.reject();
                return Err(SetupFailure {
                    physical,
                    reason: $reason,
                });
            }};
        }
        if physical.reserve_array::<PhysicalReservation>(1).is_err() {
            refused!(SetupReason::Reservation);
        }
        if physical
            .reserve_array::<Option<(&ResolvedType, usize)>>(514)
            .is_err()
        {
            refused!(SetupReason::CloneCursor);
        }
        let Some(capacity) = function.id.as_str().len().checked_add(REFUSAL_FIXED_BYTES) else {
            refused!(SetupReason::TextOverflow);
        };
        if physical.reserve(capacity).is_err() {
            refused!(SetupReason::TextReservation);
        }
        let refusal_text = String::with_capacity(capacity);
        if refusal_text.capacity() != capacity {
            refused!(SetupReason::TextCapacity);
        }
        Ok(Self {
            refusal_text: Some(refusal_text),
            physical: Some(Box::new(physical)),
            ..Self::new()
        })
    }
}
