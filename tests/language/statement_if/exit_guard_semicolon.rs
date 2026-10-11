//! A bare valued exit guard may end with `;` when another block item follows.
//! Every admitted case pairs that spelling with the current unterminated
//! control: `~` marks each guard terminator and is removed for the control.
use super::scalar_expression::backends;
use semaprax::{ast, check, codegen, format, graph, hir, parse, verify, wasm};

const PATH: &str = "exit-guard-semicolon.spx";

/// Nested while loops: continue, break and return guards, ordinary branch
/// effects, a lazy right operand that fails only when evaluated, and ensures.
const LOOPS: &str = r#"module exit.guard.loops;
@id("exit.guard.scan")
fn scan(limit: i64, fail: bool) -> i64
    ensures result >= 0
{
    let mut total = 0;
    let mut outer = 0;
    while outer < limit {
        outer = outer + 1;
        let mut inner = 0;
        while inner < 4 {
            inner = inner + 1;
            if inner == 2 { continue; } else { total = total + 100; 0 }~
            if inner == 3 { break; } else { 0 }~
            total = total + inner;
            0
        }
        if outer == 2 { continue; } else { 0 }~
        if fail && total / (outer - outer) > 0 { return total; } else { 0 }~
        if outer == 3 { break; } else { 0 }~
        total = total * 2;
        0
    }
    if total > 100000 { return 1; } else { 0 }~
    total
}
@id("main")
fn main() -> i64
{
    scan(5, false)
}
"#;

/// Inner and outer guards must keep their own nearest loop targets.
const NESTED: &str = "module exit.guard.nested; @id(\"main\") fn main()->i64 { let mut outer=0; let mut seen=0; while outer<2 { outer=outer+1; let mut inner=0; while inner<3 { inner=inner+1; if inner==2 {continue;} else {0}~ seen=seen+inner; 0 } if outer==1 {continue;} else {0}~ seen=seen+10; 0 } seen }";

/// A Copy source `for` continue must still run its exact latch once.
const FOR_LATCH: &str = r#"module exit.guard.for;
@id("main")
fn main() -> i64
{
    let mut building = vec_with_capacity<i64>(4usize);
    building = vec_push<i64>(building, 10);
    building = vec_push<i64>(building, 33);
    building = vec_push<i64>(building, 7);
    building = vec_push<i64>(building, 5);
    let values = building;
    let mut sum = 0;
    for item in values {
        if item == 33 { continue; } else { 0 }~
        if item == 5 { break; } else { sum = sum + 1000; 0 }~
        sum = sum + item;
        0
    }
    sum
}
"#;

const SIMPLE: &str = "module exit.guard.forged; @id(\"main\") fn main()->i64 { let mut i=0; while i<3 { i=i+1; if i==2 {continue;} else {0}~ i=i+0; 0 } i }";

fn spellings(template: &str) -> (String, String) {
    assert!(template.contains('~'), "template marks its guard terminators");
    (template.replace('~', ";"), template.replace('~', ""))
}

fn codes(result: &Result<ast::Program, Vec<semaprax::diagnostic::Diagnostic>>) -> Vec<&str> {
    match result {
        Ok(_) => Vec::new(),
        Err(errors) => errors.iter().map(|d| d.code).collect(),
    }
}

const CROSSING: &str = "lexical exit crosses a value-evaluation or callable boundary";

/// Every exact `SPX-T312` refusal: message and exit span.
fn exact_t312(source: &str) -> Vec<(String, String)> {
    check(source, PATH)
        .unwrap_err()
        .into_iter()
        .filter(|d| d.code == "SPX-T312")
        .map(|d| (d.message, format!("{:?}", d.span)))
        .collect()
}

/// Both spellings refuse with the same exact exit-crossing diagnostics.
fn same_exit_refusal(terminated: &str, control: &str) {
    let refused = exact_t312(terminated);
    assert_eq!(refused, exact_t312(control), "{terminated}");
    assert!(!refused.is_empty(), "{terminated}");
    for (message, _) in &refused {
        assert_eq!(message, CROSSING, "{terminated}");
    }
}

/// The terminated spelling is the control program: one canonical text and
/// revision, one graph, one checked HIR, and identical admitted native C and
/// Core Wasm modules. No emitter refusal is accepted as equivalence.
fn same_program(terminated: &str, control: &str) {
    let new = check(terminated, PATH).unwrap_or_else(|e| panic!("{terminated}: {e:?}"));
    let old = check(control, PATH).unwrap_or_else(|e| panic!("{control}: {e:?}"));
    let canonical = format::canonical(&old);
    assert_eq!(format::canonical(&new), canonical);
    assert!(!canonical.contains("let _if"), "{canonical}");
    assert_eq!(format::canonical(&check(&canonical, PATH).unwrap()), canonical);
    assert_eq!(graph::revision(&new), graph::revision(&old));
    let json = graph::to_json(&old).unwrap();
    assert_eq!(graph::to_json(&new).unwrap(), json);
    graph::verify_json(&new, &json).unwrap();
    hir::validate(&hir::resolve(&new).unwrap()).unwrap();
    hir::validate(&hir::resolve(&old).unwrap()).unwrap();
    assert_eq!(codegen::emit_c(&new).unwrap(), codegen::emit_c(&old).unwrap());
    assert_eq!(
        wasm::emit_module(&new).unwrap(),
        wasm::emit_module(&old).unwrap()
    );
}

/// Ordinary CLI execution: interpreter, or native C when requested.
fn outcome(source: &str, native: bool) -> (Option<i32>, String, bool) {
    let path = super::write_source(if native { "guard-native" } else { "guard-run" }, source);
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_semaprax"));
    command.arg("run").arg(&path);
    if native {
        command.arg("--native");
    }
    let output = command.output().unwrap();
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        output.stderr.is_empty(),
    )
}

fn same_execution(terminated: &str, control: &str) -> (Option<i32>, String, bool) {
    let interpreted = outcome(control, false);
    assert_eq!(outcome(terminated, false), interpreted);
    if super::command_available("clang") {
        assert_eq!(outcome(terminated, true), outcome(control, true));
    }
    interpreted
}

#[test]
fn terminated_loop_guards_match_unterminated_controls_on_every_backend() {
    let (terminated, control) = spellings(LOOPS);
    same_program(&terminated, &control);
    let (status, stdout, _) = same_execution(&terminated, &control);
    assert_eq!((status, stdout.as_str()), (Some(0), "804"));
    backends::assert_execution_pair(&terminated, &control, Some(804), false);
    // Selecting the lazy operand fails inside the guard condition; the checked
    // status stays selected and no later result or postcondition publishes.
    let failing = LOOPS.replace("scan(5, false)", "scan(5, true)");
    let (terminated, control) = spellings(&failing);
    same_program(&terminated, &control);
    let (status, stdout, quiet) = same_execution(&terminated, &control);
    assert_ne!(status, Some(0));
    assert!(stdout.is_empty() && !quiet);
    backends::assert_execution_pair(&terminated, &control, None, false);
}

#[test]
fn terminated_guards_keep_their_nearest_nested_loop_targets() {
    let (terminated, control) = spellings(NESTED);
    same_program(&terminated, &control);
    let (status, stdout, _) = same_execution(&terminated, &control);
    assert_eq!((status, stdout.as_str()), (Some(0), "18"));
    backends::assert_execution_pair(&terminated, &control, Some(18), false);
}

/// Independent oracle for FOR_LATCH over [10, 33, 7, 5]: 10 adds 1000 then 10;
/// 33 continues through the latch with no effect; 7 adds 1000 then 7; 5 takes
/// the break arm, whose alternative (the +1000) does not run. 1010 + 1007.
const FOR_LATCH_RESULT: i64 = 2017;

/// The Core Wasm host of the owning Copy-Vec gate, with checked arithmetic.
const VEC_HOST: &str = r#"
const fs=require('fs');
const bytes=fs.readFileSync(process.argv[1]),expected=BigInt(process.argv[2]);
const MIN=-(2n**63n),MAX=2n**63n-1n;
const checked=v=>{if(v<MIN||v>MAX)throw Error('arithmetic');return v};
let next=1n;
const entries=new Map();
const key=value=>{if(typeof value!=='bigint'||value===0n)throw Error('carrier');return value.toString()};
const read=(value,tag)=>{const entry=entries.get(key(value));if(!entry||entry.tag!==tag)throw Error('stale-or-type');return entry};
const alloc=(tag,capacity,values=[])=>{const token=next++;entries.set(key(token),{tag,capacity,values});return token};
const env={
  spx_add:(a,b)=>checked(a+b),spx_sub:(a,b)=>checked(a-b),spx_mul:(a,b)=>checked(a*b),
  spx_div:(a,b)=>{if(b===0n||(a===MIN&&b===-1n))throw Error('arithmetic');return a/b},
  spx_rem:(a,b)=>{if(b===0n||(a===MIN&&b===-1n))throw Error('arithmetic');return a%b},
  spx_neg:a=>checked(-a),
  spx_contract_fail:code=>{throw Error(`unexpected-status:${code}`)},
  spx_vec_with_capacity:(tag,capacity)=>{const n=Number(capacity);return Number.isSafeInteger(n)&&n>=0&&n<=8192?alloc(tag,n):0n},
  spx_vec_push:(source,tag,bits)=>{const old=read(source,tag);if(old.values.length>=old.capacity)return 0n;const values=old.values.concat([bits]);entries.delete(key(source));return alloc(tag,old.capacity,values)},
  spx_vec_len:(source,tag)=>BigInt(read(source,tag).values.length),
  spx_vec_capacity:(source,tag)=>BigInt(read(source,tag).capacity),
  spx_vec_get:(source,tag,index)=>{const entry=read(source,tag),n=Number(index);if(!Number.isSafeInteger(n)||n<0||n>=entry.values.length)throw Error('oob');return entry.values[n]},
  spx_vec_drop:source=>{if(!entries.delete(key(source)))throw Error('double-drop')},
  spx_vec_reserve_exact:(source,tag,additional)=>{const old=read(source,tag),n=Number(additional),capacity=Math.max(old.capacity,old.values.length+n);if(!Number.isSafeInteger(n)||n<0||capacity>8192)return 0n;entries.delete(key(source));return alloc(tag,capacity,old.values.slice())},
  spx_vec_set:(source,tag,index,bits)=>{const old=read(source,tag),n=Number(index);if(!Number.isSafeInteger(n)||n<0||n>=old.values.length)return 0n;const values=old.values.slice();values[n]=bits;entries.delete(key(source));return alloc(tag,old.capacity,values)},
  spx_vec_clear:(source,tag)=>{const old=read(source,tag);entries.delete(key(source));return alloc(tag,old.capacity,[])}
};
WebAssembly.instantiate(bytes,{env}).then(({instance})=>{
  for(let i=0;i<4;i+=1){const value=instance.exports.semaprax_main();if(value!==expected||entries.size!==0)throw Error(`semantic-or-settlement:${value}:${entries.size}`)}
}).catch(error=>{console.error(error);process.exit(2)});
"#;

#[test]
fn terminated_copy_for_guards_run_the_exact_latch_on_every_backend() {
    assert!(
        super::command_available("clang") && super::command_available("node"),
        "Copy for-latch backend evidence requires clang and Node"
    );
    let (terminated, control) = spellings(FOR_LATCH);
    same_program(&terminated, &control);
    let (status, stdout, _) = same_execution(&terminated, &control);
    assert_eq!((status, stdout), (Some(0), FOR_LATCH_RESULT.to_string()));
    for source in [&terminated, &control] {
        let program = check(source, PATH).unwrap();
        let path = super::write_source("for-latch", source);
        let root = path.parent().unwrap();
        // Native O0/O2 through the checked status ABI, three entries each.
        backends::native(&program, root, Some(FOR_LATCH_RESULT));
        let module = root.join("program.wasm");
        std::fs::write(&module, wasm::emit_module(&program).unwrap()).unwrap();
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(VEC_HOST)
            .arg(&module)
            .arg(FOR_LATCH_RESULT.to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Core Wasm: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn many_terminated_guards_share_the_control_names_and_revision() {
    let mut guards = String::new();
    for index in 0..256 {
        guards.push_str(&format!("if i=={index} {{continue;}} else {{0}}~ "));
    }
    let template = format!(
        "module exit.guard.many; @id(\"main\") fn main()->i64 {{ let mut i=0; while i<300 {{ i=i+1; {guards}i=i+0; 0 }} i }}"
    );
    let (terminated, control) = spellings(&template);
    let new = check(&terminated, PATH).unwrap();
    let old = check(&control, PATH).unwrap();
    assert_eq!(format::canonical(&new), format::canonical(&old));
    assert_eq!(graph::revision(&new), graph::revision(&old));
}

#[test]
fn terminated_guard_nesting_bound_is_the_control_bound_exactly() {
    let nested = |depth: usize, terminator: &str| {
        let mut body = format!("if i==1 {{continue;}} else {{0}}{terminator} x=x+1;");
        for _ in 0..depth {
            body = format!("if true {{ {body} }}");
        }
        format!(
            "module exit.guard.depth; @id(\"main\") fn main()->i64 {{ let mut i=0; let mut x=0; while i<2 {{ i=i+1; {body} 0 }} x }}"
        )
    };
    let limit = (0..=256)
        .find(|depth| check(&nested(*depth, ""), PATH).is_err())
        .expect("the control reaches its nesting bound");
    assert!(limit > 0, "the shallowest control guard is admitted");
    for depth in [limit - 1, limit] {
        let new = check(&nested(depth, ";"), PATH);
        let old = check(&nested(depth, ""), PATH);
        assert_eq!(codes(&new), codes(&old), "depth {depth}");
        assert_eq!(new.is_ok(), depth < limit, "depth {depth}");
        if let (Ok(new), Ok(old)) = (new, old) {
            assert_eq!(format::canonical(&new), format::canonical(&old));
        }
    }
}

#[test]
fn value_boundaries_and_outside_loop_rules_stay_closed() {
    let in_loop = |item: &str| {
        format!(
            "module exit.guard.closed; permit {{ unsafe }} @id(\"exit.guard.id\") fn id(v:i64)->i64 {{v}} @id(\"exit.guard.pair\") record Pair {{ @id(\"exit.guard.pair.a\") a:i64, }} @id(\"main\") fn main()->i64 {{ let mut i=0; while i<2 {{ i=i+1; {item} }} i }}"
        )
    };
    for item in [
        // Condition, initializer, argument, aggregate, return operand,
        // closure and unsafe evaluation remain value boundaries.
        "if if i==1 {continue;} else {true} {0} else {0}; i=i+0; 0",
        "if if i==1 {continue;} else {true} {0} else {0} i=i+0; 0",
        "let g=if i==1 {continue;} else {0}; 0",
        "id(if i==1 {continue;} else {0}); 0",
        "let p=Pair{a:if i==1 {continue;} else {0}}; 0",
        "return if i==1 {continue;} else {0};",
        "let f=fn(v:i64)->i64 { if v==1 {continue;} else {0}; v }; 0",
        "@audit(\"guard\") unsafe { if i==1 {continue;} else {0}; 0 } 0",
        // Postfix and binary continuations keep the generic discard.
        "if i==1 {continue;} else {0} + 1; 0",
        "if i==1 {continue;} else {0} == 0; 0",
        // Canonical text omits an admitted `;`; a next item that could
        // continue the value `if` therefore never selects the statement route.
        "if i==1 {continue;} else {0}; -1; 0",
        "if i==1 {continue;} else {0}; (i); 0",
    ] {
        let diagnostics = check(&in_loop(item), PATH).unwrap_err();
        assert!(
            diagnostics.iter().any(|d| d.code == "SPX-T312"),
            "{item}: {diagnostics:?}"
        );
    }
    // Outside a loop the terminated guard is the control's T313.
    let outside = "module exit.guard.outside; @id(\"main\") fn main()->i64 { let x=1; if x==1 {continue;} else {0}~ x }";
    let (terminated, control) = spellings(outside);
    for source in [terminated, control] {
        assert!(check(&source, PATH)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "SPX-T313"));
    }
    // Unconditional exits and a final `;` keep their parser refusals.
    let all_exit = in_loop("if i==1 {continue;} else {break;}~ 0");
    let (terminated, control) = spellings(&all_exit);
    assert_eq!(parse(&terminated, PATH).unwrap_err().code, "SPX-P209");
    assert_eq!(parse(&control, PATH).unwrap_err().code, "SPX-P209");
    let final_semicolon = in_loop("if i==1 {continue;} else {0};");
    assert_eq!(parse(&final_semicolon, PATH).unwrap_err().code, "SPX-P106");
    // A valued `if` without an exit keeps its exact scalar statement identity.
    let scalar = parse(
        "module exit.guard.scalar; @id(\"main\") fn main()->i64 { let x=0; if x==0 {1} else {2}; x }",
        PATH,
    )
    .unwrap();
    let ast::ExprKind::Block { statements, .. } = &scalar.functions[0].body.kind else {
        panic!("function block")
    };
    assert!(matches!(
        &statements[1],
        ast::Statement::Let {
            syntax: ast::LetSyntax::ExpressionStatement,
            ..
        }
    ));
    assert!(format::canonical(&scalar).contains("else { 2 };"));
}

#[test]
fn guard_completions_other_than_the_scalar_zero_stay_refused_in_both_spellings() {
    let template = |completion: &str| {
        format!(
            "module exit.guard.completion; @id(\"exit.guard.pair\") record Pair {{ @id(\"exit.guard.pair.a\") a:i64, }} @id(\"main\") fn main()->i64 {{ let text=\"text\"; let mut i=0; while i<2 {{ i=i+1; if i==1 {{continue;}} else {{{completion}}}~ i=i+0; 0 }} string_len(text) }}"
        )
    };
    // Owned, aggregate, borrowed-view, nonzero, non-i64 and Bool normal arms
    // would be stored: no owned, aggregate or borrowed discard is admitted.
    for completion in [
        "\"owned\"",
        "Pair{a:1}",
        "string_as_str(text)",
        "7",
        "0i32",
        "false",
    ] {
        let (terminated, control) = spellings(&template(completion));
        same_exit_refusal(&terminated, &control);
        assert_eq!(exact_t312(&terminated).len(), 1, "{completion}");
    }
    // The same guard with the scalar discard zero is admitted in both.
    let (terminated, control) = spellings(&template("0"));
    let new = check(&terminated, PATH).unwrap();
    let old = check(&control, PATH).unwrap();
    assert_eq!(format::canonical(&new), format::canonical(&old));
    hir::validate(&hir::resolve(&new).unwrap()).unwrap();
}

#[test]
fn yields_functions_keep_the_exit_crossing_refusal() {
    let source = "module exit.guard.yields; @id(\"exit.guard.ask\") fn ask(seed: i64) -> i64 yields i64 -> i64 { let mut i=0; while i<2 { i=i+1; if i==1 {continue;} else {0}~ i=i+0; 0 } let answer = yield seed + i; answer } @id(\"main\") fn main() -> i64 { 0 }";
    let (terminated, control) = spellings(source);
    same_exit_refusal(&terminated, &control);
}

fn guard(program: &mut ast::Program) -> &mut ast::Statement {
    let ast::ExprKind::Block { statements, .. } = &mut program.functions[0].body.kind else {
        panic!("function block")
    };
    let ast::Statement::While { body, .. } = &mut statements[1] else {
        panic!("authored loop")
    };
    let ast::ExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("loop block")
    };
    &mut statements[1]
}

#[test]
fn forged_or_stripped_markers_never_select_the_exit_route() {
    let (terminated, _) = spellings(SIMPLE);
    let original = parse(&terminated, PATH).unwrap();
    assert!(verify::verify(&original).is_empty());
    let mutations: [fn(&mut ast::Statement); 5] = [
        |statement| {
            if let ast::Statement::Let { mutable, .. } = statement {
                *mutable = true;
            }
        },
        |statement| {
            if let ast::Statement::Let { declared, .. } = statement {
                *declared = Some(ast::Type::I64);
            }
        },
        |statement| {
            if let ast::Statement::Let { value, .. } = statement {
                let inner = value.clone();
                value.kind = ast::ExprKind::Block {
                    statements: Vec::new(),
                    tail: Box::new(inner),
                };
            }
        },
        |statement| {
            if let ast::Statement::Let { syntax, .. } = statement {
                *syntax = ast::LetSyntax::ExpressionStatement;
            }
        },
        |statement| {
            if let ast::Statement::Let { syntax, .. } = statement {
                *syntax = ast::LetSyntax::Authored;
            }
        },
    ];
    for (index, mutate) in mutations.into_iter().enumerate() {
        let mut program = original.clone();
        mutate(guard(&mut program));
        let diagnostics = verify::verify(&program);
        assert!(
            diagnostics.iter().any(|d| d.code == "SPX-T312"),
            "mutation {index}: {diagnostics:?}"
        );
    }
    // A structurally shaped forged marker on an authored initializer whose
    // binding is read gains no authority: HIR authenticates the guard itself.
    let ast::Statement::Let { syntax: marker, .. } = guard(&mut original.clone()).clone() else {
        panic!("statement marker")
    };
    let mut reader = parse(
        "module exit.guard.forged; @id(\"main\") fn main()->i64 { let mut i=0; while i<3 { i=i+1; let g=if i==2 {continue;} else {0}; i=i+g; 0 } i }",
        PATH,
    )
    .unwrap();
    assert!(verify::verify(&reader)
        .iter()
        .any(|d| d.code == "SPX-T312"));
    if let ast::Statement::Let { syntax, .. } = guard(&mut reader) {
        *syntax = marker;
    }
    let refusal = match hir::resolve(&reader) {
        Err(errors) => errors.iter().map(|d| d.code).collect::<Vec<_>>(),
        Ok(resolved) => vec![hir::validate(&resolved).unwrap_err().code],
    };
    assert!(refusal.contains(&"SPX-H006"), "{refusal:?}");
}

fn retarget(expression: &mut hir::ResolvedExpr, site: &hir::ExpressionId) -> usize {
    use hir::{ResolvedExprKind as K, ResolvedStatement as S};
    match &mut expression.kind {
        K::LexicalExit {
            target: hir::ControlTarget::Loop { site: actual, .. },
            ..
        } if *actual != *site => {
            *actual = site.clone();
            1
        }
        K::Block { statements, tail } => {
            let mut changed = 0;
            for statement in statements.iter_mut() {
                changed += match statement {
                    S::Let { value, .. } | S::Assign { value, .. } => retarget(value, site),
                    S::LexicalWhile {
                        condition, body, ..
                    }
                    | S::While {
                        condition, body, ..
                    } => retarget(condition, site) + retarget(body, site),
                    _ => 0,
                };
            }
            changed + retarget(tail, site)
        }
        K::If {
            condition,
            then_branch,
            else_branch,
        } => {
            retarget(condition, site) + retarget(then_branch, site) + retarget(else_branch, site)
        }
        _ => 0,
    }
}

/// The innermost block tail, or the expression itself.
fn completion(expression: &mut hir::ResolvedExpr) -> &mut hir::ResolvedExpr {
    if !matches!(expression.kind, hir::ResolvedExprKind::Block { .. }) {
        return expression;
    }
    let hir::ResolvedExprKind::Block { tail, .. } = &mut expression.kind else {
        unreachable!()
    };
    completion(tail)
}

fn forge_completion(expression: &mut hir::ResolvedExpr) -> bool {
    use hir::{ResolvedExprKind as K, ResolvedStatement as S};
    let K::Block { statements, tail } = &mut expression.kind else {
        return false;
    };
    for statement in statements.iter_mut() {
        match statement {
            S::Let { value, .. } => {
                if let K::If {
                    then_branch,
                    else_branch,
                    ..
                } = &mut value.kind
                {
                    let exits = matches!(completion(then_branch).kind, K::LexicalExit { .. });
                    let zero = completion(else_branch);
                    if exits && matches!(zero.kind, K::Int(0)) {
                        zero.kind = K::Int(1);
                        return true;
                    }
                }
            }
            S::LexicalWhile { body, .. } | S::While { body, .. } => {
                if forge_completion(body) {
                    return true;
                }
            }
            _ => {}
        }
    }
    forge_completion(tail)
}

#[test]
fn hostile_hir_targets_and_guard_completions_are_independently_refused() {
    let (terminated, _) = spellings(NESTED);
    let program = check(&terminated, PATH).unwrap();
    let original = hir::resolve(&program).unwrap();
    hir::validate(&original).unwrap();
    // Retarget only the inner continue to the outer loop's exact site.
    let mut forged = original.clone();
    let function = forged
        .functions
        .iter_mut()
        .find(|function| function.name == "main")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("function block")
    };
    let outer = statements
        .iter()
        .find_map(|statement| match statement {
            hir::ResolvedStatement::LexicalWhile { condition, .. } => Some(condition.id.clone()),
            _ => None,
        })
        .expect("outer lexical loop");
    assert_eq!(retarget(&mut function.body, &outer), 1);
    assert_eq!(hir::validate(&forged).unwrap_err().code, "SPX-H006");
    // A reachable completion other than the scalar discard zero is no guard.
    let mut forged = original.clone();
    let function = forged
        .functions
        .iter_mut()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(forge_completion(&mut function.body));
    assert_eq!(hir::validate(&forged).unwrap_err().code, "SPX-H006");
}
