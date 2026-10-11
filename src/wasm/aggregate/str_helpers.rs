//! Closed borrowed-text helper sections shared by the internal aggregate route.
use super::*;

pub(super) fn helper_type(
    count: u32,
    types: &mut Vec<Signature>,
    indexes: &mut HashMap<Signature, u32>,
) -> Option<u32> {
    (count != 0).then(|| {
        intern_type(
            Signature {
                params: vec![I64, I64],
                results: vec![I32],
            },
            types,
            indexes,
        )
    })
}

pub(super) fn insert_indexes(
    count: u32,
    indexes: &mut HashMap<FunctionExecutionId, u32>,
) -> Result<(), Diagnostic> {
    if count == 0 {
        return Ok(());
    }
    let base = indexes
        .values()
        .copied()
        .min()
        .and_then(|index| index.checked_sub(count))
        .ok_or_else(|| error("borrowed text helper index extent is absent"))?;
    for (offset, id) in [crate::str_ops::STARTS_WITH_ID, crate::str_ops::CONTAINS_ID]
        .iter()
        .enumerate()
    {
        indexes.insert(
            FunctionExecutionId::Monomorphic(DeclarationId::new(*id)),
            base + offset as u32,
        );
    }
    Ok(())
}

pub(super) fn append_bodies(count: u32, code: &mut Vec<u8>) {
    if count == 0 {
        return;
    }
    let mut body = Vec::new();
    super::super::text_exports::emit_starts_with_body(&mut body);
    write_u32(code, body.len() as u32);
    code.extend(body);
    let mut body = Vec::new();
    super::super::text_exports::emit_contains_bounded_scan_body(&mut body);
    write_u32(code, body.len() as u32);
    code.extend(body);
}

// Internal aggregate String providers authenticate both tagged owned extents
// and ordinary guest ranges. A Str view preserves that same carrier; rotating
// it into a raw-memory helper would reinterpret a host token as a pointer.
impl Emitter<'_> {
    pub(super) fn owned_str_predicate(
        &mut self,
        op: crate::str_ops::StrOp,
        values: &[Value],
    ) -> Option<u32> {
        self.owned_str_predicate_pair(op, &values[0], &values[1])
    }

    pub(super) fn owned_str_predicate_pair(
        &mut self,
        op: crate::str_ops::StrOp,
        left: &Value,
        right: &Value,
    ) -> Option<u32> {
        let operation = match op {
            crate::str_ops::StrOp::StartsWith => crate::string_ops::StringOp::StartsWith,
            crate::str_ops::StrOp::Contains => crate::string_ops::StringOp::Contains,
            _ => return None,
        };
        let index = self
            .function_indexes
            .get(&FunctionExecutionId::Monomorphic(DeclarationId::new(
                operation.id(),
            )))
            .copied()?;
        self.get_scalar(left);
        self.get_scalar(right);
        Some(index)
    }
}

#[cfg(test)]
#[path = "str_helpers/tests.rs"]
mod tests;
