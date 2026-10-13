//! Private work-metered copy of the complete original bounded canonical-key plan.
//! Every census/decimal/cursor action uses the held caller callback before acting.
//! A refused read is withheld by the parent, never an authoritative cache hit.
//! Bounded allocation-free canonical-key plan. These are optimization bounds,
//! never language/type admission bounds: any failed census uses identity_key.
//! Each BTree comparison restarts a streamed cursor (O(key bytes)); construction
//! is O(nodes), lookup O(key bytes * log(cache size)). The old API built one key
//! then compared it. Fixed stack backing is sizeof(Plan)+sizeof(Bytes) per
//! queried side, with no recursive Rust calls and no retained heap directory.
use crate::hir::ResolvedType;
type Work<'a> = dyn Fn(usize) -> bool + 'a;
const NODES: usize = 64;
// Literal fixed cursor slots plus root initialization, prepaid before copying.
pub(super) const CURSOR_INITIALIZATION_WORK: usize = NODES + 1;
const MAX_BYTES: usize = 65_536;
const MAX_VISITS: usize = 256;
#[derive(Clone, Copy)]
struct Node<'a> {
    ty: &'a ResolvedType,
    length: usize,
    end: usize,
}
#[derive(Clone, Copy)]
struct Census {
    node: usize,
    child: usize,
}
pub(super) struct Plan<'a> {
    nodes: [Option<Node<'a>>; NODES],
    count: usize,
}
fn digits(mut number: usize, work: &Work<'_>) -> Option<usize> {
    if !work(1) {
        return None;
    }
    let mut count = 1;
    while number >= 10 {
        if !work(1) {
            return None;
        }
        number /= 10;
        count += 1;
    }
    Some(count)
}
fn child_count(ty: &ResolvedType) -> Option<usize> {
    match ty {
        ResolvedType::Nominal { arguments, .. } => Some(arguments.len()),
        ResolvedType::Function { parameters, .. } => parameters.len().checked_add(1),
        _ => Some(0),
    }
}
fn child(ty: &ResolvedType, index: usize) -> Option<&ResolvedType> {
    match ty {
        ResolvedType::Nominal { arguments, .. } => arguments.get(index),
        ResolvedType::Function { parameters, result } => {
            if index == parameters.len() {
                Some(result)
            } else {
                parameters.get(index)
            }
        }
        _ => None,
    }
}
fn literal(ty: &ResolvedType) -> Option<&'static [u8]> {
    Some(match ty {
        ResolvedType::OnceFunction => b"fn-once:bytes:i64:v1",
        ResolvedType::OnceFunctionI64 => b"fn-once:bytes+i64:i64:v2",
        ResolvedType::OnceFunctionI64Pair => b"fn-once:bytes+i64+i64:i64:v3",
        ResolvedType::MutFunctionI64 => b"fn-mut:i64+i64:i64:v1",
        ResolvedType::Never => b"never:lexical-exit:v1",
        ResolvedType::Unit => b"unit",
        ResolvedType::I64 => b"i64",
        ResolvedType::I32 => b"i32",
        ResolvedType::Char => b"char",
        ResolvedType::U8 => b"u8",
        ResolvedType::Usize => b"usize",
        ResolvedType::F32 => b"f32",
        ResolvedType::F64 => b"f64",
        ResolvedType::Bool => b"bool",
        ResolvedType::String => b"string",
        ResolvedType::Bytes => b"bytes",
        ResolvedType::Str => b"str",
        ResolvedType::SliceU8 => b"slice-u8",
        ResolvedType::StringMap => b"map:string:i64:v1",
        _ => return None,
    })
}
fn prefix_length(ty: &ResolvedType, work: &Work<'_>) -> Option<usize> {
    if !work(1) {
        return None;
    }
    if let Some(bytes) = literal(ty) {
        return Some(bytes.len());
    }
    match ty {
        ResolvedType::ArrayU8(length) => {
            9usize.checked_add(digits(usize::try_from(*length).ok()?, work)?)
        }
        ResolvedType::TypeParameter { owner, index } => 12usize
            .checked_add(digits(owner.as_str().len(), work)?)?
            .checked_add(owner.as_str().len())?
            .checked_add(digits(*index as usize, work)?),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => 11usize
            .checked_add(digits(declaration.as_str().len(), work)?)?
            .checked_add(declaration.as_str().len())?
            .checked_add(digits(arguments.len(), work)?),
        ResolvedType::Function { parameters, .. } => {
            13usize.checked_add(digits(parameters.len(), work)?)
        }
        _ => None,
    }
}
impl<'a> Plan<'a> {
    pub(super) fn new(ty: &'a ResolvedType, work: &Work<'_>) -> Option<Self> {
        if !work(0) {
            return None;
        }
        Self::with_limits(ty, NODES, MAX_BYTES, MAX_VISITS, work)
    }
    fn with_limits(
        ty: &'a ResolvedType,
        nodes: usize,
        bytes: usize,
        visit_limit: usize,
        work: &Work<'_>,
    ) -> Option<Self> {
        if nodes == 0 || nodes > NODES {
            return None;
        }
        // Literal inline array initialization; not heap bytes or a quota.
        if !work(2 * NODES + 1) {
            return None;
        }
        let mut plan = Self {
            nodes: [None; NODES],
            count: 1,
        };
        plan.nodes[0] = Some(Node {
            ty,
            length: prefix_length(ty, work)?,
            end: 0,
        });
        let mut stack = [None; NODES];
        stack[0] = Some(Census { node: 0, child: 0 });
        let mut depth = 1;
        let mut visits = 0usize;
        while depth != 0 {
            if !work(1) {
                return None;
            }
            visits = visits.checked_add(1)?;
            if visits > visit_limit {
                return None;
            }
            let frame = stack[depth - 1]?;
            let node = plan.nodes[frame.node]?;
            if frame.child < child_count(node.ty)? {
                let ty = child(node.ty, frame.child)?;
                stack[depth - 1] = Some(Census {
                    child: frame.child + 1,
                    ..frame
                });
                if plan.count == nodes || depth == NODES {
                    return None;
                }
                let index = plan.count;
                plan.count += 1;
                plan.nodes[index] = Some(Node {
                    ty,
                    length: prefix_length(ty, work)?,
                    end: 0,
                });
                stack[depth] = Some(Census {
                    node: index,
                    child: 0,
                });
                depth += 1;
            } else {
                let mut length = node.length;
                let mut index = frame.node + 1;
                while index < plan.count {
                    if !work(1) {
                        return None;
                    }
                    visits = visits.checked_add(1)?;
                    if visits > visit_limit {
                        return None;
                    }
                    let child = plan.nodes[index]?;
                    length = length
                        .checked_add(digits(child.length, work)?)?
                        .checked_add(1)?
                        .checked_add(child.length)?;
                    index = child.end;
                }
                if length > bytes {
                    return None;
                }
                plan.nodes[frame.node] = Some(Node {
                    length,
                    end: plan.count,
                    ..node
                });
                depth -= 1;
            }
        }
        Some(plan)
    }
    pub(super) fn bytes(&self) -> Bytes<'_> {
        let mut stack = [None; NODES];
        stack[0] = Some(Frame {
            node: 0,
            stage: 0,
            child: 1,
        });
        Bytes {
            plan: self,
            stack,
            depth: 1,
            segment: Segment::Empty,
        }
    }
}
#[derive(Clone, Copy)]
struct Frame {
    node: usize,
    stage: u8,
    child: usize,
}
enum Segment<'a> {
    Empty,
    Text(std::slice::Iter<'a, u8>),
    Decimal {
        digits: [u8; usize::BITS as usize],
        index: usize,
    },
}
impl<'a> Segment<'a> {
    fn decimal(mut value: usize, work: &Work<'_>) -> Option<Self> {
        let mut buffer = [0; usize::BITS as usize];
        let mut index = buffer.len();
        loop {
            if !work(1) {
                return None;
            }
            index -= 1;
            buffer[index] = b'0' + (value % 10) as u8;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        Some(Self::Decimal {
            digits: buffer,
            index,
        })
    }
    fn next(&mut self) -> Option<u8> {
        match self {
            Self::Empty => None,
            Self::Text(bytes) => bytes.next().copied(),
            Self::Decimal { digits, index } => {
                let result = digits.get(*index).copied();
                *index += usize::from(result.is_some());
                result
            }
        }
    }
}
pub(super) struct Bytes<'a> {
    plan: &'a Plan<'a>,
    stack: [Option<Frame>; NODES],
    depth: usize,
    segment: Segment<'a>,
}
fn prefix<'a>(ty: &'a ResolvedType, stage: u8, work: &Work<'_>) -> Option<Segment<'a>> {
    if !work(1) {
        return None;
    }
    if let Some(text) = literal(ty) {
        return (stage == 0).then(|| Segment::Text(text.iter()));
    }
    Some(match (ty, stage) {
        (ResolvedType::ArrayU8(_), 0) => Segment::Text(b"array:u8:".iter()),
        (ResolvedType::ArrayU8(length), 1) => {
            Segment::decimal(usize::try_from(*length).ok()?, work)?
        }
        (ResolvedType::TypeParameter { .. }, 0) => Segment::Text(b"parameter:".iter()),
        (ResolvedType::TypeParameter { owner, .. }, 1) => {
            Segment::decimal(owner.as_str().len(), work)?
        }
        (ResolvedType::TypeParameter { .. }, 2 | 4) => Segment::Text(b":".iter()),
        (ResolvedType::TypeParameter { owner, .. }, 3) => {
            Segment::Text(owner.as_str().as_bytes().iter())
        }
        (ResolvedType::TypeParameter { index, .. }, 5) => Segment::decimal(*index as usize, work)?,
        (ResolvedType::Nominal { .. }, 0) => Segment::Text(b"nominal:".iter()),
        (ResolvedType::Nominal { declaration, .. }, 1) => {
            Segment::decimal(declaration.as_str().len(), work)?
        }
        (ResolvedType::Nominal { .. }, 2 | 4 | 6) => Segment::Text(b":".iter()),
        (ResolvedType::Nominal { declaration, .. }, 3) => {
            Segment::Text(declaration.as_str().as_bytes().iter())
        }
        (ResolvedType::Nominal { arguments, .. }, 5) => Segment::decimal(arguments.len(), work)?,
        (ResolvedType::Function { .. }, 0) => Segment::Text(b"function:v1:".iter()),
        (ResolvedType::Function { parameters, .. }, 1) => Segment::decimal(parameters.len(), work)?,
        (ResolvedType::Function { .. }, 2) => Segment::Text(b":".iter()),
        _ => return None,
    })
}
impl Bytes<'_> {
    pub(super) fn next(&mut self, work: &Work<'_>) -> Option<u8> {
        loop {
            if !work(1) {
                return None;
            }
            if let Some(byte) = self.segment.next() {
                return Some(byte);
            }
            if self.depth == 0 {
                return None;
            }
            let frame = self.stack[self.depth - 1]
                .as_mut()
                .expect("validated key cursor");
            let node = self.plan.nodes[frame.node].expect("validated key node");
            if frame.stage < 16 {
                let segment = prefix(node.ty, frame.stage, work);
                if !work(0) {
                    return None;
                }
                if let Some(segment) = segment {
                    frame.stage += 1;
                    self.segment = segment;
                    continue;
                }
                frame.stage = 16;
            }
            if frame.child < node.end {
                let child = self.plan.nodes[frame.child].expect("validated child");
                match frame.stage {
                    16 => {
                        self.segment = Segment::decimal(child.length, work)?;
                        frame.stage = 17;
                    }
                    17 => {
                        self.segment = Segment::Text(b":".iter());
                        frame.stage = 18;
                    }
                    _ => {
                        let index = frame.child;
                        frame.child = child.end;
                        frame.stage = 16;
                        self.stack[self.depth] = Some(Frame {
                            node: index,
                            stage: 0,
                            child: index + 1,
                        });
                        self.depth += 1;
                    }
                }
            } else {
                self.depth -= 1;
            }
        }
    }
}

#[cfg(test)]
mod tests;
