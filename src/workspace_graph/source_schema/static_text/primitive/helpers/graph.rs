//! One borrowed retained-helper closure graph under the caller's preparation.
//! The balanced directory names only the caller's current functions and the
//! exact external members already selected, matching the saved linear lookup
//! universe. A lookup hit grants no authority: every first external target
//! still runs the unchanged `authenticate_target` and its provider receipt
//! before its ordinal is published. Each selected body is walked once; its
//! outgoing edges form one contiguous range in call order, which Kahn's
//! frontier then visits exactly once per edge. Growth, comparisons, retained
//! nodes and rebalancing are debited on the same sticky preparation before
//! they happen, and the directory is retained for the closure's later lookup.
use super::*;
use std::cmp::Ordering;

/// Absent arena link.
const NIL: usize = usize::MAX;
/// An AVL tree of `n` nodes is at most `1.4405 * log2(n + 2)` levels high,
/// below 93 for any addressable node count; a deeper path refuses.
pub(super) const PATH: usize = 96;

/// Arena slot `i` is ordinal `i`: local functions in source order, then
/// published helpers in discovery order. Only the key's borrowed defining
/// pointer is held; provenance stays with the caller module and `Member`.
struct Node<'a> {
    function: &'a ResolvedFunction,
    left: usize,
    right: usize,
    height: u8,
    /// A later equal current ID exists. The first equal local stays linked,
    /// so the retained lookup answers the saved first-match ordinal while a
    /// discovery call to the shared ID refuses like the saved lookup.
    shared: bool,
}
/// Local functions keep ordinals `0..local`; published helpers follow in
/// discovery order. `offsets[from]..offsets[from + 1]` indexes `targets`.
pub(super) struct Graph<'a> {
    pub(super) helpers: Vec<Member<'a>>,
    pub(super) offsets: Vec<usize>,
    pub(super) targets: Vec<usize>,
    pub(super) directory: Directory<'a>,
    #[cfg(test)]
    pub(super) walks: usize,
}
type Debit<'d> = dyn FnMut(usize) -> Result<(), Vec<Diagnostic>> + 'd;
fn compare(a: &str, b: &str, debit: &mut Debit<'_>) -> Result<Ordering, Vec<Diagnostic>> {
    debit(a.len().min(b.len()).checked_add(1).ok_or_else(fail)?)?;
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
/// The one insertion path scratch of a discovery. Insertion is never
/// reentrant, so a single path is live at a time; its stack backing is
/// reserved and its full initialization debited once, before it happens.
/// Each insertion then writes only the slots of the levels it descends,
/// debited per level, and reads back no slot beyond its own depth.
pub(super) struct Path {
    at: [usize; PATH],
    right: [bool; PATH],
}
impl Path {
    pub(super) fn new(
        resources: &mut SelectedReplayPreparation,
        anchor: &ResolvedFunction,
    ) -> Result<Self, Vec<Diagnostic>> {
        resources
            .reserve_array::<usize>(anchor, PATH, "imported scalar closure directory path")
            .map_err(|_| fail())?;
        resources
            .reserve_array::<bool>(anchor, PATH, "imported scalar closure directory path")
            .map_err(|_| fail())?;
        charge(resources, anchor, 2 * PATH)?;
        Ok(Self {
            at: [NIL; PATH],
            right: [false; PATH],
        })
    }
}
/// Deterministic byte-order AVL directory over borrowed IDs. Publication
/// appends one arena node and relinks at most one (double) rotation, so no
/// retained entry ever shifts; no hashing or pointer order is consulted.
pub(super) struct Directory<'a> {
    nodes: Vec<Node<'a>>,
    root: usize,
}
impl<'a> Directory<'a> {
    /// Seeded with exactly the caller's current functions; no provider
    /// function enters until its own authentication and receipt succeed.
    pub(super) fn new(
        module: &'a WorkspaceResolvedModule,
        path: &mut Path,
        resources: &mut SelectedReplayPreparation,
        anchor: &ResolvedFunction,
    ) -> Result<Self, Vec<Diagnostic>> {
        let local = module.functions.len();
        resources
            .reserve_array::<Node<'_>>(anchor, local, "imported scalar closure directory")
            .map_err(|_| fail())?;
        charge(resources, anchor, local)?;
        let nodes = Vec::with_capacity(local);
        if nodes.capacity() != local {
            return Err(fail());
        }
        let mut directory = Self { nodes, root: NIL };
        for function in &module.functions {
            directory.link(function, false, path, &mut |n| charge(resources, anchor, n))?;
        }
        Ok(directory)
    }
    /// Appends `function` as the next ordinal. A `fresh` key must be absent;
    /// an equal local key marks the linked first occurrence shared and stays
    /// unlinked. The search debits each level, comparison and the two path
    /// slots it writes; the node and every retrace level, including a
    /// possible double rotation, are debited together before the arena or
    /// any link changes.
    fn link(
        &mut self,
        function: &'a ResolvedFunction,
        fresh: bool,
        path: &mut Path,
        debit: &mut Debit<'_>,
    ) -> Result<usize, Vec<Diagnostic>> {
        if self.nodes.len() == self.nodes.capacity() {
            return Err(fail());
        }
        let ordinal = self.nodes.len();
        let mut depth = 0;
        let mut at = self.root;
        let mut equal = None;
        while at != NIL {
            debit(1)?;
            if depth == PATH {
                return Err(fail());
            }
            let node = &self.nodes[at];
            let order = compare(node.function.id.as_str(), function.id.as_str(), debit)?;
            if order == Ordering::Equal {
                equal = Some(at);
                break;
            }
            debit(2)?;
            path.at[depth] = at;
            path.right[depth] = order == Ordering::Less;
            at = if order == Ordering::Less {
                node.right
            } else {
                node.left
            };
            depth += 1;
        }
        if let Some(first) = equal {
            if fresh {
                return Err(fail());
            }
            debit(1)?;
            self.nodes[first].shared = true;
            self.nodes.push(Node {
                function,
                left: NIL,
                right: NIL,
                height: 1,
                shared: true,
            });
            return Ok(ordinal);
        }
        debit(depth.checked_add(3).ok_or_else(fail)?)?;
        self.nodes.push(Node {
            function,
            left: NIL,
            right: NIL,
            height: 1,
            shared: false,
        });
        let Some(last) = depth.checked_sub(1) else {
            self.root = ordinal;
            return Ok(ordinal);
        };
        self.attach(path.at[last], path.right[last], ordinal);
        for level in (0..depth).rev() {
            let top = self.rebalance(path.at[level]);
            match level.checked_sub(1) {
                Some(parent) => self.attach(path.at[parent], path.right[parent], top),
                None => self.root = top,
            }
        }
        Ok(ordinal)
    }
    fn attach(&mut self, parent: usize, right: bool, child: usize) {
        if right {
            self.nodes[parent].right = child;
        } else {
            self.nodes[parent].left = child;
        }
    }
    fn height(&self, at: usize) -> u8 {
        if at == NIL {
            0
        } else {
            self.nodes[at].height
        }
    }
    fn update(&mut self, at: usize) {
        let node = &self.nodes[at];
        let height = self.height(node.left).max(self.height(node.right)) + 1;
        self.nodes[at].height = height;
    }
    fn rotate_right(&mut self, at: usize) -> usize {
        let top = self.nodes[at].left;
        self.nodes[at].left = self.nodes[top].right;
        self.nodes[top].right = at;
        self.update(at);
        self.update(top);
        top
    }
    fn rotate_left(&mut self, at: usize) -> usize {
        let top = self.nodes[at].right;
        self.nodes[at].right = self.nodes[top].left;
        self.nodes[top].left = at;
        self.update(at);
        self.update(top);
        top
    }
    /// Restores the AVL bound at `at` and returns the subtree's new root.
    fn rebalance(&mut self, at: usize) -> usize {
        let (left, right) = (self.nodes[at].left, self.nodes[at].right);
        let (l, r) = (self.height(left), self.height(right));
        if l > r + 1 {
            if self.height(self.nodes[left].left) < self.height(self.nodes[left].right) {
                self.nodes[at].left = self.rotate_left(left);
            }
            return self.rotate_right(at);
        }
        if r > l + 1 {
            if self.height(self.nodes[right].right) < self.height(self.nodes[right].left) {
                self.nodes[at].right = self.rotate_right(right);
            }
            return self.rotate_left(at);
        }
        self.update(at);
        at
    }
    /// The unique current ordinal of `id`, if any. A shared current ID
    /// refuses like the saved linear lookup over locals and selected helpers.
    fn find(&self, id: &str, debit: &mut Debit<'_>) -> Result<Option<usize>, Vec<Diagnostic>> {
        let mut at = self.root;
        while at != NIL {
            debit(1)?;
            let node = &self.nodes[at];
            match compare(node.function.id.as_str(), id, debit)? {
                Ordering::Less => at = node.right,
                Ordering::Greater => at = node.left,
                Ordering::Equal if node.shared => return Err(fail()),
                Ordering::Equal => return Ok(Some(at)),
            }
        }
        Ok(None)
    }
    /// The retained replay lookup: the saved first match in ordinal order
    /// over exactly the closure's functions, in one comparison per level of
    /// the AVL-bounded height. It selects; it never authenticates.
    pub(super) fn resolve(&self, id: &str) -> Option<&'a ResolvedFunction> {
        let mut at = self.root;
        while at != NIL {
            let node = &self.nodes[at];
            match node.function.id.as_str().cmp(id) {
                Ordering::Less => at = node.right,
                Ordering::Greater => at = node.left,
                Ordering::Equal => return Some(node.function),
            }
        }
        None
    }
    /// Publication binds the authenticated defining pointer as the next
    /// ordinal; arena growth is reserved and its relocation debited first.
    fn publish(
        &mut self,
        function: &'a ResolvedFunction,
        ordinal: usize,
        path: &mut Path,
        resources: &mut SelectedReplayPreparation,
        anchor: &ResolvedFunction,
    ) -> Result<(), Vec<Diagnostic>> {
        grow(
            &mut self.nodes,
            "imported scalar closure directory",
            resources,
            anchor,
        )?;
        if self.link(function, true, path, &mut |n| charge(resources, anchor, n))? != ordinal {
            return Err(fail());
        }
        Ok(())
    }
    /// Test audit: arena pointers in ordinal order, linked ordinals in key
    /// order and the tree height, checking every stored height and balance.
    #[cfg(test)]
    pub(super) fn audit(&self) -> (Vec<*const ResolvedFunction>, Vec<usize>, usize) {
        let arena = self
            .nodes
            .iter()
            .map(|n| n.function as *const ResolvedFunction)
            .collect();
        let mut sorted = Vec::new();
        let mut stack = Vec::new();
        let mut at = self.root;
        while at != NIL || !stack.is_empty() {
            while at != NIL {
                stack.push(at);
                at = self.nodes[at].left;
            }
            let top = stack.pop().unwrap();
            let node = &self.nodes[top];
            let (l, r) = (self.height(node.left), self.height(node.right));
            assert_eq!(node.height, l.max(r) + 1, "stored height");
            assert!(l.abs_diff(r) <= 1, "AVL balance");
            sorted.push(top);
            at = node.right;
        }
        for pair in sorted.windows(2) {
            assert!(
                self.nodes[pair[0]].function.id.as_str() < self.nodes[pair[1]].function.id.as_str(),
                "strict key order"
            );
        }
        (arena, sorted, usize::from(self.height(self.root)))
    }
}

pub(super) fn discover<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<Graph<'a>, Vec<Diagnostic>> {
    // The saved whole-input member bound and receipt slots stay as before;
    // members, like the directory, grow only by actual reachable demand.
    let maximum = modules
        .iter()
        .try_fold(0usize, |n, m| n.checked_add(m.functions.len()))
        .ok_or_else(fail)?;
    charge(resources, anchor, modules.len())?;
    resources
        .reserve_array::<u8>(
            anchor,
            modules.len(),
            "imported scalar defining source debits",
        )
        .map_err(|_| fail())?;
    charge(resources, anchor, modules.len())?;
    let mut charged = vec![0u8; modules.len()];
    let mut helpers: Vec<Member<'a>> = Vec::new();
    let mut path = Path::new(resources, anchor)?;
    let mut directory = Directory::new(module, &mut path, resources, anchor)?;
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
            let found = directory.find(id.as_str(), &mut |n| charge(resources, anchor, n))?;
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
                    directory.publish(member.function, to, &mut path, resources, anchor)?;
                    push(
                        &mut helpers,
                        member,
                        "imported scalar closure members",
                        resources,
                        anchor,
                    )?;
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
        directory,
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
