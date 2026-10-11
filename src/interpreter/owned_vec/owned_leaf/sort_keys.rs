//! Stable selected-field comparison; carriers are exchanged without cloning.
use super::*;
impl Evaluator<'_> {
    pub(in crate::interpreter) fn sort_record_vec_by(
        &mut self,
        element: &ResolvedType,
        identity: &str,
        values: Vec<Value>,
    ) -> Result<Value, Flow> {
        let descriptor = crate::vec_ops::sort_keys::replay(self.declarations, element, identity)
            .ok_or(Flow::Guard("forged explicit Vec sort descriptor"))?;
        let mut operands = values.into_iter();
        let (Some(Value::Vec(vector)), None) = (operands.next(), operands.next()) else {
            return Err(Flow::Guard("invalid explicit Vec sort operands"));
        };
        if vector.element != *element
            || vector
                .values
                .iter()
                .any(|value| !element_value_matches_type(self.declarations, value, element))
        {
            return Err(Flow::Guard("forged explicit Vec sort carrier"));
        }
        let mut vector = Arc::try_unwrap(vector)
            .map_err(|_| Flow::Guard("aliased explicit Vec sort carrier"))?;
        for value in &vector.values {
            self.fused.preflight_value(value)?;
        }
        for index in 1..vector.values.len() {
            let mut at = index;
            while at > 0 {
                let (Value::Record(left), Value::Record(right)) =
                    (&vector.values[at], &vector.values[at - 1])
                else {
                    return Err(Flow::Guard("explicit sort element is not a record"));
                };
                let mut order = std::cmp::Ordering::Equal;
                for key in &descriptor.keys {
                    let a = left
                        .fields
                        .get(&key.field)
                        .ok_or(Flow::Guard("missing explicit sort field"))?;
                    let b = right
                        .fields
                        .get(&key.field)
                        .ok_or(Flow::Guard("missing explicit sort field"))?;
                    order = if key.descending {
                        leaf_compare(b, a, &mut self.fused)?
                    } else {
                        leaf_compare(a, b, &mut self.fused)?
                    };
                    if !order.is_eq() {
                        break;
                    }
                }
                if !order.is_lt() {
                    break;
                }
                vector.values.swap(at, at - 1);
                at -= 1;
            }
        }
        vector.generation = vector
            .generation
            .checked_add(1)
            .ok_or(Flow::Guard("Vec generation overflowed"))?;
        Ok(Value::Vec(Arc::new(vector)))
    }
}
