//! A scalar statement is an ordinary immutable let with independently checked syntax.
use semaprax::{ast, check, format, graph, hir, parse, verify};
const PATH: &str = "scalar-expression.spx";
fn source(body: &str) -> String {
    format!("module scalar.discard; @id(\"main\") fn main()->i64 {{ {body} }}")
}
fn errors(body: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    check(&source(body), PATH).unwrap_err()
}
#[test]
fn all_eight_copy_scalars_have_canonical_statement_spelling() {
    for value in ["1", "1i32", "1u8", "1usize", "'x'", "1.0f32", "1.0", "true"] {
        let text = source(&format!("{value}; 7"));
        let program = check(&text, PATH).unwrap();
        hir::validate(&hir::resolve(&program).unwrap()).unwrap();
        let canonical = format::canonical(&program);
        assert!(!canonical.contains("let _if"), "{canonical}");
        let round = check(&canonical, PATH).unwrap();
        assert_eq!(canonical, format::canonical(&round));
        assert_eq!(
            graph::to_json(&program).unwrap(),
            graph::to_json(&round).unwrap()
        );
    }
}
#[test]
fn names_skip_authored_identifiers_including_nested_and_later_readers() {
    let program = check(
        &source("let _if1=11; { 2; 3 }; let _if2=7; _if1+_if2"),
        PATH,
    )
    .unwrap();
    let canonical = format::canonical(&program);
    assert!(canonical.contains("let _if1 = 11;"));
    assert!(canonical.contains("let _if2 = 7;"));
    assert!(canonical.contains("2;"));
    assert!(!canonical.contains("let _if3"));
    let round = check(&canonical, PATH).unwrap();
    assert_eq!(
        graph::to_json(&program).unwrap(),
        graph::to_json(&round).unwrap()
    );
}
#[test]
fn owned_aggregate_function_and_borrowed_results_require_explicit_binding() {
    for body in [
        "\"text\"; 0",
        "bytes_zeroed(1usize); 0",
        "[1u8;2]; 0",
        "fn(x:i64)->i64{x}; 0",
        "let text=\"x\"; string_as_str(text); 0",
    ] {
        let diagnostics = errors(body);
        assert!(
            diagnostics.iter().any(|d| d.code == "SPX-T315"),
            "{body}: {diagnostics:?}"
        );
    }
    // Explicit locals preserve the original ownership route.
    assert!(check(&source("let text=\"text\"; string_len(text)"), PATH).is_ok());
}
#[test]
fn final_tail_underscore_and_initializer_exit_priorities_stay_closed() {
    assert_eq!(parse(&source("1;"), PATH).unwrap_err().code, "SPX-P106");
    assert!(
        errors("let _=1; let _=2; 0")
            .iter()
            .any(|d| d.code == "SPX-T209")
    );
    let diagnostics = errors("let mut i=0; while i<1 { {continue;}; i=i+1; 0 } i");
    assert!(
        diagnostics.iter().any(|d| d.code == "SPX-T312"),
        "{diagnostics:?}"
    );
    // Operand diagnostics precede discard typing; no marker can hide an unknown input.
    assert!(errors("missing; 0").iter().any(|d| d.code == "SPX-T208"));
}
#[test]
fn forged_marker_never_hides_a_reader_mutability_annotation_or_duplicate() {
    let mut program = parse(&source("1; 7"), PATH).unwrap();
    let ast::ExprKind::Block { statements, tail } = &mut program.functions[0].body.kind else {
        unreachable!()
    };
    let ast::Statement::Let { name, value, .. } = &statements[0] else {
        unreachable!()
    };
    tail.kind = ast::ExprKind::Var(name.clone());
    let name = name.clone();
    let value_span = value.span;
    let canonical = format::canonical(&program);
    assert!(
        canonical.contains(&format!("let {name} = 1;")),
        "{canonical}"
    );
    assert!(check(&canonical, PATH).is_ok());
    let ast::ExprKind::Block { statements, tail } = &mut program.functions[0].body.kind else {
        unreachable!()
    };
    tail.kind = ast::ExprKind::Int(7);
    if let ast::Statement::Let { mutable, .. } = &mut statements[0] {
        *mutable = true;
    }
    assert!(
        verify::verify(&program)
            .iter()
            .any(|d| d.code == "SPX-T315")
    );
    assert!(format::canonical(&program).contains("let mut"));
    if let ast::ExprKind::Block { statements, .. } = &mut program.functions[0].body.kind {
        if let ast::Statement::Let {
            mutable, declared, ..
        } = &mut statements[0]
        {
            *mutable = false;
            *declared = Some(ast::Type::I64);
        }
    }
    assert!(
        verify::verify(&program)
            .iter()
            .any(|d| d.code == "SPX-T315")
    );
    assert!(format::canonical(&program).contains(": i64"));
    if let ast::ExprKind::Block { statements, .. } = &mut program.functions[0].body.kind {
        if let ast::Statement::Let {
            declared,
            name_span,
            ..
        } = &mut statements[0]
        {
            *declared = None;
            *name_span = value_span;
        }
        let duplicate = statements[0].clone();
        statements.push(duplicate);
    }
    assert!(
        verify::verify(&program)
            .iter()
            .any(|d| d.code == "SPX-T209")
    );
    assert!(format::canonical(&program).contains(&format!("let {name}")));
}
#[test]
fn normalizer_preserves_lazy_failure_and_consumed_arguments_across_backends() {
    let text = "module scalar.calls; @id(\"consume\") fn consume(text:string)->i64 { string_len(text) } @id(\"main\") fn main()->i64 { consume(\"abc\"); false && (1/0==0); 19 }";
    let program = check(text, PATH).unwrap();
    let lowered = "module scalar.calls; @id(\"consume\") fn consume(text:string)->i64 { string_len(text) } @id(\"main\") fn main()->i64 { let _if1=consume(\"abc\"); let _if2=false && (1/0==0); 19 }";
    let ordinary = check(lowered, PATH).unwrap();
    assert_eq!(
        graph::to_json(&program).unwrap(),
        graph::to_json(&ordinary).unwrap()
    );
    assert_eq!(super::run(text, false), "19");
    if super::command_available("clang") {
        assert_eq!(super::run(text, true), "19");
    }
    // A completed discarded call still selects its original failure and never publishes the later tail.
    let failing = "module scalar.failure; @id(\"fail\") fn fail(text:string)->i64 { let n=string_len(text); n/(n-3) } @id(\"main\") fn main()->i64 { fail(\"abc\"); 19 }";
    let ordinary_failure = "module scalar.failure; @id(\"fail\") fn fail(text:string)->i64 { let n=string_len(text); n/(n-3) } @id(\"main\") fn main()->i64 { let _if1=fail(\"abc\"); 19 }";
    let failure_program = check(failing, PATH).unwrap();
    let ordinary_program = check(ordinary_failure, PATH).unwrap();
    assert_eq!(
        graph::to_json(&failure_program).unwrap(),
        graph::to_json(&ordinary_program).unwrap()
    );
    for native in [false, true] {
        if native && !super::command_available("clang") {
            continue;
        }
        let mut outputs = Vec::new();
        for text in [failing, ordinary_failure] {
            let path = super::write_source("discard-failure", text);
            let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_semaprax"));
            command.arg("run").arg(&path);
            if native {
                command.arg("--native");
            }
            let output = command.output().unwrap();
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            assert!(!output.stderr.is_empty());
            outputs.push(output.status.code());
            std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
        }
        assert_eq!(
            outputs[0], outputs[1],
            "discard must preserve the selected checked failure"
        );
    }
    let scalar = source("false && (1/0==0); 19");
    let ordinary_scalar = source("let _if1=false && (1/0==0); 19");
    assert_eq!(
        semaprax::wasm::emit_module(&check(&scalar, PATH).unwrap()).unwrap(),
        semaprax::wasm::emit_module(&check(&ordinary_scalar, PATH).unwrap()).unwrap()
    );
}
