//! Borrowed main shape validation. Discarded metadata never becomes a directory.
use super::*;
use crate::hir::ResolvedFieldDeclaration;
use inventory_lifecycle::{outcome, variants};
pub(super) enum Frame<'a> {
    Enter(&'a ResolvedType, usize),
    Children(&'a [ResolvedFieldDeclaration], usize, usize),
}
impl<'s> Builder<'s, '_> {
    pub(super) fn shape(
        &mut self,
        root: &'s ResolvedType,
    ) -> Result<(crate::cleanup::CleanupShapeProfile, bool), Diagnostic> {
        // Preserve the complete original per-root header/work authentication.
        // Only its previously empty backing is exchanged for this receiver's
        // prepaid retained frontier. A panic drops the lent backing normally.
        let mut frames = inventory_flags::vector(self.payer)?;
        std::mem::swap(&mut frames, &mut self.shape_frames);
        let result = self.shape_with(root, &mut frames);
        frames.clear();
        std::mem::swap(&mut frames, &mut self.shape_frames);
        result
    }
    fn shape_with(
        &mut self,
        root: &'s ResolvedType,
        frames: &mut Vec<Frame<'s>>,
    ) -> Result<(crate::cleanup::CleanupShapeProfile, bool), Diagnostic> {
        let mut profile = profile::Tracker::default();
        inventory_flags::push(frames, Frame::Enter(root, 0), self.payer)?;
        loop {
            metadata::step(self.payer, 1)?;
            let Some(frame) = frames.pop() else {
                break;
            };
            match frame {
                Frame::Children(fields, index, depth) => {
                    let Some(field) = fields.get(index) else {
                        continue;
                    };
                    inventory_flags::push(
                        frames,
                        Frame::Children(fields, index + 1, depth),
                        self.payer,
                    )?;
                    inventory_flags::push(frames, Frame::Enter(&field.ty, depth), self.payer)?;
                }
                Frame::Enter(ty, depth) => {
                    if !self.needs_drop(ty)? {
                        continue;
                    }
                    if let Some(id) = variants::primitive(ty, self.payer)? {
                        self.leaf(id, depth, &mut profile)?;
                        continue;
                    }
                    if bounded_vec(ty, self.payer)?
                        || owned_vec(self.program.declarations(), ty, self.payer)?
                        || outcome::field(self.program.declarations(), ty, self.payer)?
                        || variants::is_iter(ty, self.payer)?
                    {
                        let id = if variants::is_iter(ty, self.payer)? {
                            crate::cleanup::ITER_DROP_LIFECYCLE_ID
                        } else {
                            crate::cleanup::VEC_DROP_LIFECYCLE_ID
                        };
                        self.leaf(id, depth, &mut profile)?;
                        continue;
                    }
                    if bounded_box(ty, self.payer)? {
                        self.leaf(crate::cleanup::BOX_DROP_LIFECYCLE_ID, depth, &mut profile)?;
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
                                format_args!("droppable type `{key}` is not nominal"),
                            )?,
                        ));
                    };
                    let program = self.program;
                    let Some(item) = self.find(declaration, program)? else {
                        return Err(self.unknown(declaration)?);
                    };
                    match &item.kind {
                        ResolvedTypeDeclarationKind::Resource { drop } => {
                            if !arguments.is_empty() {
                                return Err(self.unsupported(ty)?);
                            }
                            self.leaf(drop.id.as_str(), depth, &mut profile)?;
                        }
                        ResolvedTypeDeclarationKind::Record { fields }
                        | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                            if !arguments.is_empty() {
                                if !matches!(item.kind, ResolvedTypeDeclarationKind::Record { .. })
                                {
                                    return Err(self.unsupported(ty)?);
                                }
                                self.generic(ty, depth, &mut profile)?;
                            } else {
                                let child_depth =
                                    profile.record(depth, fields.len(), self.payer)?;
                                inventory_flags::push(
                                    frames,
                                    Frame::Children(fields, 0, child_depth),
                                    self.payer,
                                )?;
                            }
                        }
                        ResolvedTypeDeclarationKind::Variant { cases } => {
                            profile.variant(cases, self.payer)?;
                            for case in cases {
                                metadata::step(self.payer, 1)?;
                                for field in &case.fields {
                                    metadata::step(self.payer, 1)?;
                                    let field_ty = inventory_substitute::substitute(
                                        &field.ty,
                                        declaration,
                                        arguments,
                                        self.payer,
                                    )?;
                                    if variants::record(
                                        self.program,
                                        ty,
                                        &case.id,
                                        &field.id,
                                        &field_ty,
                                        self.payer,
                                    )? {
                                        self.generic(&field_ty, depth, &mut profile)?;
                                    } else if self.needs_drop(&field_ty)? {
                                        let id=variants::leaf(self.program,ty,&case.id,&field.id,&field_ty,self.payer)?.ok_or_else(|| error("droppable variant field is outside its admitted cleanup profile"))?;
                                        self.leaf(id, depth, &mut profile)?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        profile.finish(self.payer)
    }
}
pub(super) fn bounded_vec(ty: &ResolvedType, payer: &mut Payer<'_>) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    if !inventory_text::same_bytes(declaration.as_str(), crate::prelude::VEC_ID, payer)? {
        return Ok(false);
    }
    metadata::step(payer, 1)?;
    Ok(
        matches!(arguments.as_slice(),[element] if crate::vec_ops::resolved_vec_element_is_admitted(element)),
    )
}
pub(super) fn bounded_box(ty: &ResolvedType, payer: &mut Payer<'_>) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    if !inventory_text::same_bytes(declaration.as_str(), crate::prelude::BOX_ID, payer)? {
        return Ok(false);
    }
    metadata::step(payer, 1)?;
    Ok(
        matches!(arguments.as_slice(),[element] if crate::box_ops::resolved_box_element_is_admitted(element)),
    )
}
pub(super) fn owned_vec(
    index: &DeclarationIndex,
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    if !inventory_text::same_bytes(declaration.as_str(), crate::prelude::VEC_ID, payer)? {
        return Ok(false);
    }
    let [element] = arguments.as_slice() else {
        return Ok(false);
    };
    let probes = index.inventory_probes();
    metadata::with_work(payer, |work| {
        crate::hir::owned_record_collection::element_with_work(&probes, element, work)
    })
}
