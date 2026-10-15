//! Saved owned-environment evaluator, plus storage and semantic controls.
use super::*;
use crate::hir::DeclarationId;
use crate::kernel_zero::term::{KernelFn, KernelType};
use crate::test_allocation_observer::measure;

type OwnedEnv = Vec<(ValueId, Value)>;

fn owned_lookup(env: &OwnedEnv, id: &ValueId) -> Value {
    env.iter()
        .rev()
        .find(|(bound, _)| bound == id)
        .map(|(_, value)| *value)
        .unwrap_or_else(|| {
            panic!(
                "kernel-0 reference evaluator: unbound variable {id:?} -- \
                 the translator (`super::reify`) only ever produces closed \
                 terms for an admitted function's own parameters and lets"
            )
        })
}

/// Evaluates `entry`'s body applied to `args`, in the order `entry.params`
/// names them. `args.len()` must equal `entry.params.len()`; the translator
/// and this evaluator's only caller (the differential test) both guarantee
/// this by construction, so a mismatch panics rather than returning an
/// error -- it can only mean a bug in the caller, not a Kernel-0 program
/// that got stuck.
fn owned_eval(program: &KernelProgram, entry: &KernelFn, args: &[Value]) -> Result<Value, Fault> {
    assert_eq!(
        args.len(),
        entry.params.len(),
        "kernel-0 reference evaluator: argument count must match parameter count"
    );
    let mut env: OwnedEnv = Vec::with_capacity(args.len());
    for ((id, _), value) in entry.params.iter().zip(args.iter()) {
        env.push((id.clone(), *value));
    }
    owned_term(program, &entry.body, &mut env)
}

fn owned_term(program: &KernelProgram, term: &Term, env: &mut OwnedEnv) -> Result<Value, Fault> {
    match term {
        Term::Int(value) => Ok(Value::Int(*value)),
        Term::Bool(value) => Ok(Value::Bool(*value)),
        Term::Var(id) => Ok(owned_lookup(env, id)),
        Term::Unary(op, operand) => {
            let value = owned_term(program, operand, env)?;
            eval_unary(*op, value)
        }
        // `&&`/`||` are call-by-need per RFC 0001's "lazy boolean operands
        // execute only when required" invariant: `left` is evaluated
        // unconditionally, first, and `right` only in the one case the
        // grammar's own reduction rule names.
        Term::Binary(BinaryOp::And, left, right) => match owned_term(program, left, env)? {
            Value::Bool(false) => Ok(Value::Bool(false)),
            Value::Bool(true) => owned_term(program, right, env),
            Value::Int(_) => unreachable!(
                "kernel-0 translation only admits bool operands for `&&` (RFC 0001 typing)"
            ),
        },
        Term::Binary(BinaryOp::Or, left, right) => match owned_term(program, left, env)? {
            Value::Bool(true) => Ok(Value::Bool(true)),
            Value::Bool(false) => owned_term(program, right, env),
            Value::Int(_) => unreachable!(
                "kernel-0 translation only admits bool operands for `||` (RFC 0001 typing)"
            ),
        },
        Term::Binary(op, left, right) => {
            // Left to right, unconditionally: evaluate `left` fully, keep
            // its value, then evaluate `right`. Two sequential `let`
            // statements, not a tuple literal, so the order is this
            // function's own statement order rather than an artifact of
            // Rust's (also left-to-right, but incidental here) tuple rule.
            let left_value = owned_term(program, left, env)?;
            let right_value = owned_term(program, right, env)?;
            eval_binary(*op, left_value, right_value)
        }
        Term::If {
            condition,
            then_branch,
            else_branch,
        } => match owned_term(program, condition, env)? {
            Value::Bool(true) => owned_term(program, then_branch, env),
            Value::Bool(false) => owned_term(program, else_branch, env),
            Value::Int(_) => {
                unreachable!("kernel-0 translation only admits a bool `if` condition")
            }
        },
        Term::Let { bound, value, body } => {
            let bound_value = owned_term(program, value, env)?;
            env.push((bound.clone(), bound_value));
            let result = owned_term(program, body, env);
            env.pop();
            result
        }
        Term::Call { callee, args } => {
            // Left to right: each argument is evaluated fully, in the
            // grammar's authored order, before the callee is entered --
            // matching `f(v1,...,vn) -> body_f[x1:=v1,...,xn:=vn]`, which
            // presupposes every `vi` is already a value.
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(owned_term(program, arg, env)?);
            }
            let function = program.function(callee).unwrap_or_else(|| {
                panic!(
                    "kernel-0 reference evaluator: call to {callee:?}, absent from the \
                     translated program -- the translator only admits calls within the \
                     reifying, already-verified acyclic subgraph"
                )
            });
            owned_eval(program, function, &values)
        }
    }
}

fn parameter(name: &str) -> ValueId {
    ValueId::intrinsic_parameter(name, 0)
}
fn function(name: &str, params: Vec<(ValueId, KernelType)>, body: Term) -> KernelFn {
    KernelFn {
        id: DeclarationId::new(name),
        params,
        body,
        return_type: KernelType::I64,
    }
}

#[test]
fn borrowed_parameter_environment_has_literal_allocation_census_and_settles_each_call() {
    let id = parameter("a-long-current-parameter-identity");
    let program = KernelProgram {
        functions: vec![function(
            "entry",
            vec![(id.clone(), KernelType::I64)],
            Term::Var(id.clone()),
        )],
    };
    let entry = &program.functions[0];
    let args = [Value::Int(42)];
    let (_, old) = measure(|| {
        for _ in 0..1000 {
            assert_eq!(owned_eval(&program, entry, &args), Ok(Value::Int(42)));
        }
    });
    let (_, new) = measure(|| {
        for _ in 0..1000 {
            assert_eq!(eval_program(&program, entry, &args), Ok(Value::Int(42)));
        }
    });
    assert!(!old.invalid && !new.invalid);
    assert_eq!(old.allocations, 1000);
    assert_eq!(
        old.allocated,
        1000 * std::mem::size_of::<(ValueId, Value)>()
    );
    assert_eq!(new.allocations, 1000);
    assert_eq!(
        new.allocated,
        1000 * std::mem::size_of::<(&ValueId, Value)>()
    );
    for metrics in [old, new] {
        assert_eq!(metrics.reallocations, 0);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.freed, metrics.allocated);
    }
    assert!(new.allocated < old.allocated);
    assert_eq!(program.functions[0].params[0].0, id);
}

fn binary(op: BinaryOp, a: Term, b: Term) -> Term {
    Term::Binary(op, Box::new(a), Box::new(b))
}
fn divide_zero() -> Term {
    binary(BinaryOp::Div, Term::Int(1), Term::Int(0))
}

#[test]
fn borrowed_environments_preserve_shadow_call_order_lazy_fault_and_authenticated_renderer() {
    let x = parameter("shadow");
    let callee_x = parameter("callee-shadow");
    let callee = function(
        "helper",
        vec![(callee_x.clone(), KernelType::I64)],
        binary(BinaryOp::Add, Term::Var(callee_x), Term::Int(1)),
    );
    let shadow = Term::Let {
        bound: x.clone(),
        value: Box::new(Term::Int(10)),
        body: Box::new(Term::Let {
            bound: x.clone(),
            value: Box::new(binary(BinaryOp::Add, Term::Var(x.clone()), Term::Int(1))),
            body: Box::new(Term::Call {
                callee: callee.id.clone(),
                args: vec![Term::Var(x.clone())],
            }),
        }),
    };
    let lazy = binary(
        BinaryOp::And,
        Term::Bool(false),
        binary(BinaryOp::Eq, divide_zero(), Term::Int(0)),
    );
    let left_fault = binary(
        BinaryOp::Add,
        divide_zero(),
        binary(BinaryOp::Add, Term::Int(i64::MAX), Term::Int(1)),
    );
    let first_arg_fault = Term::Call {
        callee: DeclarationId::new("two"),
        args: vec![
            divide_zero(),
            binary(BinaryOp::Add, Term::Int(i64::MAX), Term::Int(1)),
        ],
    };
    let a = parameter("two-a");
    let b = parameter("two-b");
    let two = function(
        "two",
        vec![(a.clone(), KernelType::I64), (b.clone(), KernelType::I64)],
        binary(BinaryOp::Add, Term::Var(a), Term::Var(b)),
    );
    for (body, expected) in [
        (shadow, Ok(Value::Int(12))),
        (lazy, Ok(Value::Bool(false))),
        (left_fault, Err(Fault::DivisionByZero)),
        (first_arg_fault, Err(Fault::DivisionByZero)),
    ] {
        let mut entry = function("entry", vec![], body);
        if matches!(expected, Ok(Value::Bool(_))) {
            entry.return_type = KernelType::Bool;
        }
        let program = KernelProgram {
            functions: vec![entry, callee.clone(), two.clone()],
        };
        for _ in 0..3 {
            assert_eq!(owned_eval(&program, &program.functions[0], &[]), expected);
            assert_eq!(eval_program(&program, &program.functions[0], &[]), expected);
        }
    }
    // Let bindings are removed even when the body selects a fault. The caller's
    // binding remains, and borrowed references cannot escape this held scope.
    let program = KernelProgram { functions: vec![] };
    let faulting_let = Term::Let {
        bound: x.clone(),
        value: Box::new(Term::Int(99)),
        body: Box::new(divide_zero()),
    };
    let mut new_env = vec![(&x, Value::Int(42))];
    let mut old_env = vec![(x.clone(), Value::Int(42))];
    assert_eq!(
        owned_term(&program, &faulting_let, &mut old_env),
        Err(Fault::DivisionByZero)
    );
    assert_eq!(
        eval_term(&program, &faulting_let, &mut new_env),
        Err(Fault::DivisionByZero)
    );
    assert_eq!(old_env.len(), 1);
    assert_eq!(new_env.len(), 1);
    assert_eq!(owned_lookup(&old_env, &x), Value::Int(42));
    assert_eq!(lookup(&new_env, &x), Value::Int(42));
    let source = crate::kernel_zero::canonical_int_renderer::SOURCE;
    let entry_id = DeclarationId::new("format.render-byte");
    let binding = crate::kernel_zero::reify::BoundTranslation::derive(source, &entry_id).unwrap();
    let program = binding.replay(source, &entry_id).unwrap();
    for n in [i64::MIN, -1, 0, 1, i64::MAX] {
        for (index, byte) in n.to_string().bytes().enumerate() {
            let args = [Value::Int(n), Value::Int(index as i64)];
            let entry = program.function(&entry_id).unwrap();
            assert_eq!(
                owned_eval(program, entry, &args),
                Ok(Value::Int(i64::from(byte)))
            );
            assert_eq!(
                eval_program(program, entry, &args),
                Ok(Value::Int(i64::from(byte)))
            );
        }
    }
}
