//! One private host sort, same row storage and one final epoch renewal.
use super::*;
pub(in crate::wasm::aggregate) fn metadata(
    program: &ResolvedProgram,
    element: &ResolvedType,
) -> Result<(i64, i64), Diagnostic> {
    if let Some(fields) = crate::hir::copy_record_collection::fields(&program.declarations, element)
    {
        let mut shape = 0u64;
        let mut hash = Sha256::new();
        hash.update(b"semaprax.wasm.copy-sort-keys.v1\0");
        hash.update(element.identity_key());
        for (i, field) in fields.iter().enumerate() {
            let code = u8::try_from(vec_element_tag(&field.ty)?)
                .map_err(|_| error("invalid Copy sort field tag"))?;
            shape |= u64::from((code << 4) | i as u8) << (i * 8);
            hash.update([0, code]);
            hash.update(field.id.as_str());
            hash.update([0]);
            hash.update(field.ty.identity_key());
        }
        let bytes = hash.finalize();
        Ok((
            i64::from_le_bytes(bytes[..8].try_into().expect("digest prefix")),
            shape as i64,
        ))
    } else {
        let desc = descriptor(program, element)?;
        Ok((desc.identity, desc.shape))
    }
}
pub(in crate::wasm::aggregate) fn import_index(program: &ResolvedProgram) -> u32 {
    import_base(program)
        + (if super::super::super::vec_ops::program_uses_owned_leaf_vec(program) {
            IMPORT_COUNT
        } else {
            0
        })
        + u32::from(super::super::vec_field::uses(program))
}
impl Emitter<'_> {
    pub(in crate::wasm::aggregate) fn explicit_sort(
        &mut self,
        expr: &ResolvedExpr,
        element: &ResolvedType,
        owner: &Value,
        bind_only: bool,
    ) -> Result<(), Diagnostic> {
        let policy = if bind_only {
            0
        } else {
            let positions =
                crate::vec_ops::sort_keys::expression_positions(&self.program.declarations, expr)
                    .ok_or_else(|| error("explicit sort descriptor failed replay"))?;
            positions
                .iter()
                .enumerate()
                .fold(0u64, |out, (i, (position, descending))| {
                    out | (((position + 1) * 2 + usize::from(*descending)) as u64) << (i * 8)
                }) as i64
        };
        let (identity, shape) = metadata(self.program, element)?;
        self.get_scalar(owner);
        self.leaf_const(identity);
        self.leaf_const(shape);
        self.leaf_const(policy);
        self.output.push(0x10);
        write_u32(self.output, import_index(self.program));
        Ok(())
    }
}
