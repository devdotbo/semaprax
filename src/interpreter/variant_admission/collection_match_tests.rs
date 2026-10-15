//! Exact collection-outcome joins share HIR authority, not nominal spelling.
use super::*;

const SOURCE: &str = r#"module collection.join;
@id("copy.row") record CopyRow { @id("copy.row.n") n:i64, }
@id("stage") variant Stage {
    @id("stage.ready") Ready { @id("stage.rows") rows:Vec<CopyRow>, },
    @id("stage.error") Error { @id("stage.code") code:i64, @id("stage.offset") offset:usize, @id("stage.field") field:i64, },
}
@id("out") variant Outcome {
    @id("out.ready") Ready { @id("out.words") words:Vec<string>, },
    @id("out.error") Error { @id("out.code") code:i64, @id("out.offset") offset:usize, @id("out.field") field:i64, },
}
@id("convert") fn convert(stage:own Stage)->Outcome {
    match own stage {
        Stage::Ready{rows}=>Outcome::Ready{words:vec_push<string>(vec_with_capacity<string>(1usize),"aé\u{0}")},
        Stage::Error{code,offset,field}=>Outcome::Error{code:code,offset:offset,field:field},
    }
}
@id("app.main") fn main()->i64 {
    let stage=Stage::Ready{rows:vec_push<CopyRow>(vec_with_capacity<CopyRow>(1usize),CopyRow{n:9})};
    let outcome=convert(stage);
    match own outcome {
        Outcome::Ready{words}=>{let first=vec_clone_at<string>(words,0usize);if vec_len<string>(words)==1usize && string_len(first)==4 {42}else{0}},
        Outcome::Error{code,offset,field}=>if offset==5usize && field==7 {code}else{0},
    }
}
"#;

fn program(source: &str) -> hir::ResolvedProgram {
    let ast = crate::check(source, "collection-join.spx").unwrap();
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    program
}

fn expression(program: &hir::ResolvedProgram) -> &hir::ResolvedExpr {
    let body = &program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "convert")
        .unwrap()
        .body;
    let ResolvedExprKind::Block { tail, .. } = &body.kind else {
        panic!("function block")
    };
    tail
}

fn reject_forged_hir(program: &hir::ResolvedProgram, expression: hir::ResolvedExpr) {
    let mut forged = program.clone();
    let body = &mut forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "convert")
        .unwrap()
        .body;
    let ResolvedExprKind::Block { tail, .. } = &mut body.kind else {
        unreachable!()
    };
    **tail = expression;
    assert_eq!(hir::validate(&forged).unwrap_err().code, "SPX-H006");
}

fn admitted(program: &hir::ResolvedProgram, expression: &hir::ResolvedExpr) -> bool {
    let ResolvedExprKind::Match {
        mode,
        scrutinee,
        arms,
    } = &expression.kind
    else {
        return false;
    };
    hir::collection_outcome::match_join(&program.declarations, expression)
        && super::super::variant_pattern_is_admitted(
            &program.declarations,
            *mode,
            &scrutinee.ty,
            arms,
        )
}

#[test]
fn exact_owned_collection_join_executes_utf8_nul_and_scalar_error_on_reentry() {
    use crate::interpreter::resolved_case::{
        evaluate_resolved_profile_i64_entry, ResolvedFunctionProfile,
    };
    for source in [
        SOURCE.to_owned(),
        SOURCE.replace(
            "Stage::Ready{rows:vec_push<CopyRow>(vec_with_capacity<CopyRow>(1usize),CopyRow{n:9})}",
            "Stage::Error{code:42,offset:5usize,field:7}",
        ),
    ] {
        let program = program(&source);
        assert!(admitted(&program, expression(&program)));
        for _ in 0..4 {
            let result = evaluate_resolved_profile_i64_entry(
                &program,
                "app.main",
                100_000,
                ResolvedFunctionProfile::StreamOwned,
            )
            .unwrap();
            assert!(matches!(
                result.outcome,
                ResolvedEvaluationOutcome::ReturnedI64(42)
            ));
        }
    }
}

#[test]
fn collection_join_keeps_exact_result_and_pattern_ownership_authority() {
    let program = program(SOURCE);
    let original = expression(&program);
    for result_mode in [hir::OwnershipMode::Value, hir::OwnershipMode::Borrow] {
        let mut forged = original.clone();
        forged.ownership = result_mode;
        assert!(!admitted(&program, &forged));
        reject_forged_hir(&program, forged);
    }
    for mutation in 0..6 {
        let mut forged = original.clone();
        let ResolvedExprKind::Match { mode, arms, .. } = &mut forged.kind else {
            unreachable!()
        };
        if mutation == 0 {
            *mode = hir::ResolvedMatchMode::Value;
        }
        let hir::ResolvedMatchPattern::Variant {
            variant,
            case,
            fields,
        } = &mut arms[0].pattern
        else {
            unreachable!()
        };
        match mutation {
            1 => *variant = hir::DeclarationId::new("foreign.stage"),
            2 => *case = hir::DeclarationId::new("out.ready"),
            3 => fields[0].binding.ownership = hir::OwnershipMode::Borrow,
            4 => fields[0].field = hir::DeclarationId::new("out.words"),
            5 => fields[0].binding.ty = ResolvedType::String,
            _ => {}
        }
        assert!(!admitted(&program, &forged));
        reject_forged_hir(&program, forged);
    }
    let mut forged = original.clone();
    forged.ty = ResolvedType::String;
    assert!(!admitted(&program, &forged));
    reject_forged_hir(&program, forged);
}
