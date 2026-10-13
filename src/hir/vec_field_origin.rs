//! Source read form retained through every checked projection.
//! The form does not grant root authority: nominal layout, live carrier, index
//! and loans are independently checked; source replay rejects form laundering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VecFieldReadOrigin {
    /// Authored `vec_field`, with one ordinary checked usize index.
    Explicit,
    /// Authored `row.field`, bound to its enclosing traversal's exact row index.
    Traversal,
}

impl VecFieldReadOrigin {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Traversal => "traversal",
        }
    }
}
