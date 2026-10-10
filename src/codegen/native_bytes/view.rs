//! Borrowed invocation context; physical storage remains shared with the plan.
use super::*;
use crate::cleanup_plan::{BlockId, CleanupPlan};
#[derive(Clone, Copy)]
pub(in crate::codegen) struct NativeBytesView<'a> {
    plan: &'a NativeBytesPlan,
    cleanup: &'a CleanupPlan,
    resume: Option<BlockId>,
}
impl<'a> NativeBytesView<'a> {
    pub(in crate::codegen) fn ordinary(
        plan: &'a NativeBytesPlan,
        cleanup: &'a CleanupPlan,
    ) -> Self {
        Self {
            plan,
            cleanup,
            resume: None,
        }
    }
    pub(in crate::codegen) fn new(
        plan: &'a NativeBytesPlan,
        cleanup: &'a CleanupPlan,
        resume: Option<BlockId>,
    ) -> Result<Self, Diagnostic> {
        if resume.is_some_and(|entry| cleanup.lexical_resume(entry).is_none()) {
            return Err(error("native cleanup resume domain is absent or ambiguous"));
        }
        Ok(Self {
            plan,
            cleanup,
            resume,
        })
    }
    pub(in crate::codegen) fn result_at(&self, at: &ExpressionId) -> Option<&'a str> {
        producer::result_at(self.plan, self.transitions(), at)
    }
    pub(in crate::codegen) fn apply_try_success_at(
        &self,
        at: &ExpressionId,
        case: &DeclarationId,
    ) -> Result<(String, Option<&'a str>), Diagnostic> {
        let (output, _) = NativeBytesProducer::apply_try_success_at(self, at, case)?;
        let result = producer::try_success_result_at(self.plan, self.variant_cases(), at, case);
        Ok((output, result))
    }
    pub(in crate::codegen) fn value(&self, storage: &StorageId) -> Result<&'a str, Diagnostic> {
        self.plan.value(storage)
    }
    pub(in crate::codegen) fn value_at(&self, place: &CleanupPlace) -> Result<&'a str, Diagnostic> {
        self.plan.value_at(place)
    }
    pub(in crate::codegen) fn projected_value(
        &self,
        storage: &StorageId,
        path: &[DeclarationId],
    ) -> Result<&'a str, Diagnostic> {
        self.plan.projected_value(storage, path)
    }
    pub(in crate::codegen) fn projected_value_if_present(
        &self,
        storage: &StorageId,
        path: &[DeclarationId],
    ) -> Option<&'a str> {
        self.plan.projected_value_if_present(storage, path)
    }
    pub(in crate::codegen) fn variant_value_if_present(
        &self,
        storage: &StorageId,
        case: &DeclarationId,
        field: &DeclarationId,
    ) -> Option<&'a str> {
        self.plan.variant_value_if_present(storage, case, field)
    }
    pub(in crate::codegen) fn provisional(&self) -> Result<(&'a str, &'a str), Diagnostic> {
        self.plan.provisional()
    }
    pub(in crate::codegen) fn call_argument(
        &self,
        call: &ExpressionId,
        index: u32,
    ) -> Result<(&'a str, &'a str, bool), Diagnostic> {
        self.plan.call_argument(call, index)
    }
    pub(in crate::codegen) fn scope_exit(
        &self,
        anchors: &BTreeSet<StorageId>,
    ) -> Result<String, Diagnostic> {
        self.plan
            .scope_exit_in_context(anchors, Some((self.cleanup, self.resume)))
    }
    pub(in crate::codegen) fn block_scope_exit(
        &self,
        anchors: &BTreeSet<StorageId>,
        direct: &BTreeSet<StorageId>,
    ) -> Result<String, Diagnostic> {
        self.plan
            .block_scope_exit_in_context(anchors, direct, Some((self.cleanup, self.resume)))
    }
    pub(in crate::codegen) fn scalar_match_guard_scope_exit(
        &self,
        guard: &ExpressionId,
        anchors: &BTreeSet<StorageId>,
    ) -> Result<String, Diagnostic> {
        self.plan.scalar_match_guard_scope_exit_in_context(
            guard,
            anchors,
            Some((self.cleanup, self.resume)),
        )
    }
    pub(in crate::codegen) fn scalar_match_value_scope_exit(
        &self,
        scrutinee: &ExpressionId,
        anchors: &BTreeSet<StorageId>,
    ) -> Result<String, Diagnostic> {
        self.plan.scalar_match_value_scope_exit_in_context(
            scrutinee,
            anchors,
            Some((self.cleanup, self.resume)),
        )
    }
    pub(in crate::codegen) fn with_resume(self, entry: BlockId) -> Result<Self, Diagnostic> {
        Self::new(self.plan, self.cleanup, Some(entry))
    }
}
impl std::ops::Deref for NativeBytesView<'_> {
    type Target = NativeBytesPlan;
    fn deref(&self) -> &NativeBytesPlan {
        self.plan
    }
}
impl NativeBytesProducer for NativeBytesView<'_> {
    fn base(&self) -> &NativeBytesPlan {
        self.plan
    }
    fn transitions(&self) -> origin::Selection<'_, ExpressionId> {
        self.plan
            .transitions
            .select(self.resume.map(|entry| (self.cleanup, entry)))
    }
    fn variant_cases(&self) -> origin::Selection<'_, (ExpressionId, DeclarationId)> {
        self.plan
            .variant_case_transitions
            .select(self.resume.map(|entry| (self.cleanup, entry)))
    }
}
