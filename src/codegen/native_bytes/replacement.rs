//! Successful String replacement consumes only the replay-authenticated old leaf.
use super::*;
impl NativeBytesPlan {
    pub(super) fn release_replaced_string(
        &self,
        transition: &CleanupTransition,
        destination: &ByteSlot,
    ) -> String {
        if !matches!(transition, CleanupTransition::Renew { .. })
            || destination.kind != OwnedLeafKind::String
        {
            return String::new();
        }
        self.emit_finalizers(std::slice::from_ref(destination), false)
    }
}
