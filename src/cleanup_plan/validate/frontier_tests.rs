use super::*;
use crate::hir::ResolvedExprKind;
use crate::loop_calls::byte_writer::validation_frontier::Frontier;
use crate::loop_calls::byte_writer::{self, resolved_target};
const HELPERS: &str = include_str!("../../../tests/owned_data/byte_helper_renewal.spx");
const LEXICAL: &str = r#"module frontier.lexical;
@id("first") fn first()->i64 {0}
@id("late") fn late()->i64 {return 1;}
"#;
fn program(source: &str) -> ResolvedProgram {
    let parsed = crate::parse(source, "validation-frontier.spx").unwrap();
    let diagnostics = crate::verify::verify(&parsed);
    if parsed
        .functions
        .iter()
        .any(|function| function.name == "main")
    {
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        crate::hir::resolve(&parsed).unwrap()
    } else {
        // Keep the original complete library and both public entry refusals.
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "SPX-T105");
        let rejected = crate::hir::resolve(&parsed).unwrap_err();
        assert_eq!(rejected.len(), 1, "{rejected:?}");
        assert_eq!(rejected[0].code, "SPX-T105");
        // The success receiver owns an executable fixture containing the whole
        // unchanged library prefix; neither verifier bypasses entry admission.
        let executable = format!("{source}\n@id(\"frontier.fixture.main\") fn main()->i64 {{0}}\n");
        let checked = crate::check(&executable, "validation-frontier.spx").unwrap();
        assert_eq!(
            &checked.functions[..parsed.functions.len()],
            parsed.functions
        );
        crate::hir::resolve(&checked).unwrap()
    }
}

fn old(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    super::owned::validate_program_unindexed_reference(program)
}
fn new(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    crate::cleanup_plan::validate_program(program)
}
fn attached(program: &ResolvedProgram) -> Vec<Vec<u8>> {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .map(|f| crate::cache_codec::encode(&f.cleanup_plan).unwrap())
        .collect()
}
fn parity(program: &ResolvedProgram) {
    let before = attached(program);
    let expected = old(program).map_err(|d| (d.code, d.message));
    let actual = new(program).map_err(|d| (d.code, d.message));
    assert_eq!(actual, expected);
    assert_eq!(attached(program), before);
}
#[test]
fn deferred_frontier_preserves_full_ordinary_lexical_generic_and_helper_plans() {
    for source in [
        HELPERS,
        LEXICAL,
        r#"module frontier.vec;
@id("row") record Row {@id("row.s") text:string,@id("row.n") n:i64,}
@id("main") fn main()->i64 {let mut rows=vec_with_capacity<Row>(1usize);let mut bytes=bytes_zeroed(1usize);let mut i=0;
 while i<1 {rows=vec_push<Row>(rows,Row{text:"row",n:i});bytes=bytes_set(bytes,0usize,65u8);i=i+1;0} 0}"#,
        r#"module frontier.iterator;
@id("main") fn main()->i64 {let values=vec_push<i64>(vec_with_capacity<i64>(1usize),1);let mut target=vec_with_capacity<i64>(1usize);
 for own item in vec_into_iter<i64>(values) {if item==1 {target=vec_push<i64>(target,item);0}else{0};0} 0}"#,
        r#"module frontier.string;
@id("main") fn main()->i64 {let mut text="a";let mut i=0;while i<1 {text=string_concat("prefix",text);i=i+1;0} 0}"#,
        r#"module frontier.consuming;
@id("row") record Row {@id("row.a") a:Bytes,@id("row.n") n:i64,@id("row.b") b:Bytes,}
@id("main") fn main()->i64 {let rows=vec_push<Row>(vec_with_capacity<Row>(1usize),Row{a:bytes_zeroed(1usize),n:1,b:bytes_zeroed(1usize)});let mut target=vec_with_capacity<Row>(1usize);
 for own row in vec_into_iter<Row>(rows) {target=vec_push<Row>(target,row);0} 0}"#,
        r#"module frontier.generic;
@id("box") record Box<T> {@id("box.value") value:T,}
@id("relay") fn relay<T>(value:own Box<T>)->Box<T> {value}
@id("main") fn main()->i64 {let value=relay<Bytes>(Box<Bytes>{value:bytes_zeroed(1usize)});0}"#,
    ] {
        let program = program(source);
        old(&program).unwrap();
        new(&program).unwrap();
        parity(&program);
        let mut frontier = Frontier::new(&program);
        for function in program
            .functions
            .iter()
            .chain(program.function_instances.iter().map(|i| &i.function))
        {
            let current = frontier.current().unwrap();
            let rebuilt = crate::cleanup_plan::build::helper_index::validation::build_current(
                &program, &current,
            )
            .unwrap();
            assert_eq!(
                crate::cache_codec::encode(&rebuilt).unwrap(),
                crate::cache_codec::encode(&function.cleanup_plan).unwrap()
            );
            current.finish().unwrap();
            frontier.advance().unwrap();
        }
    }
}
#[test]
fn deferred_frontier_preserves_earlier_mismatch_and_global_lexical_owner_priority() {
    let mut missing = program(LEXICAL);
    missing.functions[1].id = crate::hir::DeclarationId::new("foreign.late");
    parity(&missing);
    missing.functions[0].cleanup_plan.schema = "forged";
    let selected = old(&missing).unwrap_err();
    assert!(selected.message.contains("non-canonical schema"));
    parity(&missing);
    let mut ambiguous = program(LEXICAL);
    ambiguous.functions.push(ambiguous.functions[1].clone());
    assert!(old(&ambiguous).unwrap_err().message.contains("ambiguous"));
    parity(&ambiguous);
    ambiguous.functions[0].cleanup_plan.schema = "forged";
    parity(&ambiguous);
    let mut deep = program(LEXICAL);
    for _ in 0..600 {
        let mut wrapper = deep.functions[1].body.clone();
        // A typed leaf is only the temporary replacement; the original
        // complete body becomes the mandatory tail of every nested block.
        wrapper.kind = ResolvedExprKind::Int(0);
        let child = std::mem::replace(&mut deep.functions[1].body, wrapper);
        deep.functions[1].body.kind = ResolvedExprKind::Block {
            statements: vec![],
            tail: Box::new(child),
        };
    }
    parity(&deep);
    deep.functions[0].cleanup_plan.schema = "forged";
    assert!(old(&deep)
        .unwrap_err()
        .message
        .contains("non-canonical schema"));
    parity(&deep);
}
#[test]
fn deferred_frontier_exact_short_refusal_and_bool_proof_boundary_are_sticky() {
    let program = program(HELPERS);
    let (ok, refused, required) =
        crate::bounded_output::with_limit_usage(usize::MAX, || new(&program));
    ok.unwrap();
    assert!(!refused);
    assert!(required > 0);
    let (ok, refused, used) = crate::bounded_output::with_limit_usage(required, || new(&program));
    ok.unwrap();
    assert!(!refused);
    assert_eq!(used, required);
    let (short, refused, _) =
        crate::bounded_output::with_limit_usage(required - 1, || new(&program));
    assert!(short.is_err());
    assert!(refused);
    let before = attached(&program);
    let (_, refused, _) = crate::bounded_output::with_limit_usage(0, || new(&program));
    assert!(refused);
    assert_eq!(before, attached(&program));
    let leaf = program.functions.iter().find(|f| f.name == "leaf").unwrap();
    let (proved, refused, _) = crate::bounded_output::with_limit_usage(0, || {
        let mut cursor = Vec::new();
        byte_writer::validation_frontier::test_bool_target(&program, leaf, &mut cursor)
    });
    assert!(!proved);
    assert!(refused);
    let (proved, refused, _) =
        crate::bounded_output::with_limit_usage(0, || resolved_target(&program, leaf));
    assert!(!proved);
    assert!(refused);
}
#[test]
fn deferred_frontier_whole_validation_allocation_debit_and_drop_are_observed() {
    let start = HELPERS.find("@id(\"app.main\")").unwrap();
    let mut source = HELPERS.to_owned();
    for i in 1..16 {
        source.push_str(
            &HELPERS[start..]
                .replace("app.main", &format!("app.main{i}"))
                .replace("fn main()", &format!("fn main{i}()")),
        );
    }
    source.push_str(
        "@id(\"dense.string\") fn dense()->i64 {let mut text=\"a\";let mut i=0;while i<2 {",
    );
    for _ in 0..64 {
        source.push_str("text=string_concat(\"prefix\",text);");
    }
    source.push_str("i=i+1;0} 0}");
    let program = program(&source);
    fn observed(
        program: &ResolvedProgram,
        indexed: bool,
    ) -> (crate::test_allocation_observer::Metrics, usize) {
        let (((), metrics), refused, debit) =
            crate::bounded_output::with_limit_usage(usize::MAX, || {
                crate::test_allocation_observer::measure_with_capacity(131_072, || {
                    let result = if indexed { new(program) } else { old(program) };
                    result.unwrap();
                })
            });
        assert!(!refused);
        assert!(
            !metrics.invalid,
            "whole validation observation: {metrics:?}; first invalid cause: {:?}",
            crate::test_allocation_observer::invalid_reason()
        );
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
        (metrics, debit)
    }
    let (saved, old_debit) = observed(&program, false);
    let (current, new_debit) = observed(&program, true);
    assert!(
        current.allocated < saved.allocated,
        "complete receiver through final replay and Drop: {saved:?} -> {current:?}"
    );
    assert!(
        new_debit < old_debit,
        "actual paid requests: {old_debit} -> {new_debit}"
    );
}

#[test]
fn deferred_frontier_rejects_foreign_program_and_copied_function_at_builder_receiver() {
    let program = program(HELPERS);
    let copied = program.clone();
    let mut frontier = Frontier::new(&program);
    let current = frontier.current().unwrap();
    assert!(
        crate::cleanup_plan::build::helper_index::validation::reject_foreign(
            &copied,
            &copied.functions[0],
            &current
        )
        .is_err()
    );
    assert!(
        crate::cleanup_plan::build::helper_index::validation::reject_foreign(
            &program,
            &program.functions[0].clone(),
            &current
        )
        .is_err()
    );
    crate::cleanup_plan::build::helper_index::validation::build_current(&program, &current)
        .unwrap();
    current.finish().unwrap();
    frontier.advance().unwrap();
    assert!(
        frontier.advance().is_err(),
        "a consumed ordinal cannot advance again"
    );
    // Rust prevents retaining the old Current across mutable advancement.
    // Exercise the builder's runtime refusal with the original stale ordinal.
    let next = frontier.current().unwrap();
    let stale = next.with_ordinal_for_test(0);
    assert!(
        crate::cleanup_plan::build::helper_index::validation::build_current(&program, &stale)
            .is_err()
    );
}
#[test]
fn deferred_frontier_preserves_original_first_duplicate_callee_and_nonlexical_owner() {
    let mut program = program(HELPERS);
    let mut duplicate = program
        .functions
        .iter()
        .find(|f| f.name == "leaf")
        .unwrap()
        .clone();
    duplicate.body.kind = ResolvedExprKind::String("allocating later duplicate".into());
    program.functions.push(duplicate);
    parity(&program);
    let duplicate = program.functions.pop().unwrap();
    program.functions.insert(0, duplicate);
    parity(&program);
    let mut plain = program_plain();
    plain.functions[0].id = crate::hir::DeclarationId::new("nonlexical.foreign");
    parity(&plain);
}
fn program_plain() -> ResolvedProgram {
    program("module frontier.plain; @id(\"main\") fn main()->i64 {0}")
}

#[test]
fn deferred_frontier_first_lexical_owner_refusal_precedes_all_new_backing() {
    let mut program = program(LEXICAL);
    program.functions.swap(0, 1);
    program.functions[0].id = crate::hir::DeclarationId::new("foreign.first");
    let (expected, refused, debit) =
        crate::bounded_output::with_limit_usage(usize::MAX, || old(&program));
    assert!(!refused);
    let expected = expected.unwrap_err();
    assert!(expected.message.contains("unregistered lexical function"));
    let (actual, refused, used) = crate::bounded_output::with_limit_usage(debit, || new(&program));
    assert!(!refused);
    assert_eq!(used, debit);
    let actual = actual.unwrap_err();
    assert_eq!(
        (actual.code, actual.message),
        (expected.code, expected.message)
    );
}

#[test]
fn deferred_frontier_large_plain_member_overhead_and_complete_drop_are_observed() {
    for count in [128, 512] {
        let mut source = String::from("module frontier.many; @id(\"main\") fn main()->i64 {0}\n");
        for i in 1..count {
            source.push_str(&format!("@id(\"member.{i}\") fn member{i}()->i64 {{0}}\n"));
        }
        let program = program(&source);
        let before = attached(&program);
        for indexed in [false, true] {
            let start = std::time::Instant::now();
            let (((), measured), refused, debit) =
                crate::bounded_output::with_limit_usage(usize::MAX, || {
                    crate::test_allocation_observer::measure(|| {
                        let result = if indexed {
                            new(&program)
                        } else {
                            old(&program)
                        };
                        result.unwrap();
                    })
                });
            let elapsed = start.elapsed();
            assert!(!refused);
            assert!(!measured.invalid);
            assert_eq!(measured.live, 0);
            assert_eq!(measured.allocated, measured.freed);
            assert_eq!(attached(&program), before);
            // No expected gain is imposed on the no-helper control. This owns
            // literal current-row/query overhead through complete phase Drop.
            println!(
                "validation-frontier members={count} indexed={indexed} elapsed_ns={} debit={debit} allocations={measured:?}",
                elapsed.as_nanos()
            );
        }
    }
}
