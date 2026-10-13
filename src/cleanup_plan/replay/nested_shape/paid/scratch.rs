//! One complete inventory receiver owns this paid backing, never a verdict.
use super::*;
pub(super) struct CaseMeta {
    pub(super) case: DeclarationId,
    pub(super) case_index: u32,
    pub(super) fields: Vec<(DeclarationId, u32)>,
}
pub(super) enum Frame {
    Enter(ResolvedType, usize),
    FinishRecord(DeclarationId, Vec<(DeclarationId, u32)>),
    FinishVariant(DeclarationId, Vec<CaseMeta>),
}

#[derive(Default)]
pub(in crate::cleanup_plan::replay) struct Scratch {
    // Match the original reverse-local destruction order, including backing.
    shapes: Vec<FieldLivenessShape>,
    frames: Vec<Frame>,
}

pub(super) struct Lease<'a>(&'a mut Scratch);
impl Scratch {
    pub(super) fn lease(&mut self) -> Lease<'_> {
        Lease(self)
    }
    #[cfg(test)]
    pub(in crate::cleanup_plan::replay) fn capacities(&self) -> (usize, usize) {
        (self.frames.capacity(), self.shapes.capacity())
    }
    #[cfg(test)]
    pub(in crate::cleanup_plan::replay) fn settled(&self) -> bool {
        self.frames.is_empty() && self.shapes.is_empty()
    }
}
impl Lease<'_> {
    pub(super) fn parts(&mut self) -> (&mut Vec<Frame>, &mut Vec<FieldLivenessShape>) {
        (&mut self.0.frames, &mut self.0.shapes)
    }
}
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        self.0.shapes.clear();
        self.0.frames.clear();
    }
}
