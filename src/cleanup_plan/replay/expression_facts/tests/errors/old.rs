//! Saved add034 expression-fact producer, independent of new policies.
use super::*;
mod capacity;
#[derive(Clone)]
pub(in crate::cleanup_plan::replay::expression_facts) struct CallFact {
    pub(in crate::cleanup_plan::replay::expression_facts) callee: DeclarationId,
    pub(in crate::cleanup_plan::replay::expression_facts) instance: Option<FunctionInstanceId>,
    pub(in crate::cleanup_plan::replay::expression_facts) arguments: Vec<ExpressionId>,
    pub(in crate::cleanup_plan::replay::expression_facts) type_arguments: Vec<ResolvedType>,
}
type FactFrame<'a> = Option<(expression_children::ReplayChildCursor<'a>, bool)>;

pub(in crate::cleanup_plan::replay::expression_facts) fn expression_facts(
    function: &ResolvedFunction,
) -> Result<BTreeMap<ExpressionId, Option<CallFact>>, Diagnostic> {
    let mut facts = BTreeMap::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        collect_expression_facts(function, expression, &mut facts, None)?;
    }
    Ok(facts)
}

/// The selected entry retains this independently authenticated map across census
/// and structural replay. The legacy entry keeps its existing allocations.
pub(in crate::cleanup_plan::replay::expression_facts) fn expression_facts_with_reservation(
    function: &ResolvedFunction,
    reservation: &mut crate::cleanup_plan::replay::physical_storage::PhysicalReservation,
) -> Result<BTreeMap<ExpressionId, Option<CallFact>>, Diagnostic> {
    let mut capacity = capacity::FactCapacity::new(function, reservation)?;
    let mut facts = BTreeMap::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        collect_expression_facts(function, expression, &mut facts, Some(&mut capacity))?;
    }
    Ok(facts)
}

fn collect_expression_facts<'a>(
    function: &'a ResolvedFunction,
    expression: &'a ResolvedExpr,
    facts: &mut BTreeMap<ExpressionId, Option<CallFact>>,
    mut capacity: Option<&mut capacity::FactCapacity<'a, '_>>,
) -> Result<(), Diagnostic> {
    // One borrowed continuation per ancestor keeps the original pre-order walk
    // and avoids rescanning wide Block/Match prefixes for each child.
    let mut stack = [None; 514];
    stack[0] = Some((
        expression_children::ReplayChildCursor::new(expression),
        false,
    ));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (mut cursor, started) = stack[len].take().expect("expression-fact frame retained");
        let current = cursor.expression();
        if !started {
            if let Some(capacity) = capacity.as_deref_mut() {
                capacity.expression(current)?;
            }
            let fact = match &current.kind {
                _ if crate::hir::closure::once::call(current).is_some() => {
                    let (callee, args) = if capacity.is_some() {
                        crate::hir::closure::once::call(current).expect("same closed affine call")
                    } else {
                        let (callee, args, _) = crate::hir::function_value::cleanup_call(current)?;
                        (callee, args)
                    };
                    Some(CallFact {
                        callee: callee.clone(),
                        instance: None,
                        arguments: args.iter().map(|a| a.id.clone()).collect(),
                        type_arguments: vec![],
                    })
                }
                ResolvedExprKind::Invoke { callable, args } => Some(CallFact {
                    callee: crate::hir::function_value::INVOKE_ID.clone(),
                    instance: None,
                    arguments: args.iter().map(|a| a.id.clone()).collect(),
                    type_arguments: vec![callable.ty.clone()],
                }),
                ResolvedExprKind::VecFieldRead { args, .. } => Some(CallFact {
                    callee: crate::vec_field::operation_id().clone(),
                    instance: None,
                    arguments: args.iter().map(|argument| argument.id.clone()).collect(),
                    type_arguments: Vec::new(),
                }),
                ResolvedExprKind::LiteralFormat { args, .. } => Some(CallFact {
                    callee: crate::literal_format::operation_id().clone(),
                    instance: None,
                    arguments: args.iter().map(|argument| argument.id.clone()).collect(),
                    type_arguments: Vec::new(),
                }),
                ResolvedExprKind::Call {
                    callee,
                    instance,
                    args,
                    type_arguments,
                } => Some(CallFact {
                    callee: callee.clone(),
                    instance: instance.clone(),
                    arguments: args.iter().map(|argument| argument.id.clone()).collect(),
                    type_arguments: type_arguments.clone(),
                }),
                ResolvedExprKind::NativeRustImportCall(call)
                    if crate::cleanup_plan::native_rust::owns(current) =>
                {
                    Some(CallFact {
                        callee: call.import.clone(),
                        instance: None,
                        arguments: call
                            .args
                            .iter()
                            .map(|argument| argument.id.clone())
                            .collect(),
                        type_arguments: Vec::new(),
                    })
                }
                ResolvedExprKind::HostCommandCall(call) => Some(CallFact {
                    callee: DeclarationId::new(crate::command_io_ops::id(call.operation)),
                    instance: None,
                    arguments: call
                        .args
                        .iter()
                        .map(|argument| argument.id.clone())
                        .collect(),
                    type_arguments: Vec::new(),
                }),
                ResolvedExprKind::ByteRange {
                    operation,
                    source,
                    start,
                    end,
                } => Some(CallFact {
                    callee: operation.clone(),
                    instance: None,
                    arguments: [source.as_ref(), start.as_ref(), end.as_ref()]
                        .into_iter()
                        .map(|argument| argument.id.clone())
                        .collect(),
                    type_arguments: Vec::new(),
                }),
                _ => None,
            };
            if facts.insert(current.id.clone(), fact).is_some() {
                return Err(replay_error(
                    function,
                    format!("HIR expression identity `{}` is repeated", current.id),
                ));
            }
        }
        if let Some(child) = cursor.next() {
            if len + 2 > stack.len() {
                return Err(replay_error(
                    function,
                    "HIR expression fact traversal exceeds the admitted depth",
                ));
            }
            stack[len] = Some((cursor, true));
            stack[len + 1] = Some((expression_children::ReplayChildCursor::new(child), false));
            len += 2;
        }
    }
    Ok(())
}
