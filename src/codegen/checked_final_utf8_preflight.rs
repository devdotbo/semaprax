//! Read-only preflight for the proposed checked final-UTF8 native profile.
//! Ordinary HIR replay runs first. These observations are not a certificate,
//! profile grant, allocator binding, clone proof, or permission to emit C.
use crate::cleanup_plan::{StatusLane, StatusProducer};
use crate::diagnostic::Diagnostic;
use crate::hir::{self, DeclarationId, ExpressionId, ResolvedExprKind as K};

const MAX_FUNCTIONS: usize = 256;
const MAX_SITES: usize = 4096;
const MAX_WORK: usize = 131_072;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SiteKind {
    /// A final constructor candidate, not authority for allocation failure.
    FromUtf8 {
        existing_propagated_failure: bool,
    },
    /// Literal rendering, including empty Strings, allocates an owner today.
    Literal,
    /// The native ordinary operation returns a String; it needs selected proof.
    OtherStringOperation {
        callee: DeclarationId,
    },
    /// Native place rendering must independently distinguish moves from clones.
    StringPlace,
    /// Aggregate/capture/default/drop proof remains separate from this census.
    OtherStringValue,
    /// Runtime/import/instance behavior cannot be inferred from a signature.
    UnexpandedCall {
        callee: DeclarationId,
    },
    DynamicBoundary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Site {
    pub function: DeclarationId,
    pub expression: ExpressionId,
    pub kind: SiteKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inventory {
    /// Authoritative declaration identities, in deterministic discovery order.
    pub functions: Vec<DeclarationId>,
    /// Authored-order observations, including contracts, never sorted cleanup.
    pub sites: Vec<Site>,
    /// Charged walk work, including the child accessor's linear search bound.
    pub work: usize,
}

/// Inspect one monomorphic direct-call closure after ordinary full HIR replay.
/// Instances/imports/dynamic calls are retained as unexpanded observations;
/// an empty inventory cannot authorize emission or prove absence of hidden
/// aggregate clones. Limits bound this optional report, not language admission.
pub fn inspect(
    program: &hir::ResolvedProgram,
    entry: &DeclarationId,
) -> Result<Inventory, Diagnostic> {
    hir::validate(program)?;
    let index = program
        .functions
        .iter()
        .position(|f| &f.id == entry)
        .ok_or_else(|| error("entry is not a monomorphic function in this program"))?;
    let mut pending = vec![index];
    let mut output = Inventory {
        functions: Vec::new(),
        sites: Vec::new(),
        work: 0,
    };
    let mut cursor = 0;
    while cursor < pending.len() {
        let function = &program.functions[pending[cursor]];
        cursor += 1;
        output.functions.push(function.id.clone());
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            let mut stack = [None; crate::cache_codec::MAX_DEPTH + 1];
            stack[0] = Some((root, 0usize));
            let mut length = 1;
            while length != 0 {
                let (expression, child_index) =
                    stack[length - 1].expect("occupied inventory frame");
                // child(Block, n) scans the authored statements up to n; this
                // conservative charge prevents a flat-block quadratic escape.
                charge(&mut output.work, child_index.saturating_add(1))?;
                if child_index == 0 {
                    let observation = match &expression.kind {
                        K::Call {
                            callee, instance, ..
                        } => {
                            charge(&mut output.work, program.functions.len())?;
                            let target = instance
                                .is_none()
                                .then(|| program.functions.iter().position(|f| &f.id == callee))
                                .flatten();
                            if let Some(target) = target {
                                charge(&mut output.work, pending.len())?;
                                if !pending.contains(&target) {
                                    if pending.len() == MAX_FUNCTIONS {
                                        return Err(error("function inventory limit exceeded"));
                                    }
                                    pending.push(target);
                                }
                                (expression.ty == hir::ResolvedType::String)
                                    .then_some(SiteKind::OtherStringValue)
                            } else if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                                if op == crate::string_ops::StringOp::FromUtf8 {
                                    charge(
                                        &mut output.work,
                                        function.cleanup_plan.status_sources.len(),
                                    )?;
                                    Some(SiteKind::FromUtf8 { existing_propagated_failure:
                                        function.cleanup_plan.status_sources.iter().any(|source|
                                            source.id.expression == expression.id && source.id.lane == StatusLane::OperationFailure &&
                                            matches!(&source.producer, StatusProducer::PropagatedCall { callee: actual } if actual == callee)) })
                                } else if expression.ty == hir::ResolvedType::String {
                                    Some(SiteKind::OtherStringOperation {
                                        callee: callee.clone(),
                                    })
                                } else {
                                    Some(SiteKind::UnexpandedCall {
                                        callee: callee.clone(),
                                    })
                                }
                            } else {
                                Some(SiteKind::UnexpandedCall {
                                    callee: callee.clone(),
                                })
                            }
                        }
                        K::String(_) | K::LiteralFormat { .. } => Some(SiteKind::Literal),
                        K::Place(_) if expression.ty == hir::ResolvedType::String => {
                            Some(SiteKind::StringPlace)
                        }
                        K::Invoke { .. }
                        | K::Closure { .. }
                        | K::FunctionReference { .. }
                        | K::NativeRustImportCall(_)
                        | K::HostCommandCall(_)
                        | K::Yield { .. } => Some(SiteKind::DynamicBoundary),
                        _ if expression.ty == hir::ResolvedType::String => {
                            Some(SiteKind::OtherStringValue)
                        }
                        _ => None,
                    };
                    if let Some(kind) = observation {
                        if output.sites.len() == MAX_SITES {
                            return Err(error("site inventory limit exceeded"));
                        }
                        output.sites.push(Site {
                            function: function.id.clone(),
                            expression: expression.id.clone(),
                            kind,
                        });
                    }
                }
                if let Some(next) = hir::lexical_exit::child(expression, child_index) {
                    if length == stack.len() {
                        return Err(error("expression inventory depth exceeded"));
                    }
                    stack[length - 1] = Some((expression, child_index + 1));
                    stack[length] = Some((next, 0));
                    length += 1;
                } else {
                    stack[length - 1] = None;
                    length -= 1;
                }
            }
        }
    }
    Ok(output)
}

fn charge(work: &mut usize, amount: usize) -> Result<(), Diagnostic> {
    *work = work
        .checked_add(amount)
        .filter(|n| *n <= MAX_WORK)
        .ok_or_else(|| error("constructor inventory work limit exceeded"))?;
    Ok(())
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-B104",
        format!("checked final-UTF8 preflight: {message}"),
    )
}

#[cfg(test)]
mod tests;
