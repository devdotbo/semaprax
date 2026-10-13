//! Borrowed authored call signatures against the independent recursive call oracle.
use std::borrow::Cow;

use super::call_parameters::{self, CallParams};
use super::helper_index::{attach_program_plans, build_program_plans};
use super::*;

const SOURCE: &str = r#"module cleanup.call_params;
@id("take") fn take(input: own Bytes, tag: i64) -> i64 { tag + i64_from_usize(byte_len(bytes_as_slice(input))) }
@id("pair_sum") fn pair_sum(left: i64, right: i64) -> i64 { left + right }
@id("identity") fn identity<T>(value: T) -> T { value }
@id("exercise") fn exercise(tag: i64) -> i64 { take(bytes_zeroed(2usize), tag) }
@id("copy_exercise") fn copy_exercise(tag: i64) -> i64 { pair_sum(tag, 1) }
@id("generic_exercise") fn generic_exercise(tag: i64) -> i64 {
  let size = take(bytes_zeroed(2usize), tag);
  identity<i64>(size)
}
@id("main") fn main() -> i64 { 0 }
"#;

fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "call-parameters.spx").unwrap()).unwrap()
}

fn function<'p>(program: &'p ResolvedProgram, id: &str) -> &'p ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap()
}

struct Lowered<'a> {
    builder: PlanBuilder<'a>,
    result: Result<EvalResult, Diagnostic>,
}

fn lower<'a>(
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    expression: &ResolvedExpr,
    recursive: bool,
) -> Lowered<'a> {
    let mut builder = PlanBuilder::new(program, function).unwrap();
    let state = builder.initial_state.clone();
    let (region, block) = (CleanupRegionId(0), BlockId(0));
    let result = if recursive {
        builder.lower_expr_recursive_reference(expression, block, state, region)
    } else {
        builder.lower_expr_iterative(expression, block, state, region)
    };
    Lowered { builder, result }
}

/// Close the lowered flow exactly like a scalar-result function and serialize
/// the resulting plan.
fn encoded(lowered: Lowered<'_>) -> impl PartialEq + std::fmt::Debug {
    let Lowered {
        mut builder,
        result,
    } = lowered;
    let evaluated = result.unwrap();
    let expression = builder.function.body.id.clone();
    builder
        .finish_success(
            evaluated.block,
            evaluated.state,
            CleanupRegionId(0),
            CleanupResultSource::Scalar { expression },
        )
        .unwrap();
    let blocks = builder
        .blocks
        .into_iter()
        .map(|block| CleanupBlock {
            id: block.id,
            region: block.region,
            transitions: block.transitions,
            terminator: block.terminator.expect("lowered block is terminated"),
        })
        .collect::<Vec<_>>();
    crate::cache_codec::encode(&CleanupPlan {
        schema: builder.schema,
        entry: BlockId(0),
        entry_state: builder.entry_state,
        slots: builder.slots,
        status_sources: builder.status_sources,
        blocks,
        edges: builder.edges,
        regions: builder.regions,
        exits: builder.exits,
        lexical_control: None,
    })
    .unwrap()
}

#[test]
fn borrowed_authored_calls_match_recursive_oracle_plan_and_bytes() {
    let program = program();
    assert!(!program.function_instances.is_empty());
    for id in ["exercise", "copy_exercise", "generic_exercise"] {
        let function = function(&program, id);
        let iterative = lower(&program, function, &function.body, false);
        let recursive = lower(&program, function, &function.body, true);
        PlanBuilder::assert_lowering_oracle(
            &iterative.builder,
            &recursive.builder,
            &iterative.result,
            &recursive.result,
            &function.body,
        );
        if id != "copy_exercise" {
            // The owned Bytes argument still commits exactly one epoch.
            assert!(iterative
                .builder
                .blocks
                .iter()
                .flat_map(|block| &block.transitions)
                .any(|transition| matches!(
                    transition,
                    CleanupTransition::CallCommit { arguments, .. } if arguments.len() == 1
                )));
        }
        assert_eq!(encoded(iterative), encoded(recursive), "{id}");
    }
    for function in &program.functions {
        assert_expression_lowering_oracle(&program, function, &function.body);
    }
    let plans = build_program_plans(&program).unwrap();
    for (function, plan) in program.functions.iter().zip(&plans) {
        assert_eq!(plan, &function.cleanup_plan);
        assert_eq!(
            crate::cache_codec::encode(plan).unwrap(),
            crate::cache_codec::encode(&function.cleanup_plan).unwrap()
        );
    }
    crate::hir::validate(&program).unwrap();
    crate::cleanup_plan::validate_program(&program).unwrap();
}

#[test]
fn authored_helper_borrows_program_signature_and_owned_carrier_counts_capacity() {
    let program = program();
    let callee = DeclarationId::new("take");
    let target = program.resolve_call_target(&callee, None).unwrap();
    let borrowed = call_parameters::authored(&program, &callee, None, 2, "probe").unwrap();
    assert!(matches!(borrowed, Cow::Borrowed(_)));
    assert!(std::ptr::eq(borrowed.as_ptr(), target.params.as_ptr()));
    assert_eq!(call_parameters::owned_capacity(&borrowed), 0);

    // The old route cloned the whole signature; that is the capacity saved.
    let cloned: CallParams<'_> = Cow::Owned(target.params.clone());
    assert!(matches!(cloned, Cow::Owned(_)));
    assert!(!std::ptr::eq(cloned.as_ptr(), borrowed.as_ptr()));
    let Cow::Owned(owned) = &cloned else {
        unreachable!("old control owns its clone")
    };
    let expected = owned.capacity() * std::mem::size_of::<crate::hir::ResolvedParam>()
        + owned
            .iter()
            .map(resolved_param_owned_capacity)
            .sum::<usize>();
    assert!(expected >= 2 * std::mem::size_of::<crate::hir::ResolvedParam>());
    assert_eq!(call_parameters::owned_capacity(&cloned), expected);

    let (_, old_allocations) = crate::test_allocation_observer::measure(|| {
        let old = target.params.clone();
        assert_eq!(old, target.params);
        drop(old);
    });
    let (_, current_allocations) = crate::test_allocation_observer::measure(|| {
        let current = call_parameters::authored(&program, &callee, None, 2, "probe").unwrap();
        assert!(std::ptr::eq(current.as_ptr(), target.params.as_ptr()));
        drop(current);
    });
    assert!(!old_allocations.invalid && !current_allocations.invalid);
    assert!(old_allocations.allocated > 0);
    assert_eq!(current_allocations.allocated, 0);
    assert_eq!(old_allocations.live, 0);
    assert_eq!(current_allocations.live, 0);

    let identity = DeclarationId::new("identity");
    for instance in &program.function_instances {
        let target = program
            .resolve_call_target(&identity, Some(&instance.id))
            .unwrap();
        let borrowed =
            call_parameters::authored(&program, &identity, Some(&instance.id), 1, "generic")
                .unwrap();
        assert!(matches!(borrowed, Cow::Borrowed(_)));
        assert!(std::ptr::eq(borrowed.as_ptr(), target.params.as_ptr()));
        assert_eq!(call_parameters::owned_capacity(&borrowed), 0);
    }
}

#[test]
fn authored_helper_rejects_hostile_arity_and_unknown_targets() {
    let program = program();
    let callee = DeclarationId::new("take");
    for arity in [0usize, 1, 3] {
        let error =
            call_parameters::authored(&program, &callee, None, arity, "hostile").unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(error.message.contains("`hostile`"));
        assert!(error.message.contains("inconsistent arity"));
    }
    let missing = DeclarationId::new("missing");
    let error = call_parameters::authored(&program, &missing, None, 2, "hostile").unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert!(error
        .message
        .contains("unknown cleanup call target `missing`"));
}

#[test]
fn hostile_authored_call_arity_fails_identically_in_both_routes() {
    let program = program();
    let function = function(&program, "exercise");
    for drop_last in [true, false] {
        let mut hostile = function.body.clone();
        let ResolvedExprKind::Block { tail, .. } = &mut hostile.kind else {
            panic!("exercise body is a block");
        };
        let ResolvedExprKind::Call { args, .. } = &mut tail.kind else {
            panic!("exercise tail is a call");
        };
        if drop_last {
            args.pop();
        } else {
            let extra = args[0].clone();
            args.push(extra);
        }
        let iterative = lower(&program, function, &hostile, false);
        let recursive = lower(&program, function, &hostile, true);
        assert!(iterative
            .result
            .as_ref()
            .unwrap_err()
            .message
            .contains("inconsistent arity"));
        PlanBuilder::assert_lowering_oracle(
            &iterative.builder,
            &recursive.builder,
            &iterative.result,
            &recursive.result,
            &hostile,
        );
    }
}

#[test]
fn ledger_usage_is_not_worse_than_oracle_and_exact_one_short_refuses() {
    let program = program();
    // The bounded ledger is not asserted to see authored signature clones:
    // Vec clones do not pass through it. Equal or lower usage is all that is
    // claimed here; the signature saving is proven by the capacity census.
    let usage = |recursive: bool| {
        let function = function(&program, "exercise");
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            lower(&program, function, &function.body, recursive)
                .result
                .is_ok()
        })
    };
    let (iterative_ok, iterative_overflowed, iterative_used) = usage(false);
    let (recursive_ok, recursive_overflowed, recursive_used) = usage(true);
    assert!(iterative_ok && recursive_ok);
    assert!(!iterative_overflowed && !recursive_overflowed);
    assert!(iterative_used <= recursive_used);

    let bytes = |program: &ResolvedProgram| {
        program
            .functions
            .iter()
            .chain(
                program
                    .function_instances
                    .iter()
                    .map(|instance| &instance.function),
            )
            .map(|function| crate::cache_codec::encode(&function.cleanup_plan).unwrap())
            .collect::<Vec<_>>()
    };
    let mut attached = program.clone();
    let before = bytes(&attached);
    let (plans, overflowed, required) =
        crate::bounded_output::with_limit_usage(usize::MAX, || build_program_plans(&attached));
    assert!(plans.is_ok());
    assert!(!overflowed);
    assert!(required > 0);
    let (plans, overflowed, used) =
        crate::bounded_output::with_limit_usage(required, || build_program_plans(&attached));
    assert!(plans.is_ok());
    assert!(!overflowed);
    assert_eq!(used, required);
    let (failure, overflowed, _) = crate::bounded_output::with_limit_usage(required - 1, || {
        attach_program_plans(&mut attached)
    });
    assert_eq!(failure.unwrap_err().code, "SPX-H006");
    assert!(overflowed);
    assert_eq!(before, bytes(&attached));
}
