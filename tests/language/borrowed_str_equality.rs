//! Borrowed Str equality compares exact UTF-8 bytes without owner clones.
use super::byte_widening::available;
use super::owned_string_loops_v1::support::Fixture;
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, format, graph, hir, parse, verify, wasm};
use std::path::Path;
use std::process::Command;

const SOURCE: &str = r#"module test.str_equal;
@id("text.same") fn same(a:borrow str,b:borrow str)->bool {a==b}
@id("text.different") fn different(a:borrow str,b:borrow str)->bool {a!=b}
@id("app.main") fn main()->i64 {
    let a="é\u{0}x"; let b="é\u{0}x"; let c="é\u{0}y";
    let empty=""; let empty2=""; let prefix="é"; let decomposed="e\u{301}\u{0}x";
    let av=string_as_str(a); let bv=string_as_str(b); let cv=string_as_str(c);
    let ev=string_as_str(empty); let e2v=string_as_str(empty2);
    let pv=string_as_str(prefix); let dv=string_as_str(decomposed);
    let skipped=false && same(av,cv);
    if same(av,bv) && different(av,cv) && different(av,pv) && same(ev,e2v) && same(av,av) && different(av,dv) && !skipped {42}else{-1}
}
"#;
const STATIC: &str = r#"module test.static_equal;
@id("app.main") fn main()->i64 {
 let a=str"é\u{0}x"; let b=str"é\u{0}x"; let c=str"é\u{0}y";
 if a==b && a!=c {42}else{-1}
}
"#;
fn failure_source() -> String {
    SOURCE.replace("fn main()->i64 {", "fn main()->i64 ensures false {")
}

#[test]
fn borrowed_str_equality_source_graph_and_hostile_modes() {
    let ast = parse(SOURCE, Path::new("str-equality.spx")).unwrap();
    assert!(verify::verify(&ast).is_empty());
    let canonical = format::canonical(&ast);
    let round = parse(&canonical, Path::new("canonical.spx")).unwrap();
    assert_eq!(
        graph::to_json(&ast).unwrap(),
        graph::to_json(&round).unwrap()
    );
    let json = graph::to_json(&ast).unwrap();
    graph::verify_json(&ast, &json).unwrap();
    let altered = json.replacen("\"op\":\"==\"", "\"op\":\"<\"", 1);
    assert_ne!(altered, json);
    assert!(graph::verify_json(&ast, &altered).is_err());
    let baseline = hir::resolve(&ast).unwrap();
    hir::validate(&baseline).unwrap();
    let foreign = baseline
        .functions
        .iter()
        .find(|f| f.id.as_str() == "text.different")
        .unwrap()
        .params[0]
        .id
        .clone();
    for mutation in 0..4 {
        let mut program = baseline.clone();
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "text.same")
            .unwrap();
        let hir::ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("block")
        };
        let hir::ResolvedExprKind::Binary { op, left, right } = &mut tail.kind else {
            panic!("binary")
        };
        match mutation {
            0 => *op = semaprax::ast::BinaryOp::Lt,
            1 => right.ty = hir::ResolvedType::String,
            2 => left.ownership = hir::OwnershipMode::Own,
            _ => {
                let hir::ResolvedExprKind::Place(place) = &mut left.kind else {
                    panic!("place")
                };
                place.root = foreign.clone();
            }
        }
        assert_eq!(
            hir::validate(&program).unwrap_err().code,
            "SPX-H006",
            "mutation {mutation}"
        );
    }
    for (body, code) in [("a<b", "SPX-T208"), ("a==\"owned\"", "SPX-T207")] {
        let source=format!("module bad; @id(\"bad.same\") fn same(a:borrow str,b:borrow str)->bool {{{body}}} @id(\"app.main\") fn main()->i64 {{0}}");
        let ast = parse(&source, Path::new("invalid.spx")).unwrap();
        assert!(verify::verify(&ast).iter().any(|d| d.code == code));
        assert!(hir::resolve(&ast).is_err());
    }
    // A live borrowed view continues to prohibit consuming its actual owner.
    let invalid = SOURCE.replace("let a=", "let mut a=").replace(
        "let skipped=false",
        "a=string_concat(a,\"x\"); let skipped=false",
    );
    let ast = parse(&invalid, Path::new("live-loan.spx")).unwrap();
    assert!(!verify::verify(&ast).is_empty());
    assert!(hir::resolve(&ast).is_err());
}

#[test]
fn borrowed_str_equality_interpreter_native_bytes_and_late_failure() {
    for (source, failed, allocations) in [
        (SOURCE.to_owned(), false, 7),
        (failure_source(), true, 7),
        (STATIC.to_owned(), false, 0),
    ] {
        let mut fixture = Fixture::new(&source);
        for _ in 0..3 {
            let result = interpreter::interpret(
                &fixture.source,
                "app.main",
                &[],
                &InterpreterOptions::default(),
            )
            .unwrap();
            let value: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            let outcome = &value["payload"]["outcome"];
            if failed {
                assert_eq!(outcome["status"]["domain_id"], "semaprax.contract.v1");
                assert_eq!(outcome["status"]["code"], 2);
            } else {
                assert_eq!(outcome["kind"], "returned");
                assert_eq!(outcome["value"], "42");
            }
        }
        if available("clang") {
            let ast = parse(&source, Path::new("native.spx")).unwrap();
            let generated = codegen::emit_c(&ast).unwrap();
            let assertion = if failed {
                "REQUIRE(token!=0 && value==INT64_MIN); const struct spx_normalized_status *status=spx_status_resolve(&context,token); REQUIRE(status && strcmp(status->domain_id,\"semaprax.contract.v1\")==0 && status->code==2);"
            } else {
                "REQUIRE(token==0 && value==42);"
            };
            let probe=format!("{}\n{}\n{generated}\n#undef malloc\n#undef free\nint main(void) {{ REQUIRE(fixture_binary_stdout()); struct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL)); for(int i=0;i<3;i++) {{size_t before=fixture_allocations; int64_t value=INT64_MIN; spx_status_token token=spx_decl_6170702e6d61696e(&context,&value); {assertion} REQUIRE(fixture_allocations-before=={allocations}); REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);}} (void)fixture_malloc;(void)fixture_free;return 0;}}\n",include_str!("../support/native_fixture_stdio.c"),include_str!("../native_owned_utf8_settlement_v1/allocations.c"));
            for optimization in ["-O0", "-O2"] {
                assert_eq!(fixture.native(&probe, optimization), "");
            }
        }
        fixture.cleanup();
    }
}

#[test]
fn borrowed_str_equality_aggregate_wasm_authenticates_tagged_owners() {
    if !available("node") {
        return;
    }
    for (source, failed, allocations) in [
        (SOURCE.to_owned(), false, 7),
        (failure_source(), true, 7),
        (STATIC.to_owned(), false, 0),
    ] {
        let ast = parse(&source, Path::new("wasm.spx")).unwrap();
        let bytes = wasm::emit_module(&ast).unwrap();
        assert_eq!(bytes, wasm::emit_module(&ast).unwrap());
        let mut fixture = Fixture::new(&source);
        fixture.write("program.wasm", bytes);
        let script = fixture.write(
            "probe.mjs",
            include_str!("borrowed_str_equality/aggregate.mjs"),
        );
        let output = Command::new("node")
            .current_dir(&fixture.root)
            .arg(script)
            .arg(if failed { "failure" } else { "success" })
            .arg(allocations.to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fixture.cleanup();
    }
}

#[test]
fn borrowed_str_equality_public_text_validates_both_roots_before_entry() {
    const PUBLIC:&str="module public.text; @id(\"text.same\") fn same(a:borrow str,b:borrow str)->bool {a==b} @id(\"text.different\") fn different(a:borrow str,b:borrow str)->bool {a!=b} @id(\"app.main\") fn main()->i64 {0}";
    let ast = parse(PUBLIC, Path::new("public.spx")).unwrap();
    let ids = ["text.same".to_owned(), "text.different".to_owned()];
    let bytes = wasm::emit_module_with_text_exports(&ast, &ids).unwrap();
    assert_eq!(
        bytes,
        wasm::emit_module_with_text_exports(&ast, &ids).unwrap()
    );
    if !available("node") {
        return;
    }
    let mut fixture = Fixture::new(PUBLIC);
    fixture.write("program.wasm", bytes);
    let script = fixture.write(
        "probe.mjs",
        include_str!("borrowed_str_equality/public.mjs"),
    );
    let output = Command::new("node")
        .current_dir(&fixture.root)
        .arg(script)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fixture.cleanup();
}
