//! Funded sparse generic-template call graph for an active validation owner.
//! The root wrapper keeps the complete old BTree route when no owner is active.
use super::*;
use crate::hir::inspection::calls::try_visit_resolved_calls;
use std::alloc::Layout;

const REFUSED: &str = "generic function template call graph exceeds the active validation budget";
const NO_OWNER: &str =
    "paid generic function template call graph requires an active validation owner";
const CORRUPT: &str = "generic function template call graph lost an indexed template";

fn refused() -> Diagnostic {
    crate::hir::hir_error_static(REFUSED)
}
fn corrupt() -> Diagnostic {
    crate::hir::hir_error_static(CORRUPT)
}
fn reserve(bytes: usize) -> Result<(), Diagnostic> {
    if crate::bounded_output::reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(refused())
    }
}
fn physical() -> Diagnostic {
    crate::bounded_output::refuse_active_required(usize::MAX);
    refused()
}
fn layout_bytes<T>(count: usize) -> Result<usize, Diagnostic> {
    Layout::array::<T>(count)
        .map(|layout| layout.size())
        .map_err(|_| physical())
}
/// The requested Global Vec Layout was reserved before acquisition. Pin its
/// capacity before publishing elements. An unexpected allocator/toolchain path
/// refuses publication; it is not retrospectively paid or a funded fallback.
fn settle<T>(vec: &Vec<T>, wanted: usize) -> Result<(), Diagnostic> {
    if vec.capacity() == wanted {
        Ok(())
    } else {
        Err(physical())
    }
}
fn with_capacity<T>(count: usize) -> Result<Vec<T>, Diagnostic> {
    reserve(0)?;
    if count == 0 {
        return Ok(Vec::new());
    }
    reserve(layout_bytes::<T>(count)?)?;
    let mut vec = Vec::new();
    vec.try_reserve_exact(count).map_err(|_| physical())?;
    settle(&vec, count)?;
    Ok(vec)
}
mod ordered;
#[cfg(test)]
pub(in crate::hir::validation) mod original;
use ordered::Outgoing;
fn enqueue(queue: &mut Vec<usize>, ordinal: usize) -> Result<(), Diagnostic> {
    reserve(0)?;
    if queue.len() == queue.capacity() {
        return Err(corrupt());
    }
    queue.push(ordinal);
    Ok(())
}

struct Graph<'a> {
    program: &'a ResolvedProgram,
    ids: Vec<&'a DeclarationId>,
    edges: Vec<Outgoing>,
    indegree: Vec<usize>,
    queue: Vec<usize>,
}
impl<'a> Graph<'a> {
    fn new(program: &'a ResolvedProgram) -> Result<Self, Diagnostic> {
        let count = program.function_templates.len();
        // Modeled inline receiving owner, separate from requested heap Layout bytes.
        // Authenticate and debit it before constructing or publishing that owner.
        reserve(std::mem::size_of::<Self>())?;
        let mut ids = with_capacity::<&DeclarationId>(count)?;
        for template in &program.function_templates {
            ids.push(&template.id);
        }
        ids.sort_unstable();
        ids.dedup();
        let unique = ids.len();
        let mut edges = with_capacity::<Outgoing>(unique)?;
        for _ in 0..unique {
            edges.push(Outgoing::new());
        }
        let mut indegree = with_capacity::<usize>(unique)?;
        for _ in 0..unique {
            indegree.push(0);
        }
        let queue = with_capacity::<usize>(unique)?;
        Ok(Self {
            program,
            ids,
            edges,
            indegree,
            queue,
        })
    }
    fn ordinal(&self, id: &DeclarationId) -> Option<usize> {
        self.ids.binary_search_by(|probe| (*probe).cmp(id)).ok()
    }
    /// Ordered ordinal sets preserve complete ID ordering without insertion shifts.
    fn add_edge(&mut self, from: usize, to: usize) -> Result<(), Diagnostic> {
        reserve(0)?;
        let inserted = self.edges.get_mut(from).ok_or_else(corrupt)?.insert(to)?;
        if inserted {
            let count = self.indegree.get_mut(to).ok_or_else(corrupt)?;
            *count = count.checked_add(1).ok_or_else(corrupt)?;
        }
        Ok(())
    }
    /// Every original template is scanned, duplicate IDs included.
    fn scan(&mut self) -> Result<(), Diagnostic> {
        let program = self.program;
        for template in &program.function_templates {
            reserve(0)?;
            let from = self.ordinal(&template.id).ok_or_else(corrupt)?;
            for expression in template
                .requires
                .iter()
                .chain(std::iter::once(&template.body))
                .chain(&template.ensures)
            {
                try_visit_resolved_calls(
                    expression,
                    &mut |callee, _, _| -> Result<(), Diagnostic> {
                        reserve(0)?;
                        if let Some(to) = self.ordinal(callee) {
                            self.add_edge(from, to)?;
                        }
                        Ok(())
                    },
                )?;
            }
        }
        Ok(())
    }
    /// All backing stays owned until the complete receiving graph is dropped.
    fn drain(&mut self) -> Result<usize, Diagnostic> {
        for (ordinal, count) in self.indegree.iter().enumerate() {
            reserve(0)?;
            if *count == 0 {
                enqueue(&mut self.queue, ordinal)?;
            }
        }
        let mut head = 0usize;
        while head < self.queue.len() {
            reserve(0)?;
            let node = self.queue[head];
            head += 1;
            let targets = self.edges.get(node).ok_or_else(corrupt)?;
            let indegree = &mut self.indegree;
            let queue = &mut self.queue;
            targets.visit(&mut |callee| {
                let count = indegree.get_mut(callee).ok_or_else(corrupt)?;
                *count = count.checked_sub(1).ok_or_else(corrupt)?;
                if *count == 0 {
                    enqueue(queue, callee)?;
                }
                Ok(())
            })?;
        }
        Ok(head)
    }
}

/// Private paid route; refuses to run without an active owner.
pub(super) fn validate_call_graph_paid(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    if crate::bounded_output::active_remaining().is_none() {
        return Err(crate::hir::hir_error_static(NO_OWNER));
    }
    reserve(0)?;
    if program.function_templates.is_empty() {
        return Ok(());
    }
    let mut graph = Graph::new(program)?;
    graph.scan()?;
    let unique = graph.ids.len();
    let visited = graph.drain()?;
    (visited == unique).then_some(()).ok_or_else(|| {
        crate::hir::hir_error_static("generic function template call graph contains a direct cycle")
    })
}

#[cfg(test)]
mod tests;
