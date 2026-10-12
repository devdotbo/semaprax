//! Borrowed field uniqueness over the exact current carrier. The carrier Arc is
//! only read: no unwrap, row clone, payload clone or generation renewal. One
//! fallible ordinal reservation is the sole scratch and is released before the
//! Bool is published.
use super::*;
use std::cmp::Ordering;

fn key<'v>(
    rows: &'v [Value],
    field: &crate::hir::DeclarationId,
    ordinal: u32,
) -> Result<&'v Value, Flow> {
    match rows.get(ordinal as usize) {
        Some(Value::Record(row)) => row
            .fields
            .get(field)
            .ok_or(Flow::Guard("missing borrowed uniqueness field")),
        _ => Err(Flow::Guard("borrowed uniqueness element is not a record")),
    }
}

fn compare_keys(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        // Exact unsigned bytes, then length: NUL and UTF-8 prefixes are data.
        (Value::String(a), Value::String(b)) => a.as_bytes().cmp(b.as_bytes()),
        _ => compare_scalar(a, b),
    }
}

impl Evaluator<'_> {
    pub(super) fn unique_record_vec_by(
        &mut self,
        element: &ResolvedType,
        identity: &str,
        values: Vec<Value>,
    ) -> Result<Value, Flow> {
        let descriptor = crate::vec_ops::field_unique::replay(self.declarations, element, identity)
            .ok_or(Flow::Guard("forged borrowed field uniqueness descriptor"))?;
        let mut operands = values.into_iter();
        let (Some(Value::Vec(vector)), None) = (operands.next(), operands.next()) else {
            return Err(Flow::Guard("invalid borrowed field uniqueness operand"));
        };
        if vector.element != *element {
            return Err(Flow::Guard("forged borrowed field uniqueness carrier"));
        }
        // Authentication of every row precedes scratch publication and reads.
        for value in &vector.values {
            self.charge()?;
            if !element_value_matches_type(self.declarations, value, element) {
                return Err(Flow::Guard("forged borrowed field uniqueness carrier"));
            }
        }
        let rows = vector.values.as_slice();
        let field = &descriptor.field;
        if rows.len() < 2 {
            return Ok(Value::Bool(true));
        }
        let length = u32::try_from(rows.len())
            .map_err(|_| Flow::Failure(normalize_vec(crate::vec_ops::ALLOCATION_FAILURE_CODE)))?;
        let mut ordinals: Vec<u32> = Vec::new();
        if ordinals.try_reserve_exact(length as usize).is_err() {
            return Err(Flow::Failure(normalize_vec(
                crate::vec_ops::ALLOCATION_FAILURE_CODE,
            )));
        }
        ordinals.extend(0..length);
        let n = ordinals.len();
        for root in (0..n / 2).rev() {
            self.sift_unique(rows, field, &mut ordinals, root, n)?;
        }
        for end in (1..n).rev() {
            ordinals.swap(0, end);
            self.sift_unique(rows, field, &mut ordinals, 0, end)?;
        }
        let mut unique = true;
        for pair in ordinals.windows(2) {
            self.charge()?;
            if compare_keys(key(rows, field, pair[0])?, key(rows, field, pair[1])?).is_eq() {
                unique = false;
                break;
            }
        }
        drop(ordinals);
        Ok(Value::Bool(unique))
    }

    fn sift_unique(
        &mut self,
        rows: &[Value],
        field: &crate::hir::DeclarationId,
        ordinals: &mut [u32],
        mut root: usize,
        n: usize,
    ) -> Result<(), Flow> {
        loop {
            let mut child = 2 * root + 1;
            if child >= n {
                return Ok(());
            }
            if child + 1 < n {
                self.charge()?;
                let order = compare_keys(
                    key(rows, field, ordinals[child])?,
                    key(rows, field, ordinals[child + 1])?,
                );
                if order.is_lt() {
                    child += 1;
                }
            }
            self.charge()?;
            let order = compare_keys(
                key(rows, field, ordinals[root])?,
                key(rows, field, ordinals[child])?,
            );
            if !order.is_lt() {
                return Ok(());
            }
            ordinals.swap(root, child);
            root = child;
        }
    }
}
