//! Lookup data grants no declaration/body verdict; the old constructor checks remain.
use super::*;
use crate::hir::function_directory::Directory;
mod permits;
pub(super) use permits::Permits;

#[derive(Clone)]
pub(super) enum Lookup<'a> {
    Legacy(BTreeMap<DeclarationId, &'a ResolvedFunction>),
    Borrowed {
        directory: &'a Directory<'a>,
        visible: usize,
    },
}
impl<'a> Lookup<'a> {
    pub(super) fn permits_borrowed(&self, program: &'a ResolvedProgram) -> Permits<'a> {
        match self {
            Self::Borrowed { directory, .. } => Permits::Borrowed((*directory).permits()),
            Self::Legacy(_) => Permits::Source(&program.permits),
        }
    }
    // Complete original BTree receiver for independently compiled saved validators.
    #[cfg(test)]
    pub(super) fn permits(&self, program: &'a ResolvedProgram) -> Permits<'a> {
        match self {
            Self::Borrowed { directory, .. } => Permits::Borrowed((*directory).permits()),
            Self::Legacy(_) => {
                Permits::Legacy(program.permits.iter().map(String::as_str).collect())
            }
        }
    }
    pub(super) fn directory(&self) -> Option<&'a Directory<'a>> {
        match self {
            Self::Legacy(_) => None,
            Self::Borrowed { directory, .. } => Some(directory),
        }
    }
    fn insert(&mut self, function: &'a ResolvedFunction) -> Result<bool, Diagnostic> {
        match self {
            Self::Legacy(map) => Ok(map.insert(function.id.clone(), function).is_some()),
            Self::Borrowed { directory, visible } => {
                // Only the original current source-order member may enter.
                if !directory.member_is(*visible, function)
                    || directory.first(&function.id).is_none()
                {
                    return Err(hir_error(
                        "function directory does not index the current function",
                    ));
                }
                let duplicate = directory
                    .first(&function.id)
                    .is_some_and(|entry| entry.ordinal < *visible);
                *visible += 1;
                Ok(duplicate)
            }
        }
    }
    pub(super) fn get(&self, id: &DeclarationId) -> Option<&&'a ResolvedFunction> {
        match self {
            Self::Legacy(map) => map.get(id),
            Self::Borrowed { directory, visible } => directory
                .first(id)
                .filter(|entry| entry.ordinal < *visible)
                .map(|entry| &entry.function),
        }
    }
    pub(super) fn contains_key(&self, id: &DeclarationId) -> bool {
        self.get(id).is_some()
    }
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Legacy(map) => map.len(),
            Self::Borrowed { visible, .. } => *visible,
        }
    }
    #[cfg(test)]
    pub(super) fn keys(&self) -> Keys<'_, 'a> {
        match self {
            Self::Legacy(map) => Keys::Legacy(map.keys()),
            Self::Borrowed { directory, visible } => Keys::Borrowed {
                entries: directory.entries().iter(),
                visible: *visible,
            },
        }
    }
}

#[cfg(test)]
pub(super) enum Keys<'s, 'a> {
    Legacy(std::collections::btree_map::Keys<'s, DeclarationId, &'a ResolvedFunction>),
    Borrowed {
        entries: std::slice::Iter<'s, crate::hir::function_directory::Entry<'a>>,
        visible: usize,
    },
}
#[cfg(test)]
impl<'s, 'a: 's> Iterator for Keys<'s, 'a> {
    type Item = &'s DeclarationId;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Legacy(keys) => keys.next(),
            Self::Borrowed { entries, visible } => {
                for entry in entries.by_ref() {
                    if entry.ordinal < *visible {
                        return Some(&entry.function.id);
                    }
                }
                None
            }
        }
    }
}

pub(super) fn authenticate<'a>(
    program: &'a ResolvedProgram,
    directory: Option<&'a Directory<'a>>,
) -> Result<Lookup<'a>, Diagnostic> {
    let mut functions = match directory {
        Some(directory) if directory.is_current(program) => Lookup::Borrowed {
            directory,
            visible: 0,
        },
        Some(_) => {
            return Err(hir_error(
                "function directory is not bound to the current resolved program",
            ))
        }
        None => Lookup::Legacy(BTreeMap::new()),
    };
    for function in &program.functions {
        if crate::host_io_ops::by_id(function.id.as_str()).is_some()
            || crate::command_io_ops::by_id(function.id.as_str()).is_some()
        {
            return Err(hir_error(format!(
                "resolved function `{}` aliases a compiler-owned host I/O operation",
                function.id
            )));
        }
        if functions.insert(function)? {
            return Err(hir_error(format!(
                "duplicate resolved function identity `{}`",
                function.id
            )));
        }
        match program.declarations.declaration(&function.id) {
            Some(declaration)
                if declaration.kind == DeclarationKind::Function
                    && declaration.name == function.name => {}
            Some(_) => {
                return Err(hir_error(format!(
                    "resolved function `{}` disagrees with its declaration index entry",
                    function.id
                )));
            }
            None => {
                return Err(hir_error(format!(
                    "resolved function `{}` is absent from the declaration index",
                    function.id
                )));
            }
        }
    }
    Ok(functions)
}

#[cfg(test)]
mod tests;
