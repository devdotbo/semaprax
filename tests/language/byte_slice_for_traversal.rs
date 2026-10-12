//! Immutable borrowed `Slice<u8>` traversal: canonical source, the exact
//! receiver lowering, its independently replayed latch and census, the
//! synthesized identity prebound, and stable source refusals.
use std::path::Path;

use semaprax::ast::BinaryOp;
use semaprax::hir::{self, ResolvedExpr, ResolvedExprKind, ResolvedStatement, ResolvedType};
use semaprax::{format, graph, parse, verify};

const SOURCE: &str = include_str!("../fixtures/byte-slice-traversal.spx");

const REFUSAL: &str = r#"module app.byte_traversal_refusal;

@id("app.refusal.main")
fn main() -> i64 {
    let data = [1u8, 2u8];
    let owned = bytes_copy(array_as_slice(data));
    let view = bytes_as_slice(owned);
    let mut total = 0;
    for byte in view { let first = byte; total = total + 1; 0 }
    total
}
"#;

fn parsed(source: &str) -> semaprax::ast::Program {
    parse(source, Path::new("byte-slice-traversal.spx")).unwrap()
}

fn checked() -> hir::ResolvedProgram {
    let program = parsed(SOURCE);
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    resolved
}

fn wrapper<'a>(program: &'a hir::ResolvedProgram, name: &str) -> &'a ResolvedExpr {
    let function = program
        .functions
        .iter()
        .find(|function| function.name == name)
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("function block")
    };
    statements
        .iter()
        .find_map(|statement| match statement {
            ResolvedStatement::Let { binding, value, .. } if binding.name == "#for-bytes" => {
                Some(value)
            }
            _ => None,
        })
        .expect("byte traversal wrapper")
}

fn wrapper_statements<'a>(
    program: &'a mut hir::ResolvedProgram,
    name: &str,
) -> &'a mut Vec<ResolvedStatement> {
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.name == name)
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("function block")
    };
    let value = statements
        .iter_mut()
        .find_map(|statement| match statement {
            ResolvedStatement::Let { binding, value, .. } if binding.name == "#for-bytes" => {
                Some(value)
            }
            _ => None,
        })
        .expect("byte traversal wrapper");
    let ResolvedExprKind::Block { statements, .. } = &mut value.kind else {
        panic!("wrapper block")
    };
    statements
}

fn loop_steps(
    statements: &mut [ResolvedStatement],
) -> (&mut Vec<ResolvedStatement>, Option<&mut hir::LoopLatchKind>) {
    let (body, latch) = match &mut statements[2] {
        ResolvedStatement::While { body, .. } => (body, None),
        ResolvedStatement::LexicalWhile { body, latch, .. } => (body, Some(latch)),
        _ => panic!("traversal loop"),
    };
    let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("loop body")
    };
    (statements, latch)
}

fn place_root(expression: &ResolvedExpr) -> &hir::ValueId {
    let ResolvedExprKind::Place(place) = &expression.kind else {
        panic!("bare place")
    };
    assert!(place.projections.is_empty());
    &place.root
}

/// Every synthesized node of one lowered wrapper, excluding the authored body.
fn synthesized(expression: &ResolvedExpr, authored: &hir::ExpressionId) -> usize {
    if &expression.id == authored {
        return 0;
    }
    1 + match &expression.kind {
        ResolvedExprKind::Block { statements, tail } => {
            statements
                .iter()
                .map(|statement| match statement {
                    ResolvedStatement::Let { value, .. }
                    | ResolvedStatement::Assign { value, .. } => synthesized(value, authored),
                    ResolvedStatement::While {
                        condition, body, ..
                    }
                    | ResolvedStatement::LexicalWhile {
                        condition, body, ..
                    } => synthesized(condition, authored) + synthesized(body, authored),
                    ResolvedStatement::Unsafe { .. } => panic!("unsafe in lowering"),
                })
                .sum::<usize>()
                + synthesized(tail, authored)
        }
        ResolvedExprKind::Call { args, .. } => {
            args.iter().map(|arg| synthesized(arg, authored)).sum()
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            synthesized(left, authored) + synthesized(right, authored)
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            synthesized(scrutinee, authored)
                + arms
                    .iter()
                    .map(|arm| synthesized(&arm.value, authored))
                    .sum::<usize>()
        }
        ResolvedExprKind::Place(_) | ResolvedExprKind::Usize(_) => 0,
        other => panic!("unexpected synthesized node {other:?}"),
    }
}

#[test]
fn byte_for_preserves_source_and_lowers_to_one_authenticated_receiver() {
    let program = parsed(SOURCE);
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert_eq!(canonical.matches("for byte in view").count(), 4);
    assert_eq!(canonical.matches("for byte in sub").count(), 1);
    assert_eq!(format::canonical(&parsed(&canonical)), canonical);

    let resolved = checked();
    let ResolvedExprKind::Block { statements, tail } = &wrapper(&resolved, "sum").kind else {
        panic!("wrapper block")
    };
    assert!(matches!(tail.kind, ResolvedExprKind::Usize(0)));
    let [ResolvedStatement::Let {
        binding: length,
        mutable: false,
        value: count,
        ..
    }, ResolvedStatement::Let {
        binding: counter,
        mutable: true,
        value: seed,
        ..
    }, ResolvedStatement::While {
        condition, body, ..
    }] = statements.as_slice()
    else {
        panic!("snapshot, counter and exit-free loop")
    };
    assert_eq!(length.name, "#for-bytes-length");
    assert_eq!(counter.name, "#for-index");
    assert!(matches!(seed.kind, ResolvedExprKind::Usize(0)));
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &count.kind
    else {
        panic!("length snapshot call")
    };
    assert_eq!(callee.as_str(), "core.bytes.len");
    assert!(type_arguments.is_empty());
    let [view] = args.as_slice() else {
        panic!("one view")
    };
    assert_eq!(view.ty, ResolvedType::SliceU8);
    let view_root = place_root(view);
    let ResolvedExprKind::Binary {
        op: BinaryOp::Lt,
        left,
        right,
    } = &condition.kind
    else {
        panic!("ascending bound")
    };
    assert_eq!(place_root(left), &counter.id);
    assert_eq!(place_root(right), &length.id);

    let ResolvedExprKind::Block { statements, .. } = &body.kind else {
        panic!("loop body")
    };
    let [ResolvedStatement::Let {
        binding: item,
        mutable: false,
        value: read,
        ..
    }, ResolvedStatement::Let {
        binding: discard,
        mutable: false,
        value: authored,
        ..
    }, ResolvedStatement::Assign {
        binding: advanced,
        field: None,
        value: advance,
        ..
    }] = statements.as_slice()
    else {
        panic!("item, discarded body and unit step")
    };
    assert_eq!(item.name, "byte");
    assert_eq!(item.ty, ResolvedType::U8);
    assert_eq!(discard.name, "#for-body");
    assert_eq!(advanced.id, counter.id);
    assert!(
        matches!(&advance.kind, ResolvedExprKind::Binary { op: BinaryOp::Add, right, .. }
        if matches!(right.kind, ResolvedExprKind::Usize(1)))
    );

    // The receiver is the existing Option<u8> result with exactly one arm.
    let ResolvedExprKind::Match {
        mode: hir::ResolvedMatchMode::Value,
        scrutinee,
        arms,
    } = &read.kind
    else {
        panic!("byte receiver")
    };
    assert_eq!(read.ty, ResolvedType::U8);
    let ResolvedExprKind::Call { callee, args, .. } = &scrutinee.kind else {
        panic!("byte_get")
    };
    assert_eq!(callee.as_str(), "core.bytes.get");
    assert_eq!(place_root(&args[0]), view_root);
    assert_eq!(place_root(&args[1]), &counter.id);
    let [arm] = arms.as_slice() else {
        panic!("only Some selects")
    };
    assert!(arm.guard.is_none());
    let hir::ResolvedMatchPattern::Variant { case, fields, .. } = &arm.pattern else {
        panic!("Some pattern")
    };
    assert_eq!(case.as_str(), "core.option.some");
    assert_eq!(place_root(&arm.value), &fields[0].binding.id);

    // No buffer, copy or view is materialized by the lowering itself. The
    // receiver adds exactly two identities to the Vec form's sixteen; the
    // workspace prebound keeps sixteen and the byte lowering debits the rest.
    assert_eq!(synthesized(wrapper(&resolved, "sum"), &authored.id), 18);

    // Lexical exits keep the latch on the exact unit step.
    let ResolvedExprKind::Block { statements, .. } = &wrapper(&resolved, "exits").kind else {
        panic!("wrapper block")
    };
    let ResolvedStatement::LexicalWhile { body, latch, .. } = &statements[2] else {
        panic!("lexical traversal loop")
    };
    let ResolvedExprKind::Block { statements, .. } = &body.kind else {
        panic!("loop body")
    };
    let ResolvedStatement::Assign { value: step, .. } = &statements[2] else {
        panic!("unit step")
    };
    assert_eq!(
        latch,
        &hir::LoopLatchKind::SourceForStep {
            kind: hir::SourceForKind::BorrowBytes,
            step: step.id.clone(),
        }
    );

    let rendered = graph::to_json(&program).unwrap();
    graph::verify_json(&program, &rendered).unwrap();
    let compact =
        serde_json::to_string(&serde_json::from_str::<serde_json::Value>(&rendered).unwrap())
            .unwrap();
    assert!(compact.contains("core.bytes.get"));
    assert!(compact.contains("\"source_kind\":\"borrow_bytes\""));
    // Only the receivers select a single case; the manual oracle stays exhaustive.
    assert!(compact.contains("\"exhaustive\":false"));
    assert!(compact.contains("\"exhaustive\":true"));
    // Graph v83 owns these additive spellings.
    let document: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(document["schema"], "semaprax.graph.v83");
    let forged = rendered.replacen("semaprax.graph.v83", "semaprax.graph.v81", 1);
    assert!(graph::verify_json(&program, &forged).is_err());

    // A Vec traversal keeps its prior schema and spells none of them.
    let vec_program = parsed(
        "module app.vec_traversal;\n\n@id(\"app.vec.main\")\nfn main() -> i64 {\n    let values = vec_push<i64>(vec_with_capacity<i64>(1usize), 7);\n    let mut total = 0;\n    for value in values { total = total + value; 0 }\n    total\n}\n",
    );
    let vec_graph = graph::to_json(&vec_program).unwrap();
    assert!(!vec_graph.contains("semaprax.graph.v83"));
    assert!(!vec_graph.contains("borrow_bytes"));
    assert!(!vec_graph.contains("\"exhaustive\":false"));
}

#[test]
fn hostile_byte_traversal_hir_is_refused_independently_of_its_spelling() {
    let original = checked();
    let refuse = |program: hir::ResolvedProgram, case: &str| {
        let error = hir::validate(&program).expect_err(case);
        assert!(error.code.starts_with("SPX-H"), "{case}: {error:?}");
    };

    let mut forged = original.clone();
    let (_, latch) = loop_steps(wrapper_statements(&mut forged, "exits"));
    let Some(hir::LoopLatchKind::SourceForStep { kind, .. }) = latch else {
        panic!("byte latch")
    };
    *kind = hir::SourceForKind::Copy;
    refuse(forged, "Copy latch over a byte receiver");

    let mut forged = original.clone();
    let (steps, latch) = loop_steps(wrapper_statements(&mut forged, "exits"));
    let read = steps[0].value().id.clone();
    let Some(hir::LoopLatchKind::SourceForStep { step, .. }) = latch else {
        panic!("byte latch")
    };
    *step = read;
    refuse(forged, "latch step detached from the unit advance");

    let mut forged = original.clone();
    let statements = wrapper_statements(&mut forged, "sum");
    let length = statements[0].binding().id.clone();
    let (steps, _) = loop_steps(statements);
    let ResolvedStatement::Let { value: read, .. } = &mut steps[0] else {
        panic!("item")
    };
    let ResolvedExprKind::Match { scrutinee, .. } = &mut read.kind else {
        panic!("receiver")
    };
    let ResolvedExprKind::Call { args, .. } = &mut scrutinee.kind else {
        panic!("byte_get")
    };
    args[1].kind = ResolvedExprKind::Place(hir::Place {
        root: length,
        projections: Vec::new(),
    });
    refuse(forged, "receiver index forged to the snapshot");

    let mut forged = original.clone();
    let statements = wrapper_statements(&mut forged, "sum");
    let ResolvedStatement::Let { value: seed, .. } = &mut statements[1] else {
        panic!("seed")
    };
    seed.kind = ResolvedExprKind::Usize(1);
    refuse(forged, "skipped first element");

    let mut forged = original.clone();
    let (steps, _) = loop_steps(wrapper_statements(&mut forged, "sum"));
    let ResolvedStatement::Assign { value: advance, .. } = &mut steps[2] else {
        panic!("advance")
    };
    let ResolvedExprKind::Binary { right, .. } = &mut advance.kind else {
        panic!("unit step")
    };
    right.kind = ResolvedExprKind::Usize(2);
    refuse(forged, "skipped alternate elements");

    let mut forged = original.clone();
    let (steps, _) = loop_steps(wrapper_statements(&mut forged, "exits"));
    let ResolvedStatement::Let { value: read, .. } = &mut steps[0] else {
        panic!("item")
    };
    let ResolvedExprKind::Match { arms, .. } = &mut read.kind else {
        panic!("receiver")
    };
    let mut none = arms[0].clone();
    none.pattern = hir::ResolvedMatchPattern::Variant {
        variant: hir::DeclarationId::new("core.option"),
        case: hir::DeclarationId::new("core.option.none"),
        fields: Vec::new(),
    };
    none.value.kind = ResolvedExprKind::Uint8(0);
    arms.push(none);
    refuse(forged, "default-zero None arm under the byte latch");

    let mut forged = original.clone();
    let statements = wrapper_statements(&mut forged, "sum");
    let ResolvedStatement::Let { value: count, .. } = &mut statements[0] else {
        panic!("snapshot")
    };
    let ResolvedExprKind::Call { callee, .. } = &mut count.kind else {
        panic!("byte_len")
    };
    *callee = hir::DeclarationId::new("core.vec.len");
    refuse(forged, "receiver detached from its authenticated region");

    let mut forged = original.clone();
    let statements = wrapper_statements(&mut forged, "sum");
    let counter = statements[1].binding().clone();
    let (steps, _) = loop_steps(statements);
    let ResolvedStatement::Let {
        value: authored, ..
    } = &mut steps[1]
    else {
        panic!("authored body")
    };
    let ResolvedExprKind::Block { statements, .. } = &mut authored.kind else {
        panic!("authored block")
    };
    let ResolvedStatement::Assign { binding, value, .. } = &mut statements[0] else {
        panic!("authored assignment")
    };
    *binding = counter;
    value.kind = ResolvedExprKind::Usize(0);
    value.ty = ResolvedType::Usize;
    refuse(forged, "authored body rewinds the hidden counter");
}

#[test]
fn byte_for_refusals_and_owner_freeze_are_stable() {
    let baseline = parsed(REFUSAL);
    assert!(verify::verify(&baseline).is_empty());
    hir::validate(&hir::resolve(&baseline).unwrap()).unwrap();

    let codes = |source: String| -> Vec<&'static str> {
        verify::verify(&parsed(&source))
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect()
    };

    let computed = REFUSAL.replacen("for byte in view", "for byte in bytes_as_slice(owned)", 1);
    assert!(codes(computed).contains(&"SPX-T284"));

    let mutable = REFUSAL.replacen("let view = ", "let mut view = ", 1);
    assert!(codes(mutable.clone()).contains(&"SPX-T284"));
    assert!(hir::resolve(&parsed(&mutable)).is_err());

    let reassigned = REFUSAL.replacen("let first = byte;", "byte = 0u8;", 1);
    assert!(codes(reassigned).contains(&"SPX-U101"));

    let shadowed = REFUSAL.replacen(
        "let mut total = 0;",
        "let mut total = 0; let byte = 1u8;",
        1,
    );
    let shadowed_codes = codes(shadowed.clone());
    assert!(shadowed_codes.contains(&"SPX-T209"));
    assert!(hir::resolve(&parsed(&shadowed)).is_err());

    let nested = REFUSAL.replacen(
        "let first = byte;",
        "for inner in view { let copy = inner; 0 } let first = byte;",
        1,
    );
    assert!(codes(nested).contains(&"SPX-T284"));

    // The body never names the view, so its alias loan alone could expire;
    // the traversal freeze keeps the owner unavailable for every iteration.
    let moved = REFUSAL.replacen(
        "let first = byte;",
        "let first = byte; let stolen = owned;",
        1,
    );
    assert!(codes(moved).contains(&"SPX-T265"));

    let renewed = REFUSAL
        .replacen("let owned = ", "let mut owned = ", 1)
        .replacen(
            "let first = byte;",
            "let first = byte; owned = bytes_copy(array_as_slice(data));",
            1,
        );
    assert!(codes(renewed).contains(&"SPX-T265"));

    let subrange = REFUSAL
        .replacen(
            "let view = bytes_as_slice(owned);",
            "let whole = bytes_as_slice(owned); let view = byte_range(whole, 0usize, 1usize);",
            1,
        )
        .replacen(
            "let first = byte;",
            "let first = byte; let stolen = owned;",
            1,
        );
    assert!(codes(subrange).contains(&"SPX-T265"));
}
