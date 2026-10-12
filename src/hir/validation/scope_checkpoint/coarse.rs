//! Complete saved coarse receiving methods from 18d77555f.
use super::*;
#[rustfmt::skip]
impl Scope {
    pub(super) fn copy_binding_from_coarse(
        &mut self,
        source: &Scope,
        key: &ValueId,
    ) -> Result<(), Diagnostic> {
        let Some(value) = tree::value(source.root.as_deref(), key) else {
            return Ok(());
        };
        let nodes = usize::from(tree::height(&self.root))
            .checked_mul(3)
            .and_then(|n| n.checked_add(8));
        charge::reserve(nodes.and_then(|n| n.checked_mul(charge::carrier::<tree::Node>())))?;
        self.root = Some(tree::insert(self.root.take(), key.clone(), value.clone()));
        Ok(())
    }
    pub(super) fn insert_coarse(
        &mut self,
        key: ValueId,
        value: ValidationBinding,
    ) -> Result<(), Diagnostic> {
        self.prepare_insert_coarse()?.insert(key, value);
        Ok(())
    }
    pub(super) fn prepare_insert_coarse(&mut self) -> Result<CoarsePreparedInsert<'_>, Diagnostic> {
        // One new binding carrier plus the complete path and double rotation
        // headroom; all prior versions remain live and receive no refund.
        let nodes = usize::from(tree::height(&self.root))
            .checked_mul(3)
            .and_then(|n| n.checked_add(8));
        charge::reserve(
            nodes
                .and_then(|n| n.checked_mul(charge::carrier::<tree::Node>()))
                .and_then(|n| n.checked_add(charge::carrier::<ValidationBinding>())),
        )?;
        Ok(CoarsePreparedInsert { scope: self })
    }
}
// Only Scope can construct this paid owner. Its exclusive borrow pins the
// receiving root between payment and the one consuming insertion. Dropping
// the owner (including after payload-payment refusal) leaves the root intact.
pub(super) struct CoarsePreparedInsert<'a> {
    scope: &'a mut Scope,
}
impl CoarsePreparedInsert<'_> {
    pub(super) fn insert(self, key: ValueId, value: ValidationBinding) {
        self.scope.root = Some(tree::insert(self.scope.root.take(), key, Rc::new(value)));
    }
}
