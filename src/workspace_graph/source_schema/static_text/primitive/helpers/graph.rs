//! One borrowed retained-helper closure graph under the caller's preparation.
//! The sorted directory names only the caller's current functions and the
//! exact external members already selected, matching the saved linear lookup
//! universe. A lookup hit grants no authority: every first external target
//! still runs the unchanged `authenticate_target` and its provider receipt
//! before its ordinal is published. Each selected body is walked once; its
//! outgoing edges form one contiguous range in call order, which Kahn's
//! frontier then visits exactly once per edge. Growth, comparisons and moves
//! are debited on the same sticky preparation before they happen.
use super::*;
use std::cmp::Ordering;

struct Entry<'a> {
    function: &'a ResolvedFunction,
    ordinal: usize,
}
/// Local functions keep ordinals `0..local`; published helpers follow in
/// discovery order. `offsets[from]..offsets[from + 1]` indexes `targets`.
pub(super) struct Graph<'a> {
    pub(super) helpers: Vec<Member<'a>>,
    pub(super) offsets: Vec<usize>,
    pub(super) targets: Vec<usize>,
    #[cfg(test)]
    pub(super) walks: usize,
}
fn compare(
    a: &str,
    b: &str,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<Ordering, Vec<Diagnostic>> {
    charge(
        resources,
        anchor,
        a.len().min(b.len()).checked_add(1).ok_or_else(fail)?,
    )?;
    Ok(a.cmp(b))
}
/// A full buffer relocates every retained element; that move is debited
/// before the reservation, and the reservation precedes the allocation.
fn grow<T>(
    values: &mut Vec<T>,
    phase: &str,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<(), Vec<Diagnostic>> {
    if values.len() == values.capacity() {
        charge(resources, anchor, values.len())?;
    }
    resources
        .reserve_vec(anchor, values, 1, phase)
        .map_err(|_| fail())?;
    if values.len() == values.capacity() {
        return Err(fail());
    }
    Ok(())
}
fn push<T>(
    values: &mut Vec<T>,
    value: T,
    phase: &str,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<(), Vec<Diagnostic>> {
    grow(values, phase, resources, anchor)?;
    charge(resources, anchor, 1)?;
    values.push(value);
    Ok(())
}
struct Directory<'a> {
    entries: Vec<Entry<'a>>,
}
impl<'a> Directory<'a> {
    /// Seeded with exactly the caller's current functions; no provider
    /// function enters until its own authentication and receipt succeed.
    fn new(
        module: &'a WorkspaceResolvedModule,
        resources: &mut SelectedReplayPreparation,
        anchor: &ResolvedFunction,
    ) -> Result<Self, Vec<Diagnostic>> {
        let local = module.functions.len();
        resources
            .reserve_array::<Entry<'_>>(anchor, local, "imported scalar closure directory")
            .map_err(|_| fail())?;
        charge(resources, anchor, local)?;
        let mut entries = Vec::with_capacity(local);
        if entries.capacity() != local {
            return Err(fail());
        }
        for (ordinal, function) in module.functions.iter().enumerate() {
            entries.push(Entry { function, ordinal });
        }
        for root in (0..local / 2).rev() {
            sift(&mut entries, root, local, resources, anchor)?;
        }
        for end in (1..local).rev() {
            charge(resources, anchor, 1)?;
            entries.swap(0, end);
            sift(&mut entries, 0, end, resources, anchor)?;
        }
        Ok(Self { entries })
    }
    /// The insertion point plus the unique ordinal of an equal current ID.
    /// Equal local IDs are retained, so a duplicate identity refuses like the
    /// saved linear lookup over caller locals and selected helpers.
    fn lookup(
        &self,
        id: &hir::DeclarationId,
        resources: &mut SelectedReplayPreparation,
        anchor: &ResolvedFunction,
    ) -> Result<(usize, Option<usize>), Vec<Diagnostic>> {
        let (mut low, mut high) = (0, self.entries.len());
        while low < high {
            charge(resources, anchor, 1)?;
            let middle = low + (high - low) / 2;
            if compare(
                self.entries[middle].function.id.as_str(),
                id.as_str(),
                resources,
                anchor,
            )? == Ordering::Less
            {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        let mut end = low;
        let mut found = None;
        while end < self.entries.len()
            && compare(
                self.entries[end].function.id.as_str(),
                id.as_str(),
                resources,
                anchor,
            )? == Ordering::Equal
        {
            if found.replace(self.entries[end].ordinal).is_some() {
                return Err(fail());
            }
            end += 1;
        }
        Ok((low, found))
    }
    /// Publication binds the authenticated defining pointer at the unchanged
    /// insertion point of a missed lookup; shifted entries are debited first.
    fn publish(
        &mut self,
        at: usize,
        function: &'a ResolvedFunction,
        ordinal: usize,
        resources: &mut SelectedReplayPreparation,
        anchor: &ResolvedFunction,
    ) -> Result<(), Vec<Diagnostic>> {
        let shifted = self.entries.len().checked_sub(at).ok_or_else(fail)?;
        grow(
            &mut self.entries,
            "imported scalar closure directory",
            resources,
            anchor,
        )?;
        charge(resources, anchor, shifted.checked_add(1).ok_or_else(fail)?)?;
        self.entries.insert(at, Entry { function, ordinal });
        Ok(())
    }
}
fn sift(
    entries: &mut [Entry<'_>],
    mut root: usize,
    end: usize,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<(), Vec<Diagnostic>> {
    while let Some(child) = root
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .filter(|n| *n < end)
    {
        charge(resources, anchor, 1)?;
        let mut greatest = child;
        if child + 1 < end
            && compare(
                entries[child].function.id.as_str(),
                entries[child + 1].function.id.as_str(),
                resources,
                anchor,
            )? == Ordering::Less
        {
            greatest += 1;
        }
        if compare(
            entries[root].function.id.as_str(),
            entries[greatest].function.id.as_str(),
            resources,
            anchor,
        )? != Ordering::Less
        {
            break;
        }
        charge(resources, anchor, 1)?;
        entries.swap(root, greatest);
        root = greatest;
    }
    Ok(())
}

pub(super) fn discover<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<Graph<'a>, Vec<Diagnostic>> {
    // The saved member, receipt and capacity bounds stay exactly as before.
    let maximum = modules
        .iter()
        .try_fold(0usize, |n, m| n.checked_add(m.functions.len()))
        .ok_or_else(fail)?;
    charge(resources, anchor, modules.len())?;
    resources
        .reserve_array::<Member<'_>>(anchor, maximum, "imported scalar closure members")
        .map_err(|_| fail())?;
    resources
        .reserve_array::<u8>(
            anchor,
            modules.len(),
            "imported scalar defining source debits",
        )
        .map_err(|_| fail())?;
    charge(resources, anchor, modules.len())?;
    let mut charged = vec![0u8; modules.len()];
    let mut helpers: Vec<Member<'a>> = Vec::with_capacity(maximum);
    if helpers.capacity() != maximum {
        return Err(fail());
    }
    let mut directory = Directory::new(module, resources, anchor)?;
    let mut offsets: Vec<usize> = Vec::new();
    push(
        &mut offsets,
        0,
        "imported scalar closure edge ranges",
        resources,
        anchor,
    )?;
    let mut targets: Vec<usize> = Vec::new();
    #[cfg(test)]
    let mut walks = 0usize;
    let mut processed = 0;
    loop {
        let total = module
            .functions
            .len()
            .checked_add(helpers.len())
            .ok_or_else(fail)?;
        if processed >= total {
            break;
        }
        let function = if processed < module.functions.len() {
            &module.functions[processed]
        } else {
            helpers[processed - module.functions.len()].function
        };
        let provider = processed >= module.functions.len();
        calls(function, resources, anchor, |id, resources| {
            if builtin(id) {
                return if provider && !hir::borrowed_scalar::builtin(id.as_str()) {
                    Err(fail())
                } else {
                    Ok(())
                };
            }
            let (at, found) = directory.lookup(id, resources, anchor)?;
            let to = match found {
                Some(to) => to,
                None => {
                    let member = authenticate_target(id, modules, facts, resources, anchor)?;
                    if std::ptr::eq(member.module, module) {
                        return Err(fail());
                    }
                    let ordinal = modules
                        .iter()
                        .position(|m| std::ptr::eq(m, member.module))
                        .ok_or_else(fail)?;
                    charge(resources, anchor, modules.len())?;
                    if charged[ordinal] == 0 {
                        charge(
                            resources,
                            anchor,
                            member
                                .module
                                .scalar_helpers
                                .as_deref()
                                .ok_or_else(fail)?
                                .capture_work,
                        )?;
                        charged[ordinal] = 1;
                    }
                    if helpers.len() == maximum {
                        return Err(fail());
                    }
                    let to = module
                        .functions
                        .len()
                        .checked_add(helpers.len())
                        .ok_or_else(fail)?;
                    directory.publish(at, member.function, to, resources, anchor)?;
                    charge(resources, anchor, 1)?;
                    helpers.push(member);
                    to
                }
            };
            push(
                &mut targets,
                to,
                "imported scalar closure edges",
                resources,
                anchor,
            )
        })?;
        #[cfg(test)]
        {
            walks += 1;
        }
        push(
            &mut offsets,
            targets.len(),
            "imported scalar closure edge ranges",
            resources,
            anchor,
        )?;
        processed = processed.checked_add(1).ok_or_else(fail)?;
    }
    Ok(Graph {
        helpers,
        offsets,
        targets,
        #[cfg(test)]
        walks,
    })
}

/// Saved frontier order: zero-indegree vertices in ordinal order, LIFO pops,
/// and each popped vertex's own outgoing range in call order. Returns the
/// number of edge visits; a cycle leaves vertices unvisited and refuses.
pub(super) fn order(
    graph: &Graph<'_>,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
    mut visit: impl FnMut(usize),
) -> Result<usize, Vec<Diagnostic>> {
    let count = graph.offsets.len().checked_sub(1).ok_or_else(fail)?;
    resources
        .reserve_array::<usize>(anchor, count, "imported scalar closure indegrees")
        .map_err(|_| fail())?;
    resources
        .reserve_array::<usize>(anchor, count, "imported scalar closure ready frontier")
        .map_err(|_| fail())?;
    charge(resources, anchor, count)?;
    let mut indegree = vec![0usize; count];
    let mut pending = Vec::with_capacity(count);
    if pending.capacity() != count {
        return Err(fail());
    }
    for to in &graph.targets {
        charge(resources, anchor, 1)?;
        let n = indegree.get_mut(*to).ok_or_else(fail)?;
        *n = n.checked_add(1).ok_or_else(fail)?;
    }
    for (i, n) in indegree.iter().enumerate() {
        charge(resources, anchor, 1)?;
        if *n == 0 {
            charge(resources, anchor, 1)?;
            pending.push(i);
        }
    }
    let mut visited = 0usize;
    let mut edges = 0usize;
    while let Some(from) = pending.pop() {
        charge(resources, anchor, 1)?;
        visited = visited.checked_add(1).ok_or_else(fail)?;
        visit(from);
        let start = *graph.offsets.get(from).ok_or_else(fail)?;
        let end = *graph
            .offsets
            .get(from.checked_add(1).ok_or_else(fail)?)
            .ok_or_else(fail)?;
        for to in graph.targets.get(start..end).ok_or_else(fail)? {
            charge(resources, anchor, 1)?;
            edges = edges.checked_add(1).ok_or_else(fail)?;
            let n = indegree.get_mut(*to).ok_or_else(fail)?;
            *n = n.checked_sub(1).ok_or_else(fail)?;
            if *n == 0 {
                if pending.len() == count {
                    return Err(fail());
                }
                charge(resources, anchor, 1)?;
                pending.push(*to);
            }
        }
    }
    if visited != count {
        return Err(fail());
    }
    Ok(edges)
}
