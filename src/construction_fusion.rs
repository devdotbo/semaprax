//! Private immutable construction inventory, never source/profile authority.
//! No serializer, admission hook, ordinary resource discount or backend selector.
use crate::{ast, hir};
pub(crate) mod domain;
#[cfg(test)]
mod tests;
pub(crate) struct Descriptor<'a> {
    source: &'a ast::Program,
    program: &'a hir::ResolvedProgram,
    typed: crate::finite_row_construction::Proof<'a>,
    bytes: &'a hir::ResolvedExpr,
    field: &'a hir::ResolvedFieldDeclaration,
    payload_bytes: u64,
    string_bytes: u64,
}
impl Descriptor<'_> {
    pub(crate) fn typed(&self) -> &crate::finite_row_construction::Proof<'_> {
        &self.typed
    }
    pub(crate) fn vector(&self) -> &hir::ResolvedBinding {
        self.typed.vector
    }
    pub(crate) fn counter(&self) -> &hir::ResolvedBinding {
        self.typed.counter
    }
    pub(crate) fn push(&self) -> &hir::ResolvedExpr {
        self.typed.push
    }
    pub(crate) fn latch(&self) -> &hir::ResolvedExpr {
        self.typed.step
    }
    pub(crate) fn suffix(&self) -> &[hir::ResolvedStatement] {
        self.typed.suffix
    }
    pub(crate) fn tail(&self) -> &hir::ResolvedExpr {
        self.typed.tail
    }
    pub(crate) fn byte_site(&self) -> &hir::ResolvedExpr {
        self.bytes
    }
    pub(crate) fn byte_field(&self) -> &hir::ResolvedFieldDeclaration {
        self.field
    }
    pub(crate) fn payload_bytes(&self) -> u64 {
        self.payload_bytes
    }
    pub(crate) fn string_bytes(&self) -> u64 {
        self.string_bytes
    }
    pub(crate) fn count(&self) -> usize {
        self.typed.iterations as usize
    }
    pub(crate) fn width(&self) -> usize {
        self.typed.width as usize
    }
    /// A consumer must reserve its actual slot/admin/view representations; this
    /// count is not a guessed universal metadata byte multiplier.
    pub(crate) fn lease_slots(&self) -> usize {
        self.count()
    }
    pub(crate) fn still_held(&self, source: &ast::Program, program: &hir::ResolvedProgram) -> bool {
        std::ptr::eq(self.source, source) && std::ptr::eq(self.program, program)
    }
}
/// Independently reconstruct both closed construction shapes. The borrowed
/// source/HIR suffixes remain mandatory executor input, not a stored allow flag.
/// Complete source-to-HIR module authentication and selected lowering are still
/// required before any executor can discount ordinary constructor events.
pub(crate) fn derive<'a>(
    source: &'a ast::Program,
    program: &'a hir::ResolvedProgram,
) -> Option<Descriptor<'a>> {
    let original = crate::source_verify::construction_fusion_candidate(source)?;
    let typed = crate::finite_row_construction::fusion_structure(program)?;
    if source.module != program.module
        || original.function.name != typed.function.name
        || original.function.stable_id != typed.function.id.as_str()
        || original.function.span != typed.function.span
        || original.record.stable_id != typed.record.id.as_str()
        || original.count != u64::from(typed.iterations)
        || original.width != typed.width
        || original.condition.span != typed.condition.span
        || original.body.span != typed.body.span
    {
        return None;
    }
    let ast::TypeDeclarationKind::Record {
        fields: source_fields,
    } = &original.record.kind
    else {
        return None;
    };
    let hir::ResolvedTypeDeclarationKind::Record { fields } = &typed.record.kind else {
        return None;
    };
    let hir::ResolvedExprKind::ConstructRecord { fields: values, .. } = &typed.constructor.kind
    else {
        return None;
    };
    let ast::ExprKind::Block { statements, .. } = &original.body.kind else {
        return None;
    };
    let ast::Statement::Let {
        name: source_item,
        value: source_constructor,
        ..
    } = &statements[0]
    else {
        return None;
    };
    let ast::ExprKind::ConstructRecord {
        fields: source_values,
        ..
    } = &source_constructor.kind
    else {
        return None;
    };
    if typed.item.name != *source_item || source_constructor.span != typed.constructor.span {
        return None;
    }
    let mut bytes = None;
    let mut string_bytes = 0u64;
    for (((source_field, field), source_value), value) in source_fields
        .iter()
        .zip(fields)
        .zip(source_values)
        .zip(values)
    {
        let fact = program.declarations.declaration(&field.id)?;
        if fact.kind != hir::DeclarationKind::Field
            || fact.owner.as_ref() != Some(&typed.record.id)
            || fact.name != field.name
            || fact.identity_origin != hir::IdentityOrigin::Explicit
            || source_field.span != field.span
            || source_value.name != field.name
            || source_field.stable_id != field.id.as_str()
            || source_field.name != field.name
            || source_value.value.span != value.value.span
        {
            return None;
        }
        match (&source_value.value.kind, &value.value.kind, &field.ty) {
            (
                ast::ExprKind::String(left),
                hir::ResolvedExprKind::String(right),
                hir::ResolvedType::String,
            ) if left == right => {
                string_bytes = u64::try_from(right.len())
                    .ok()?
                    .checked_mul(u64::from(typed.iterations))?;
            }
            (_, _, hir::ResolvedType::Bytes) => bytes = Some((&value.value, field)),
            (_, _, hir::ResolvedType::I64) => {}
            _ => return None,
        }
    }
    if string_bytes != original.string_bytes {
        return None;
    }
    let (bytes, field) = bytes?;
    if bytes.span != original.byte_site.span {
        return None;
    }
    Some(Descriptor {
        source,
        program,
        typed,
        bytes,
        field,
        payload_bytes: original.payload_bytes,
        string_bytes,
    })
}
