//! Allocation-free depth guard and census of the old inventories retained by
//! delayed publication. This is physical overlap, never semantic authority.
use super::*;
use crate::cleanup::FieldLivenessShape;
const DEPTH: usize = 514;
#[derive(Clone, Copy)]
enum Node<'a> {
    Type(&'a ResolvedType),
    Shape(&'a FieldLivenessShape),
}
type Cursor<'a> = [Option<(Node<'a>, usize)>; DEPTH];
fn charge(work: &mut usize, count: usize) -> Result<(), Diagnostic> {
    *work = work
        .checked_sub(count)
        .ok_or_else(|| plan_error("attached cleanup census work exhausted"))?;
    Ok(())
}
fn guard<'a>(root: Node<'a>, cursor: &mut Cursor<'a>, work: &mut usize) -> Result<(), Diagnostic> {
    cursor[0] = Some((root, 0));
    let mut depth = 1;
    while depth != 0 {
        charge(work, 1)?;
        let (node, next) = cursor[depth - 1].expect("active census frame");
        cursor[depth - 1] = Some((node, next + 1));
        let child = match node {
            Node::Type(ResolvedType::Function { parameters, result }) => {
                if next == 0 {
                    Some(Node::Type(result))
                } else {
                    parameters.get(next - 1).map(Node::Type)
                }
            }
            Node::Type(ResolvedType::Nominal { arguments, .. }) => {
                arguments.get(next).map(Node::Type)
            }
            Node::Shape(FieldLivenessShape::Record { fields, .. }) => {
                fields.get(next).map(|field| Node::Shape(&field.shape))
            }
            _ => None,
        };
        if let Some(child) = child {
            if depth == DEPTH {
                return Err(plan_error(
                    "attached cleanup census depth exceeds admitted limit",
                ));
            }
            cursor[depth] = Some((child, 0));
            depth += 1;
        } else {
            cursor[depth - 1] = None;
            depth -= 1;
        }
    }
    Ok(())
}
fn plan<'a>(
    plan: &'a CleanupPlan,
    cursor: &mut Cursor<'a>,
    work: &mut usize,
) -> Result<usize, Diagnostic> {
    // Bound the flat inventories before walking recursive slot/result types.
    for count in [
        plan.slots.len(),
        plan.blocks.len(),
        plan.edges.len(),
        plan.regions.len(),
        plan.exits.len(),
        plan.status_sources.len(),
    ] {
        charge(work, count)?;
    }
    for slot in &plan.slots {
        guard(Node::Type(&slot.ty), cursor, work)?;
        guard(Node::Shape(&slot.field_liveness_shape), cursor, work)?;
    }
    for block in &plan.blocks {
        charge(work, block.transitions.len())?;
        for transition in &block.transitions {
            if let CleanupTransition::StageCopyResult { source } = transition {
                match source {
                    StagedCopyResultSource::Body { instance, .. } => {
                        guard(Node::Type(instance), cursor, work)?
                    }
                    StagedCopyResultSource::TryResidual {
                        source_instance,
                        target_instance,
                        ..
                    }
                    | StagedCopyResultSource::TryOptionNone {
                        source_instance,
                        target_instance,
                        ..
                    } => {
                        guard(Node::Type(source_instance), cursor, work)?;
                        guard(Node::Type(target_instance), cursor, work)?;
                    }
                }
            }
        }
    }
    crate::private_capacity_contract::cleanup_plan_owned_capacity(plan)
        .ok_or_else(|| plan_error("attached cleanup physical capacity overflowed"))
}
pub(super) fn program(program: &ResolvedProgram) -> Result<usize, Diagnostic> {
    let mut work = crate::cache_codec::MAX_NODES;
    let functions = || {
        program.functions.iter().chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
    };
    charge(&mut work, program.functions.len())?;
    charge(&mut work, program.function_instances.len())?;
    let mut recursive = false;
    for function in functions() {
        let old = &function.cleanup_plan;
        charge(&mut work, old.slots.len())?;
        charge(&mut work, old.blocks.len())?;
        recursive |= !old.slots.is_empty();
        for block in &old.blocks {
            charge(&mut work, block.transitions.len())?;
            recursive |= block
                .transitions
                .iter()
                .any(|transition| matches!(transition, CleanupTransition::StageCopyResult { .. }));
        }
    }
    if !recursive {
        return functions().try_fold(0usize, |bytes, function| {
            let old = crate::private_capacity_contract::cleanup_plan_owned_capacity(
                &function.cleanup_plan,
            )
            .ok_or_else(|| plan_error("attached cleanup physical capacity overflowed"))?;
            bytes
                .checked_add(old)
                .ok_or_else(|| plan_error("attached cleanup overlap capacity overflowed"))
        });
    }
    // Reserve one real fixed carrier and reuse it across every old type/shape.
    if !crate::bounded_output::reserve_active_required(std::mem::size_of::<Cursor<'_>>()) {
        return Err(plan_error(
            "attached cleanup census scratch exceeds admitted limit",
        ));
    }
    let mut cursor: Cursor<'_> = [None; DEPTH];
    functions().try_fold(0usize, |bytes, function| {
        bytes
            .checked_add(plan(&function.cleanup_plan, &mut cursor, &mut work)?)
            .ok_or_else(|| plan_error("attached cleanup overlap capacity overflowed"))
    })
}
