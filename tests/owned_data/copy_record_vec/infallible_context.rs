//! Every admitted carrier keeps total Vec operations lane-free in a real
//! lexical function. The saved full HIR checker remains the hostile oracle.
use semaprax::cleanup_plan::{
    ExitContinuation, StatusLane, StatusProducer, StatusSource, StatusSourceId,
};
use semaprax::hir::{DeclarationId, ExpressionId, ResolvedExpr, ResolvedExprKind, ResolvedProgram};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);

fn source(declarations: &str, body: &str) -> String {
    format!(
        r#"module app.infallible;
{declarations}
@id("app.probe") fn probe()->i64 {{
 {body}
 let mut again=true;
 while again {{ again=false; if good {{return 42;}}; 0 }}
 1
}}
@id("app.main") fn main()->i64 {{probe()}}
"#
    )
}
fn cases() -> Vec<(&'static str, String)> {
    vec![
        (
            "copy-record",
            source(
                r#"@id("app.copy") record CopyRow {@id("app.copy.key") key:i64,@id("app.copy.tie") tie:i64,}"#,
                r#"
 let a=vec_with_capacity<CopyRow>(2usize);
 let b=vec_push<CopyRow>(a,CopyRow{key:7,tie:0});
 let c=vec_push<CopyRow>(b,CopyRow{key:3,tie:1});
 let d=vec_sort<CopyRow>(c);
 let first=vec_get<CopyRow>(d,0usize);
 let e=vec_sort_by<CopyRow>(d,"key:desc");
 let second=vec_get<CopyRow>(e,0usize);
 let f=vec_clear<CopyRow>(e);
 let good=first.key==3 && second.key==7 && vec_len<CopyRow>(f)==0usize && vec_capacity<CopyRow>(f)==2usize;
"#,
            ),
        ),
        (
            "string",
            source(
                "",
                r#"
 let a=vec_with_capacity<string>(2usize);
 let b=vec_push<string>(a,"z"); let c=vec_push<string>(b,"a");
 let d=vec_sort_owned<string>(c); let first=vec_clone_at<string>(d,0usize);
 let e=vec_clear<string>(d);
 let good=first=="a" && vec_len<string>(e)==0usize && vec_capacity<string>(e)==2usize;
"#,
            ),
        ),
        (
            "bytes",
            source(
                "",
                r#"
 let a=vec_with_capacity<Bytes>(2usize);
 let b=vec_push<Bytes>(a,bytes_zeroed(1usize));
 let c=vec_push<Bytes>(b,bytes_zeroed(2usize));
 let before=vec_len<Bytes>(c)==2usize && vec_capacity<Bytes>(c)==2usize;
 let d=vec_clear<Bytes>(c);
 let good=before && vec_len<Bytes>(d)==0usize && vec_capacity<Bytes>(d)==2usize;
"#,
            ),
        ),
        (
            "legacy-owned-record",
            source(
                r#"@id("app.legacy") record Legacy {@id("app.legacy.a") a:Bytes,@id("app.legacy.b") b:Bytes,@id("app.legacy.key") key:i64,}"#,
                r#"
 let a=vec_with_capacity<Legacy>(1usize);
 let b=vec_push<Legacy>(a,Legacy{a:bytes_zeroed(1usize),b:bytes_zeroed(2usize),key:7});
 let before=vec_len<Legacy>(b)==1usize && vec_capacity<Legacy>(b)==1usize;
 let c=vec_clear<Legacy>(b);
 let good=before && vec_len<Legacy>(c)==0usize && vec_capacity<Legacy>(c)==1usize;
"#,
            ),
        ),
        (
            "owned-leaf-record",
            source(
                r#"@id("app.row") record Row {@id("app.row.key") key:i64,@id("app.row.title") title:string,@id("app.row.data") data:Bytes,}"#,
                r#"
 let a=vec_with_capacity<Row>(2usize);
 let b=vec_push<Row>(a,Row{key:7,title:"z",data:bytes_zeroed(1usize)});
 let c=vec_push<Row>(b,Row{key:3,title:"a",data:bytes_zeroed(2usize)});
 let d=vec_sort_owned<Row>(c); let first=vec_field<Row>(d,0usize,"key");
 let e=vec_sort_by<Row>(d,"key:desc"); let second=vec_field<Row>(e,0usize,"key");
 let f=vec_clear<Row>(e);
 let good=first==3 && second==7 && vec_len<Row>(f)==0usize && vec_capacity<Row>(f)==2usize;
"#,
            ),
        ),
    ]
}
fn program(text: &str) -> (semaprax::ast::Program, ResolvedProgram) {
    let ast = semaprax::check(text, "infallible-carriers.spx").unwrap();
    let program = semaprax::hir::resolve(&ast).unwrap();
    semaprax::hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.probe")
        .unwrap();
    assert!(
        function.cleanup_plan.lexical_control.is_some(),
        "fixture must exercise the actual lexical receiver"
    );
    (ast, program)
}
fn total_calls(expression: &ResolvedExpr, out: &mut Vec<(ExpressionId, DeclarationId)>) {
    if let ResolvedExprKind::Call { callee, .. } = &expression.kind {
        // Independent census uses public HIR identities, not the private classifier.
        if matches!(
            callee.as_str(),
            "core.vec.len"
                | "core.vec.capacity"
                | "core.vec.clear"
                | "core.vec.sort"
                | "core.vec.sort-owned"
        ) || callee.as_str().starts_with("core.vec.sort-by.v1:")
        {
            out.push((expression.id.clone(), callee.clone()));
        }
    }
    for index in 0..expression.child_count() {
        total_calls(expression.child(index).unwrap(), out);
    }
}
fn refuse(program: &ResolvedProgram, label: &str) {
    let independent = semaprax::hir::validate(program).expect_err(label);
    assert_eq!(independent.code, "SPX-H006", "{label}: {independent:?}");
    assert_eq!(
        semaprax::codegen::emit_hir_c(program).expect_err(label),
        independent,
        "{label}"
    );
}

#[test]
fn lexical_total_vec_carriers_execute_and_settle_on_all_engines() {
    for (label, text) in cases() {
        let (ast, program) = program(&text);
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.probe")
            .unwrap();
        let mut calls = Vec::new();
        total_calls(&function.body, &mut calls);
        assert_eq!(
            calls.len(),
            if label == "string" { 4 } else { 5 },
            "{label}: independent complete total-operation census"
        );
        for (id, _) in calls {
            assert!(
                !function
                    .cleanup_plan
                    .status_sources
                    .iter()
                    .any(|source| source.id.expression == id),
                "{label}: total operation has no lane"
            );
        }
        let generated = semaprax::codegen::emit_hir_c(&program).unwrap();
        assert!(
            generated.contains("plan-infallible Vec operation returned failure"),
            "{label}"
        );
        if label == "copy-record" {
            super::runtime::run("lexical-infallible-copy", &text, 0, 42, false);
            continue;
        }
        let root = std::env::temp_dir().join(format!(
            "infallible-vec-{}-{}-{label}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("case.spx");
        std::fs::write(&path, &text).unwrap();
        for _ in 0..2 {
            let result =
                semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default())
                    .unwrap();
            let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            assert!(result.returned, "{label}: {}", result.envelope);
            assert_eq!(envelope["payload"]["outcome"]["value"], "42");
        }
        crate::owned_collection_outcome::run_native_generated(generated, &root);
        crate::owned_collection_outcome::run_strict_wasm(&ast, &root);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn total_vec_carriers_refuse_forged_status_and_wrong_failure_context() {
    for (label, text) in cases() {
        let (_, base) = program(&text);
        let index = base
            .functions
            .iter()
            .position(|f| f.id.as_str() == "app.probe")
            .unwrap();
        let mut calls = Vec::new();
        total_calls(&base.functions[index].body, &mut calls);
        for (id, callee) in calls {
            let mut forged = base.clone();
            forged.functions[index]
                .cleanup_plan
                .status_sources
                .push(StatusSource {
                    id: StatusSourceId {
                        expression: id,
                        lane: StatusLane::OperationFailure,
                    },
                    producer: StatusProducer::PropagatedCall { callee },
                });
            refuse(&forged, label);
        }
        let failures:Vec<_>=base.functions[index].cleanup_plan.exits.iter().enumerate().filter_map(|(i,exit)|matches!(exit.continuation,ExitContinuation::ReturnFailure{ref source} if source.lane==StatusLane::OperationFailure).then_some(i)).collect();
        assert!(
            !failures.is_empty(),
            "{label}: real fallible producers retained"
        );
        for exit_index in failures {
            let mut missing = base.clone();
            let ExitContinuation::ReturnFailure { source } =
                &mut missing.functions[index].cleanup_plan.exits[exit_index].continuation
            else {
                unreachable!()
            };
            source.lane = StatusLane::ContractFalse;
            refuse(&missing, label);
            let mut wrong = base.clone();
            let plan = &mut wrong.functions[index].cleanup_plan;
            plan.exits[exit_index].from = plan.lexical_control.as_ref().unwrap().resumes[0].entry;
            refuse(&wrong, label);
            let mut foreign = base.clone();
            let other = foreign
                .functions
                .iter()
                .find(|f| f.id.as_str() == "app.main")
                .unwrap()
                .cleanup_plan
                .status_sources
                .iter()
                .find(|source| source.id.lane == StatusLane::OperationFailure)
                .unwrap()
                .id
                .clone();
            foreign.functions[index].cleanup_plan.exits[exit_index].continuation =
                ExitContinuation::ReturnFailure { source: other };
            refuse(&foreign, label);
        }
        assert!(
            semaprax::codegen::emit_hir_c(&base).is_ok(),
            "{label}: untouched authority"
        );
    }
}
