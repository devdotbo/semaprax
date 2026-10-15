//! Exact expected path bytes borrowed from the current authenticated parent.
use super::*;
use std::fmt::{self, Write};

#[derive(Clone, Copy)]
pub(super) struct Label<'a> {
    parent: &'a str,
    suffix: &'static str,
    indices: [usize; 2],
}
impl<'a> From<&'a str> for Label<'a> {
    fn from(parent: &'a str) -> Self {
        Self {
            parent,
            suffix: "",
            indices: [0; 2],
        }
    }
}
impl<'a> Label<'a> {
    pub(super) fn child(
        parent: &'a str,
        suffix: &'static str,
        indices: [usize; 2],
    ) -> Result<Self, Diagnostic> {
        let label = Self {
            parent,
            suffix,
            indices,
        };
        label.checked_len().ok_or_else(|| {
            scope_checkpoint::charge::failure(
                usize::MAX,
                "scope continuation label layout overflowed",
            )
        })?;
        Ok(label)
    }
    fn checked_len(self) -> Option<usize> {
        struct Count(usize);
        impl Write for Count {
            fn write_str(&mut self, value: &str) -> fmt::Result {
                self.0 = self.0.checked_add(value.len()).ok_or(fmt::Error)?;
                Ok(())
            }
        }
        let mut count = Count(0);
        write!(count, "{self}").ok()?;
        Some(count.0)
    }
    pub(super) fn len(self) -> usize {
        self.checked_len().expect("checked expected path length")
    }
    pub(super) fn argument(
        expression: &ResolvedExpr,
        parent: &'a str,
        index: usize,
    ) -> Result<Self, Diagnostic> {
        let fused_string_slot = matches!(&expression.kind, ResolvedExprKind::Call {callee,..} if callee.as_str() == crate::vec_ops::string_slot::BYTES_ID);
        let fused_field = matches!(
            &expression.kind,
            ResolvedExprKind::VecFieldRead { bytes: true, .. }
        );
        if fused_string_slot || fused_field {
            Self::child(parent, ".arg.0.arg.{}", [index, 0])
        } else {
            Self::child(
                parent,
                ".arg.{}",
                [
                    index
                        + usize::from(matches!(
                            &expression.kind,
                            ResolvedExprKind::LiteralFormat { .. }
                        )),
                    0,
                ],
            )
        }
    }
}
impl fmt::Display for Label<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.parent)?;
        let mut pieces = self.suffix.split("{}");
        f.write_str(pieces.next().expect("static suffix"))?;
        for (index, piece) in pieces.enumerate() {
            write!(f, "{}", self.indices[index])?;
            f.write_str(piece)?;
        }
        Ok(())
    }
}
impl ValueId {
    pub(in crate::hir::validation) fn matches_local_label(
        &self,
        function: &FunctionExecutionId,
        label: Label<'_>,
    ) -> bool {
        self.matches_local_display(function, label.len(), label)
    }
}
#[cfg(test)]
mod tests;
