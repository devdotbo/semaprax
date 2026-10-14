//! Full generic record receiver; reverse substitution and forward child visits.
use super::*;
use inventory_lifecycle::{outcome, variants};
use std::borrow::Cow;
enum Frame<'a> {
    Enter(Cow<'a, ResolvedType>, usize, usize),
    Leave(String),
}
impl Builder<'_, '_> {
    pub(super) fn generic(
        &mut self,
        root: &ResolvedType,
        record_depth: usize,
        profile: &mut profile::Tracker,
    ) -> Result<(), Diagnostic> {
        let mut frames = inventory_flags::vector(self.payer)?;
        inventory_flags::push(
            &mut frames,
            Frame::Enter(Cow::Borrowed(root), 1, record_depth),
            self.payer,
        )?;
        let mut active = inventory_ordered::Map::<()>::acquire(self.payer)?;
        let mut visited_fields = 0usize;
        let mut owned_leaves = 0usize;
        loop {
            metadata::step(self.payer, 1)?;
            let Some(frame) = frames.pop() else {
                break;
            };
            match frame {
                Frame::Leave(key) => {
                    active.remove(&key, self.payer)?;
                }
                Frame::Enter(ty, depth, record_depth) => {
                    let ty = ty.as_ref();
                    if !self.needs_drop(ty)? {
                        continue;
                    }
                    let lifecycle = if let Some(id) = variants::primitive(ty, self.payer)? {
                        Some(id)
                    } else if outcome::field(self.program.declarations(), ty, self.payer)?
                        || shape::bounded_vec(ty, self.payer)?
                        || shape::owned_vec(self.program.declarations(), ty, self.payer)?
                    {
                        Some(crate::cleanup::VEC_DROP_LIFECYCLE_ID)
                    } else if variants::is_iter(ty, self.payer)? {
                        Some(crate::cleanup::ITER_DROP_LIFECYCLE_ID)
                    } else if shape::bounded_box(ty, self.payer)? {
                        Some(crate::cleanup::BOX_DROP_LIFECYCLE_ID)
                    } else {
                        None
                    };
                    if let Some(id) = lifecycle {
                        owned_leaves = owned_leaves
                            .checked_add(1)
                            .ok_or_else(|| error("generic cleanup owned-leaf count overflowed"))?;
                        if owned_leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
                            return Err(error(
                                "generic cleanup shape exceeds its owned-leaf limit",
                            ));
                        }
                        self.leaf(id, record_depth, profile)?;
                        continue;
                    }
                    let ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } = ty
                    else {
                        let key = inventory_raw_key::identity(ty, self.payer)?;
                        return Err(Diagnostic::io(
                            "SPX-H006",
                            inventory_text::raw(
                                self.payer,
                                format_args!("droppable generic field `{key}` is not nominal"),
                            )?,
                        ));
                    };
                    if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
                        return Err(error(
                            "generic cleanup shape exceeds its record-depth limit",
                        ));
                    }
                    let program = self.program;
                    let Some(item) = self.find(declaration, program)? else {
                        return Err(self.unknown(declaration)?);
                    };
                    let ResolvedTypeDeclarationKind::Record { fields } = &item.kind else {
                        return Err(error(
                            "generic cleanup descendant is not an admitted record",
                        ));
                    };
                    let key = inventory_raw_key::identity(ty, self.payer)?;
                    if active.get(&key, self.payer)?.is_some() {
                        return Err(error("generic cleanup shape is recursive"));
                    }
                    // Retain a separately funded leave key; insertion owns its
                    // full original key until that exact completed subtree exits.
                    let leave = inventory_text::raw(self.payer, format_args!("{key}"))?;
                    active.insert(key, (), self.payer)?;
                    visited_fields = visited_fields
                        .checked_add(fields.len())
                        .ok_or_else(|| error("generic cleanup visited-field count overflowed"))?;
                    if visited_fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                        return Err(error(
                            "generic cleanup shape exceeds its visited-field limit",
                        ));
                    }
                    let child_depth = profile.record(record_depth, fields.len(), self.payer)?;
                    inventory_flags::push(&mut frames, Frame::Leave(leave), self.payer)?;
                    for field in fields.iter().rev() {
                        metadata::step(self.payer, 1)?;
                        let child = inventory_substitute::substitute(
                            &field.ty,
                            declaration,
                            arguments,
                            self.payer,
                        )?;
                        inventory_flags::push(
                            &mut frames,
                            Frame::Enter(Cow::Owned(child), depth + 1, child_depth),
                            self.payer,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
}
