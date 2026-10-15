//! Hold each already-owned method Vec; never grow the module function Vec.
//! Inline queue capacity is a representation choice, not an admission limit.
//! More than sixteen nonempty class roots spill into ordinary, explicitly
//! unpriced compatibility backing; Drop never consults or resets a payer.
const INLINE: usize = 16;

pub(super) struct MethodRoots<T> {
    inline: [Option<Vec<T>>; INLINE],
    len: usize,
    spill: Vec<Vec<T>>,
}
impl<T> MethodRoots<T> {
    pub(super) fn new() -> Self {
        Self {
            inline: std::array::from_fn(|_| None),
            len: 0,
            spill: Vec::new(),
        }
    }
    pub(super) fn push(&mut self, methods: Vec<T>) {
        if methods.is_empty() {
            return;
        }
        if self.len < INLINE {
            self.inline[self.len] = Some(methods);
            self.len += 1;
        } else {
            self.spill.push(methods);
        }
    }
    pub(super) fn into_iter(self) -> impl Iterator<Item = T> {
        self.inline
            .into_iter()
            .take(self.len)
            .flatten()
            .chain(self.spill)
            .flatten()
    }
}

#[cfg(test)]
#[path = "drop_method_roots/tests.rs"]
mod tests;

#[cfg(test)]
pub(super) fn restore_source(mut current: String) -> String {
    let registration = "#[path = \"ast/drop_method_roots.rs\"]\nmod drop_method_roots;\n#[cfg(test)]\n#[path = \"ast/drop_method_original.rs\"]\nmod drop_method_original;\n\n";
    let hooks = [
        (registration, ""),
        (
            "        let functions = mem::take(&mut self.functions);\n        let mut method_roots = drop_method_roots::MethodRoots::new();",
            "        let mut functions = mem::take(&mut self.functions);",
        ),
        (
            "                    method_roots.push(methods);",
            "                    functions.extend(methods);",
        ),
        (
            "        for function in functions.into_iter().chain(method_roots.into_iter()) {",
            "        for function in functions {",
        ),
    ];
    for (added, original) in hooks {
        assert_eq!(current.matches(added).count(), 1);
        current = current.replacen(added, original, 1);
    }
    current
}
