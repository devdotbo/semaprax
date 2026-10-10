//! Authenticated static field/direction arrays, never a runtime selector String.
use super::*;
impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_sort_keys(&mut self, expr: &ResolvedExpr) -> Result<String, Diagnostic> {
        let keys =
            crate::vec_ops::sort_keys::expression_positions(&self.program.declarations, expr)
                .ok_or_else(|| {
                    backend_error("explicit sort descriptor failed independent replay")
                })?;
        let name = format!("spx_sort_keys_{}", self.next_local);
        self.next_local += 1;
        let indexes = keys
            .iter()
            .map(|(i, _)| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let directions = keys
            .iter()
            .map(|(_, d)| u8::from(*d).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        self.line(&format!(
            "const uint32_t {name}[{}] = {{ {indexes} }};",
            keys.len()
        ));
        self.line(&format!(
            "const uint32_t {name}_descending[{}] = {{ {directions} }};",
            keys.len()
        ));
        Ok(format!(
            "UINT32_C({}), {name}, {name}_descending, ",
            keys.len()
        ))
    }
}
