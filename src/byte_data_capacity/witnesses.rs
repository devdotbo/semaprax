//! Optional evidence reconstructed only after a transcript refusal is selected.
//! Inputs are the same validated immutable projection. Limits here omit help;
//! they never admit source, change the primary refusal, or widen a profile.
use super::*;
use std::fmt::Write;
const STEPS: usize = 16;
const OPERATIONS: usize = MAX_FLOW_NODES;
const OUTPUT_BYTES: usize = 4096;
const REPAIR: &str = "Build bounded bytes first, then publish once outside loops and cycles; arbitrary repeated writes are not admitted.";

fn pay(bytes: usize) -> Option<()> {
    crate::bounded_output::reserve_active_preserving(bytes, 0).then_some(())
}
struct Work(usize);
impl Work {
    fn step(&mut self) -> Option<()> {
        self.0 = self.0.checked_sub(1)?;
        pay(1)
    }
}
#[derive(Clone, Copy)]
enum Step<'a> {
    Call {
        function: &'a str,
        site: &'a str,
        callee: &'a str,
    },
    Write {
        function: &'a str,
        site: &'a str,
        operation: &'static str,
    },
}
#[derive(Clone, Copy)]
struct Trace<'a> {
    steps: [Option<Step<'a>>; STEPS],
    len: usize,
}
impl<'a> Trace<'a> {
    fn empty() -> Self {
        Self {
            steps: [None; STEPS],
            len: 0,
        }
    }
    fn push(mut self, step: Step<'a>) -> Option<Self> {
        *self.steps.get_mut(self.len)? = Some(step);
        self.len += 1;
        Some(self)
    }
    fn render(&self, out: &mut (impl Write + ?Sized)) -> fmt::Result {
        for step in self.steps[..self.len].iter().flatten() {
            match step {
                Step::Call {
                    function,
                    site,
                    callee,
                } => write!(out, "{function:?} --call {site:?}--> {callee:?}; ")?,
                Step::Write {
                    function,
                    site,
                    operation,
                } => write!(out, "{function:?} --{operation} {site:?}")?,
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub(super) enum Query {
    Stdout,
    Stderr,
    AnyOutput,
    LoopBody,
}
#[derive(Clone, Copy)]
struct Value<'a> {
    count: usize,
    hits: [Option<Trace<'a>>; 2],
}
impl Value<'_> {
    fn empty() -> Self {
        Self {
            count: 0,
            hits: [None; 2],
        }
    }
    fn sequence(mut self, other: Self) -> Self {
        for hit in other.hits.into_iter().flatten() {
            if self.count < 2 {
                self.hits[self.count] = Some(hit);
                self.count += 1;
            }
        }
        self
    }
}
// The inline header and all inline element slots are paid before initialization.
// Spill growth pays the entire replacing backing while old capacity remains held.
struct Stack<T> {
    inline: [Option<T>; 32],
    len: usize,
    spill: Vec<T>,
}
impl<T> Stack<T> {
    fn new() -> Option<Self> {
        pay(std::mem::size_of::<Self>())?;
        Some(Self {
            inline: std::array::from_fn(|_| None),
            len: 0,
            spill: Vec::new(),
        })
    }
    fn push(&mut self, value: T) -> Option<()> {
        pay(1)?;
        if self.len < 32 {
            self.inline[self.len] = Some(value);
            self.len += 1;
            return Some(());
        }
        if self.spill.len() == self.spill.capacity() {
            let target = self.spill.capacity().checked_mul(2)?.max(4);
            pay(target.checked_mul(std::mem::size_of::<T>())?)?;
            self.spill
                .try_reserve_exact(target.checked_sub(self.spill.len())?)
                .ok()?;
            if self.spill.capacity() != target {
                return None;
            }
        }
        self.spill.push(value);
        self.len += 1;
        Some(())
    }
    fn pop(&mut self) -> Option<T> {
        pay(1)?;
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        if self.len >= 32 {
            self.spill.pop()
        } else {
            self.inline[self.len].take()
        }
    }
}
// Uniform paid frame avoids a large tagged variant or a separate boxed trace.
struct Frame<'a> {
    node: Option<&'a CapacityFlow>,
    owner: &'a str,
    prefix: Trace<'a>,
    children: usize,
    alternative: bool,
}
impl<'a> Frame<'a> {
    fn enter(node: &'a CapacityFlow, owner: &'a str, prefix: Trace<'a>) -> Self {
        Self {
            node: Some(node),
            owner,
            prefix,
            children: 0,
            alternative: false,
        }
    }
    fn finish(children: usize, alternative: bool) -> Self {
        Self {
            node: None,
            owner: "",
            prefix: Trace::empty(),
            children,
            alternative,
        }
    }
}
fn evaluated<'a>(
    flow: &'a CapacityFlow,
    function: &'a str,
    inputs: &BTreeMap<&'a str, &'a FunctionCapacityInput>,
    query: Query,
) -> Option<Value<'a>> {
    pay(std::mem::size_of::<Work>())?;
    let mut work = Work(OPERATIONS);
    let mut frames = Stack::new()?;
    let mut values = Stack::new()?;
    frames.push(Frame::enter(flow, function, Trace::empty()))?;
    while frames.len != 0 {
        work.step()?;
        let frame = frames.pop()?;
        match frame {
            Frame {
                node: Some(node),
                owner,
                prefix,
                ..
            } => {
                work.step()?;
                let (site, operation, selected) = match node {
                    CapacityFlow::StdoutWrite { site, .. } => (
                        Some(site.as_str()),
                        "stdout_write",
                        !matches!(query, Query::Stderr),
                    ),
                    CapacityFlow::StderrWrite { site, .. } => (
                        Some(site.as_str()),
                        "stderr_write",
                        !matches!(query, Query::Stdout),
                    ),
                    CapacityFlow::StdoutAppend { site } => (
                        Some(site.as_str()),
                        "stdout_append",
                        matches!(query, Query::AnyOutput)
                            || matches!(query, Query::LoopBody) && prefix.len != 0,
                    ),
                    CapacityFlow::StderrAppend { site } => (
                        Some(site.as_str()),
                        "stderr_append",
                        matches!(query, Query::AnyOutput)
                            || matches!(query, Query::LoopBody) && prefix.len != 0,
                    ),
                    _ => (None, "", false),
                };
                if selected {
                    let trace = prefix.push(Step::Write {
                        function: owner,
                        site: site?,
                        operation,
                    })?;
                    values.push(Value {
                        count: 1,
                        hits: [Some(trace), None],
                    })?;
                    continue;
                }
                match node {
                    CapacityFlow::Call { site, callee } => {
                        let prefix = prefix.push(Step::Call {
                            function: owner,
                            site,
                            callee,
                        })?;
                        work.step()?;
                        let input = inputs.get(callee.as_str())?;
                        frames.push(Frame::enter(&input.execution, &input.function, prefix))?;
                    }
                    CapacityFlow::Sequence(children) | CapacityFlow::Alternative(children) => {
                        frames.push(Frame::finish(
                            children.len(),
                            matches!(node, CapacityFlow::Alternative(_)),
                        ))?;
                        for child in children.iter().rev() {
                            work.step()?;
                            frames.push(Frame::enter(child, owner, prefix))?;
                        }
                    }
                    CapacityFlow::Loop { condition, body }
                    | CapacityFlow::FiniteOwnedLoop {
                        condition, body, ..
                    }
                    | CapacityFlow::FusedConstruction {
                        condition, body, ..
                    } => {
                        // A loop witness needs one reachable output. Never claim
                        // condition and body are two sequential runtime writes.
                        frames.push(Frame::finish(2, true))?;
                        frames.push(Frame::enter(body, owner, prefix))?;
                        frames.push(Frame::enter(condition, owner, prefix))?;
                    }
                    _ => values.push(Value::empty())?,
                }
            }
            Frame {
                node: None,
                children,
                alternative,
                ..
            } => {
                // Children are popped in reverse, so prepend sequence witnesses.
                let mut value = Value::empty();
                for _ in 0..children {
                    work.step()?;
                    let child = values.pop()?;
                    value = if alternative {
                        if child.count >= value.count {
                            child
                        } else {
                            value
                        }
                    } else {
                        child.sequence(value)
                    };
                }
                values.push(value)?;
            }
        }
    }
    let result = values.pop()?;
    work.step()?;
    if values.len != 0 {
        return None;
    }
    Some(result)
}
struct Size(usize);
impl Write for Size {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.0 = self
            .0
            .checked_add(text.len())
            .filter(|n| *n <= OUTPUT_BYTES)
            .ok_or(fmt::Error)?;
        Ok(())
    }
}
fn output(render: impl Fn(&mut dyn Write) -> fmt::Result) -> Option<String> {
    pay(std::mem::size_of::<Size>())?;
    let mut size = Size(0);
    render(&mut size).ok()?;
    pay(size.0.checked_add(std::mem::size_of::<String>())?)?;
    let mut text = String::new();
    text.try_reserve_exact(size.0).ok()?;
    if text.capacity() != size.0 {
        return None;
    }
    render(&mut text).ok()?;
    (text.len() == size.0).then_some(text)
}
pub(super) fn repeated<'a>(
    flow: &'a CapacityFlow,
    function: &'a str,
    inputs: &BTreeMap<&'a str, &'a FunctionCapacityInput>,
    query: Query,
) -> Option<String> {
    let value = evaluated(flow, function, inputs, query)?;
    if value.count < 2 {
        return None;
    }
    output(|out| {
        out.write_str("One checked flow alternative contains these first two writes: ")?;
        value.hits[0].as_ref().ok_or(fmt::Error)?.render(out)?;
        out.write_str("; then ")?;
        value.hits[1].as_ref().ok_or(fmt::Error)?.render(out)?;
        write!(out, ". {REPAIR}")
    })
}
pub(super) fn loop_output<'a>(
    flow: &'a CapacityFlow,
    function: &'a str,
    inputs: &BTreeMap<&'a str, &'a FunctionCapacityInput>,
    part: &'static str,
) -> Option<String> {
    let query = if part == "body" {
        Query::LoopBody
    } else {
        Query::AnyOutput
    };
    let value = evaluated(flow, function, inputs, query)?;
    let hit = value.hits[0]?;
    output(|out| {
        write!(
            out,
            "Reachable output in the selected enclosing loop {part} of {function:?}: "
        )?;
        hit.render(out)?;
        write!(out, ". {REPAIR}")
    })
}
pub(super) fn cycle<'a>(
    functions: &BTreeMap<&'a str, ValidatedFunction<'a>>,
    residual: &BTreeMap<&'a str, usize>,
    start: &'a str,
) -> Option<String> {
    pay(std::mem::size_of::<[Option<&str>; STEPS + 1]>())?;
    let mut chain = [None; STEPS + 1];
    chain[0] = Some(start);
    pay(std::mem::size_of::<Work>())?;
    let mut work = Work(OPERATIONS);
    for used in 1..=STEPS {
        work.step()?;
        let current = chain[used - 1]?;
        let mut predecessor = None;
        for (identity, item) in functions {
            work.step()?;
            if residual.get(identity).copied().unwrap_or(0) != 0 && item.callees.contains(current) {
                predecessor = Some(*identity);
                break;
            }
        }
        let predecessor = predecessor?;
        work.step()?;
        if let Some(begin) = chain[..used]
            .iter()
            .position(|entry| *entry == Some(predecessor))
        {
            chain[used] = Some(predecessor);
            return output(|out| {
                out.write_str("Closed checked call chain: ")?;
                for index in (begin..=used).rev() {
                    if index != used {
                        out.write_str(" -> ")?;
                    }
                    write!(out, "{:?}", chain[index].ok_or(fmt::Error)?)?;
                }
                write!(out, ". {REPAIR}")
            });
        }
        chain[used] = Some(predecessor);
    }
    None
}

/// Re-cloning the optional owned field cannot create an unpaid failure payload.
pub(super) fn clone_help(text: &str) -> Option<String> {
    output(|out| out.write_str(text))
}
