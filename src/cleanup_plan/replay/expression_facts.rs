//! Exact operation and child facts used by independent cleanup replay.
use super::*;
mod capacity;

pub(super) fn expression_facts(
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
pub(super) fn expression_facts_with_reservation(
    function: &ResolvedFunction,
    reservation: &mut super::physical_storage::PhysicalReservation,
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
    // The private replay entry admits at most 512 semantic expression levels. Keep one indexed continuation per ancestor so wide calls, records, blocks,
    // and matches never create a width-sized frontier and callback order stays identical to the former recursive pre-order walk.
    let mut stack = [None; 514];
    stack[0] = Some((expression, 0usize));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (current, next_child) = stack[len].take().expect("expression-fact frame retained");
        if next_child == 0 {
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
        if let Some(child) = replay_expression_child(current, next_child) {
            if len + 2 > stack.len() {
                return Err(replay_error(
                    function,
                    "HIR expression fact traversal exceeds the admitted depth",
                ));
            }
            stack[len] = Some((current, next_child + 1));
            stack[len + 1] = Some((child, 0));
            len += 2;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleanup_plan::replay::physical_storage::PhysicalReservation;

    fn program() -> ResolvedProgram {
        let source = crate::check(
            r#"module replay.fact_owner;
@id("fact.read") fn read(input:borrow Bytes)->usize {
 let view=bytes_as_slice(input); let range=byte_range(view,0usize,1usize); byte_len(range)
}
@id("fact.main") fn main()->i64 {
 let mut data=bytes_zeroed(1usize); let mut at=0;
 while at<2 {data=bytes_set(data,0usize,65u8);at=at+1;if at==1 {continue;};0}
 if read(data)==1usize {42}else{0}
}"#,
            "replay-fact-owner.spx",
        )
        .unwrap();
        crate::hir::resolve(&source).unwrap()
    }
    fn same(
        left: &BTreeMap<ExpressionId, Option<CallFact>>,
        right: &BTreeMap<ExpressionId, Option<CallFact>>,
    ) {
        assert_eq!(left.len(), right.len());
        for (key, left) in left {
            let right = right.get(key).expect("same complete identity inventory");
            match (left, right) {
                (Some(left), Some(right)) => {
                    assert_eq!(left.callee, right.callee);
                    assert_eq!(left.instance, right.instance);
                    assert_eq!(left.arguments, right.arguments);
                    assert_eq!(left.type_arguments, right.type_arguments);
                }
                (None, None) => {}
                _ => panic!("call-fact kind differs"),
            }
        }
    }
    #[test]
    fn selected_fact_owner_matches_legacy_exact_and_one_short() {
        let program = program();
        for function in &program.functions {
            let legacy = expression_facts(function).unwrap();
            let (selected, refused, bytes) =
                crate::bounded_output::with_limit_usage(usize::MAX, || {
                    expression_facts_with_reservation(
                        function,
                        &mut PhysicalReservation::new(usize::MAX),
                    )
                    .unwrap()
                });
            assert!(!refused);
            same(&legacy, &selected);
            assert!(bytes > 0);
            let (exact, refused, used) = crate::bounded_output::with_limit_usage(bytes, || {
                expression_facts_with_reservation(function, &mut PhysicalReservation::new(bytes))
                    .unwrap()
            });
            assert!(!refused);
            assert_eq!(used, bytes);
            same(&legacy, &exact);
            let (short, refused, used) = crate::bounded_output::with_limit_usage(bytes - 1, || {
                let mut physical = PhysicalReservation::new(usize::MAX);
                let result = expression_facts_with_reservation(function, &mut physical);
                assert!(physical.reserve(0).is_err());
                result
            });
            assert!(refused);
            assert!(used < bytes);
            assert_eq!(short.err().unwrap().code, "SPX-H006");
        }
    }
    #[test]
    fn deep_type_clone_charges_new_backing_not_borrowed_input_spare() {
        let program = program();
        let original = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "fact.read")
            .unwrap();
        let measured = |function: &ResolvedFunction| {
            crate::bounded_output::with_limit_usage(usize::MAX, || {
                expression_facts_with_reservation(
                    function,
                    &mut PhysicalReservation::new(usize::MAX),
                )
                .unwrap()
            })
        };
        let (_, refused, before) = measured(original);
        assert!(!refused);
        let mut function = original.clone();
        let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("block fixture")
        };
        let ResolvedExprKind::Call { type_arguments, .. } = &mut tail.kind else {
            panic!("call fixture")
        };
        // Fact construction is not HIR admission. This shape exercises exact
        // deep Clone storage independently of any forged type's authority.
        let mut arguments = Vec::with_capacity(31);
        arguments.push(ResolvedType::U8);
        let mut types = Vec::with_capacity(64);
        types.push(ResolvedType::Nominal {
            declaration: DeclarationId::new("test.clone.type"),
            arguments,
        });
        *type_arguments = types;
        let tail_id = tail.id.clone();
        let (selected, refused, after) = measured(&function);
        assert!(!refused);
        assert_eq!(
            after - before,
            2 * std::mem::size_of::<ResolvedType>() + "test.clone.type".len()
        );
        same(&expression_facts(&function).unwrap(), &selected);
        let fact = selected.get(&tail_id).unwrap().as_ref().unwrap();
        assert_eq!(fact.type_arguments.capacity(), 1);
        let ResolvedType::Nominal { arguments, .. } = &fact.type_arguments[0] else {
            panic!("nominal fixture")
        };
        assert_eq!(arguments.capacity(), 1);
    }

    #[test]
    fn selected_fact_owner_preserves_duplicate_identity_refusal() {
        let program = program();
        let mut function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "fact.read")
            .unwrap()
            .clone();
        let root = function.body.id.clone();
        let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("block fixture")
        };
        tail.id = root;
        let legacy = expression_facts(&function).err().unwrap();
        let selected =
            expression_facts_with_reservation(&function, &mut PhysicalReservation::new(usize::MAX))
                .err()
                .unwrap();
        assert_eq!(legacy.code, "SPX-H006");
        assert_eq!(selected.code, legacy.code);
        assert_eq!(selected.message, legacy.message);
    }
    #[test]
    fn census_constructs_once_at_original_stage_and_retains_authenticated_owner() {
        let program = program();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "fact.main")
            .unwrap();
        let mut retained = None;
        let mut calls = 0;
        super::super::lexical_census::validate_size_budget_with_expression_count(
            &program,
            function,
            || {
                calls += 1;
                let facts = expression_facts_with_reservation(
                    function,
                    &mut PhysicalReservation::new(usize::MAX),
                )?;
                let count = facts.len();
                retained = Some(facts);
                Ok(count)
            },
        )
        .unwrap();
        assert_eq!(calls, 1);
        same(
            &expression_facts(function).unwrap(),
            retained.as_ref().unwrap(),
        );
        let mut invalid = function.clone();
        invalid.cleanup_plan.blocks.clear();
        let mut called = false;
        let error = super::super::lexical_census::validate_size_budget_with_expression_count(
            &program,
            &invalid,
            || {
                called = true;
                Ok(0)
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(!called, "invalid CFG must refuse before fact construction");
    }
}
