//! Generic checked purity against the saved Vec-only oracle: identical verdict,
//! charge sequence, remaining work and first refusal, plus allocation-free
//! checked refusals. Paid acquisition and report delivery belong to the caller.
use super::*;
mod oracle;
const ROWS: &str = r#"module rows;
@id("rows.Row") record Row {@id("rows.Row.n") n:i64,@id("rows.Row.text") text:string,}
@id("rows.read") fn read(row:borrow Row)->i64 {row.n}
@id("rows.mixed") fn mixed(row:borrow Row,x:i64)->i64 {let y=x+row.n; if y>0 {y} else {0}}
@id("rows.scalar") fn scalar(x:i64,y:i64)->i64 {let z=x+y; z}
@id("rows.req") fn req(x:i64)->i64 requires x>=0 {x}
@id("rows.ens") fn ens(view:borrow Slice<u8>)->usize ensures result==byte_len(view) {byte_len(view)}
@id("rows.text") fn text(t:borrow str)->i64 {str_len_bytes(t)}
@id("rows.option") fn option(view:borrow Slice<u8>,at:usize)->i64 {match byte_get(view,at) {Option::Some {value}=>i64_from_u8(value),Option::None {}=>0,}}
@id("rows.dead") fn dead(x:i64)->i64 {if false {str_len_bytes(str"x")} else {x}}
@id("rows.main") fn main()->i64 {0}
"#;
fn program() -> hir::ResolvedProgram {
    hir::resolve(&crate::check(ROWS, "rows.spx").unwrap()).unwrap()
}
fn function<'a>(program: &'a hir::ResolvedProgram, id: &str) -> &'a ResolvedFunction {
    program
        .functions
        .iter()
        .find(|f| f.id.as_str() == id)
        .unwrap()
}
fn tail(function: &mut ResolvedFunction) -> &mut hir::ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("resolved block")
    };
    tail
}
// `row.n` rewritten as Project over the bare current Borrow root.
fn project(read: &ResolvedFunction) -> ResolvedFunction {
    let mut candidate = read.clone();
    let tail = tail(&mut candidate);
    let ResolvedExprKind::Place(place) = &tail.kind else {
        panic!("field place")
    };
    let [hir::PlaceProjection::Field(field)] = place.projections.as_slice() else {
        panic!("one field")
    };
    let field = field.clone();
    let mut base = tail.clone();
    let ResolvedExprKind::Place(root) = &mut base.kind else {
        unreachable!()
    };
    root.projections.clear();
    base.ty = read.params[0].ty.clone();
    base.ownership = OwnershipMode::Borrow;
    tail.kind = ResolvedExprKind::Project {
        base: Box::new(base),
        field,
    };
    candidate
}
/// Every BorrowRecord Field/Project/scalar/Str/Slice/Option/contract/dead
/// branch control plus Own, generic, duplicate and non-current record mutants.
fn cases(program: &hir::ResolvedProgram) -> Vec<(String, ResolvedFunction)> {
    let mut cases = Vec::new();
    for id in [
        "rows.read",
        "rows.mixed",
        "rows.scalar",
        "rows.req",
        "rows.ens",
        "rows.text",
        "rows.option",
        "rows.dead",
    ] {
        cases.push((id.to_owned(), function(program, id).clone()));
    }
    let read = function(program, "rows.read");
    let projected = project(read);
    cases.push(("project".to_owned(), projected.clone()));
    for attack in 0..11 {
        let mut candidate = if attack < 7 {
            read.clone()
        } else {
            projected.clone()
        };
        let foreign = ResolvedType::Nominal {
            declaration: hir::DeclarationId::new("foreign.Row"),
            arguments: Vec::new(),
        };
        let generic = ResolvedType::Nominal {
            declaration: hir::DeclarationId::new("rows.Row"),
            arguments: vec![ResolvedType::I64],
        };
        match attack {
            0 => candidate.params[0].ownership = OwnershipMode::Own,
            1 => candidate.params[0].ty = generic,
            2 => {
                let duplicate = candidate.params[0].clone();
                candidate.params.push(duplicate);
            }
            3 => candidate.params[0].ty = foreign,
            4 => tail(&mut candidate).ty = ResolvedType::Bool,
            5 => tail(&mut candidate).ownership = OwnershipMode::Own,
            6 => {
                let ResolvedExprKind::Place(place) = &mut tail(&mut candidate).kind else {
                    unreachable!()
                };
                place.projections[0] =
                    hir::PlaceProjection::Field(hir::DeclarationId::new("foreign.field"));
            }
            7 => candidate.params[0].ownership = OwnershipMode::Own,
            8 => {
                let ResolvedExprKind::Project { base, .. } = &mut tail(&mut candidate).kind else {
                    unreachable!()
                };
                base.ty = generic;
            }
            9 => {
                let ResolvedExprKind::Project { base, .. } = &mut tail(&mut candidate).kind else {
                    unreachable!()
                };
                base.ownership = OwnershipMode::Own;
            }
            _ => {
                let ResolvedExprKind::Project { field, .. } = &mut tail(&mut candidate).kind else {
                    unreachable!()
                };
                *field = hir::DeclarationId::new("rows.Row.text");
            }
        }
        cases.push((format!("attack {attack}"), candidate));
    }
    cases
}
// Current, duplicated, renamed (non-current), absent and empty record tables.
type Records = Vec<hir::ResolvedTypeDeclaration>;
fn record_sets(program: &hir::ResolvedProgram) -> Vec<(&'static str, Records)> {
    let current = program.types.clone();
    let mut duplicate = current.clone();
    duplicate.extend(current.iter().cloned());
    let mut foreign = current.clone();
    for record in &mut foreign {
        if record.id.as_str() == "rows.Row" {
            record.id = hir::DeclarationId::new("foreign.Row");
        }
    }
    let mut absent = current.clone();
    absent.retain(|record| record.id.as_str() != "rows.Row");
    vec![
        ("current", current),
        ("duplicate", duplicate),
        ("foreign", foreign),
        ("absent", absent),
        ("none", Vec::new()),
    ]
}
type Oracle = Result<bool, Vec<Diagnostic>>;
struct Run<R> {
    result: R,
    charges: Vec<usize>,
    work: usize,
}
fn oracle_run(
    function: &ResolvedFunction,
    records: &[hir::ResolvedTypeDeclaration],
    option_shape: bool,
    work: usize,
    stop: usize,
    refusal: &mut Option<Vec<Diagnostic>>,
) -> Run<Oracle> {
    let (mut charges, mut work) = (Vec::new(), work);
    let result = oracle::pure_profile_metered(
        function,
        &mut work,
        &mut |units| {
            charges.push(units);
            if charges.len() == stop {
                return Err(refusal.take().unwrap());
            }
            Ok(())
        },
        option_shape,
        records,
    );
    Run {
        result,
        charges,
        work,
    }
}
fn legacy_run(
    function: &ResolvedFunction,
    records: &[hir::ResolvedTypeDeclaration],
    option_shape: bool,
    work: usize,
    stop: usize,
    refusal: &mut Option<Vec<Diagnostic>>,
) -> Run<Oracle> {
    let (mut charges, mut work) = (Vec::new(), work);
    let result = pure_profile_metered(
        function,
        &mut work,
        &mut |units| {
            charges.push(units);
            if charges.len() == stop {
                return Err(refusal.take().unwrap());
            }
            Ok(())
        },
        option_shape,
        records,
    );
    Run {
        result,
        charges,
        work,
    }
}
fn checked_run(
    function: &ResolvedFunction,
    records: &[hir::ResolvedTypeDeclaration],
    option_shape: bool,
    work: usize,
    stop: usize,
) -> Run<Result<bool, PurityRefusal<usize>>> {
    let (mut charges, mut work) = (Vec::new(), work);
    let result = pure_profile_checked(
        function,
        &mut work,
        &mut |units| {
            charges.push(units);
            if charges.len() == stop {
                return Err(stop);
            }
            Ok(())
        },
        option_shape,
        records,
    );
    Run {
        result,
        charges,
        work,
    }
}
fn assert_parity(
    label: &str,
    function: &ResolvedFunction,
    records: &[hir::ResolvedTypeDeclaration],
    option_shape: bool,
    work: usize,
    stop: usize,
) {
    let prepared = || {
        let refusal = vec![graph_error("SPX-TEST", "prepared callback refusal")];
        let pointer = refusal.as_ptr();
        (Some(refusal), pointer)
    };
    let (mut old_refusal, old_pointer) = prepared();
    let (mut legacy_refusal, legacy_pointer) = prepared();
    let old = oracle_run(
        function,
        records,
        option_shape,
        work,
        stop,
        &mut old_refusal,
    );
    let legacy = legacy_run(
        function,
        records,
        option_shape,
        work,
        stop,
        &mut legacy_refusal,
    );
    let checked = checked_run(function, records, option_shape, work, stop);
    let context = format!("{label} option={option_shape} work={work} stop={stop}");
    assert_eq!(old.charges, checked.charges, "{context}");
    assert_eq!(old.charges, legacy.charges, "{context}");
    assert_eq!(
        (old.work, old.work),
        (checked.work, legacy.work),
        "{context}"
    );
    match old.result {
        Ok(verdict) => {
            assert_eq!(checked.result, Ok(verdict), "{context}");
            assert_eq!(legacy.result, Ok(verdict), "{context}");
        }
        Err(errors) if old_refusal.is_none() => {
            // The callback refused: its exact value, never a local diagnostic.
            assert_eq!(errors.as_ptr(), old_pointer, "{context}");
            assert_eq!(
                checked.result,
                Err(PurityRefusal::Charge(stop)),
                "{context}"
            );
            let legacy = legacy.result.unwrap_err();
            assert!(legacy_refusal.is_none(), "{context}");
            assert_eq!(legacy.as_ptr(), legacy_pointer, "{context}");
            assert_eq!(legacy, errors, "{context}");
        }
        Err(errors) => {
            assert_eq!(errors, fail(), "{context}");
            assert_eq!(checked.result, Err(PurityRefusal::LocalLimit), "{context}");
            assert_eq!(legacy.result.unwrap_err(), fail(), "{context}");
            assert!(legacy_refusal.is_some(), "{context}");
        }
    }
}
#[test]
fn checked_purity_matches_saved_oracle_verdict_charges_and_first_refusal() {
    let program = program();
    let cases = cases(&program);
    let records = record_sets(&program);
    let mut verdicts = [0usize; 2];
    for (label, function) in &cases {
        for (set, records) in &records {
            for option_shape in [false, true] {
                let label = format!("{label} records={set}");
                let full = checked_run(function, records, option_shape, WORK, 0);
                verdicts[usize::from(full.result == Ok(true))] += 1;
                assert_parity(&label, function, records, option_shape, WORK, 0);
                let total: usize = full.charges.iter().sum();
                // Every external refusal position, then every local budget
                // through the exact total (exact succeeds, one short refuses).
                for stop in 1..=full.charges.len() {
                    assert_parity(&label, function, records, option_shape, WORK, stop);
                }
                for work in 0..=total {
                    assert_parity(&label, function, records, option_shape, work, 0);
                }
            }
        }
    }
    // The fixtures exercise both admitted and structurally refused shapes.
    assert!(verdicts[0] > 0 && verdicts[1] > 0, "{verdicts:?}");
    let current = &records[0].1;
    for (id, expected) in [
        ("rows.read", true),
        ("rows.mixed", true),
        ("rows.scalar", true),
        ("rows.req", true),
        ("rows.ens", true),
        ("rows.text", true),
        ("rows.option", false),
        ("rows.dead", false),
    ] {
        let function = function(&program, id);
        assert_eq!(
            checked_run(function, current, false, WORK, 0).result,
            Ok(expected),
            "{id}"
        );
    }
    assert_eq!(
        checked_run(&cases[8].1, current, false, WORK, 0).result,
        Ok(true),
        "project"
    );
    for (label, function) in &cases[9..] {
        assert_eq!(
            checked_run(function, current, false, WORK, 0).result,
            Ok(false),
            "{label}"
        );
    }
}
#[test]
fn legacy_records_wrapper_keeps_g410_and_callback_vec_identity() {
    let program = program();
    let read = function(&program, "rows.read");
    let mut spent = 0;
    let mut work = WORK;
    assert!(
        pure_records_metered(read, &program.types, &mut work, &mut |n| {
            spent += n;
            Ok(())
        })
        .unwrap()
    );
    assert_eq!(WORK - work, spent);
    let mut short = spent - 1;
    let errors =
        pure_records_metered(read, &program.types, &mut short, &mut |_| Ok(())).unwrap_err();
    assert_eq!(errors, fail());
    assert_eq!(errors[0].code, "SPX-G410");
    let refusal = vec![graph_error("SPX-TEST", "prepared callback refusal")];
    let pointer = refusal.as_ptr();
    let mut refusal = Some(refusal);
    let errors = pure_records_metered(read, &program.types, &mut { WORK }, &mut |_| {
        Err(refusal.take().unwrap())
    })
    .unwrap_err();
    assert_eq!(errors.as_ptr(), pointer);
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-TEST");
}
#[test]
fn checked_records_refusals_and_verdicts_allocate_nothing() {
    let program = program();
    let read = function(&program, "rows.read");
    let mut owned = read.clone();
    owned.params[0].ownership = OwnershipMode::Own;
    let records = program.types.as_slice();
    let mut total = 0;
    let mut calls = 0;
    assert_eq!(
        pure_records_checked(read, records, &mut { WORK }, &mut |n| {
            total += n;
            calls += 1;
            Ok::<(), u8>(())
        }),
        Ok(true)
    );
    assert!(total > 1 && calls > 2);
    let middle = calls / 2;
    // Fixed inputs and expected values live outside each observation.
    for (label, function, budget, stop, expected) in [
        ("exact", read, total, usize::MAX, Ok(true)),
        (
            "one short",
            read,
            total - 1,
            usize::MAX,
            Err(PurityRefusal::LocalLimit),
        ),
        ("empty", read, 0, usize::MAX, Err(PurityRefusal::LocalLimit)),
        (
            "callback",
            read,
            WORK,
            middle,
            Err(PurityRefusal::Charge(7u8)),
        ),
        ("structural", &owned, WORK, usize::MAX, Ok(false)),
    ] {
        let mut work = budget;
        let mut seen = 0usize;
        let (result, observed) = crate::test_allocation_observer::measure(|| {
            pure_records_checked(function, records, &mut work, &mut |_| {
                seen += 1;
                if seen == stop {
                    return Err(7u8);
                }
                Ok(())
            })
        });
        assert_eq!(result, expected, "{label}");
        assert_eq!(
            (
                observed.allocations,
                observed.reallocations,
                observed.allocated
            ),
            (0, 0, 0),
            "{label}"
        );
        assert_eq!(observed.live, 0, "{label}");
        assert!(!observed.invalid, "{label}");
        if label == "exact" {
            assert_eq!(work, 0);
        }
        if label == "callback" {
            assert_eq!(seen, middle);
        }
    }
}
