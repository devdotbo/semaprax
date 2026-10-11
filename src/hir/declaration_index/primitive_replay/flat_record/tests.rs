//! Replayed flat records retain the ordinary index's empty parameter entry.
use super::*;
const SOURCE: &str = r#"module replay.flat_record;
@id("app.Row") record Row {@id("app.Row.n") n:i64,@id("app.Row.flag") flag:bool,}
@id("app.main") fn main()->i64 {let row=Row{n:42,flag:true};row.n}
"#;
pub(in super::super) fn program() -> crate::hir::ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "replay-flat-record.spx").unwrap()).unwrap()
}
pub(in super::super) fn row(program: &crate::hir::ResolvedProgram) -> &ResolvedTypeDeclaration {
    program
        .types
        .iter()
        .find(|record| record.id.as_str() == "app.Row")
        .unwrap()
}
pub(in super::super) fn row_type() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new("app.Row"),
        arguments: Vec::new(),
    }
}
/// Replay `record` into `index` with the source's explicit origins.
pub(in super::super) fn append(
    index: &mut DeclarationIndex,
    record: &ResolvedTypeDeclaration,
    physical: &mut ReplayPhysicalReservation,
    debit: &mut dyn FnMut(usize) -> Result<(), crate::diagnostic::Diagnostic>,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        panic!("record")
    };
    let count = fields.len();
    index.append_flat_record_replay_metered(
        record,
        IdentityOrigin::Explicit,
        |ordinal| (ordinal < count).then_some(IdentityOrigin::Explicit),
        physical,
        debit,
    )
}
#[test]
fn flat_record_replay_retains_the_ordinary_empty_parameter_entry_at_exact_work_and_bytes() {
    let program = program();
    let record = row(&program);
    // The ordinary index owns the expected entry; replay must reproduce it.
    let ordinary = program.declarations.type_parameters(&record.id).unwrap();
    assert!(ordinary.is_empty());
    assert!(crate::hir::copy_record_collection::admitted(
        &program.declarations,
        &row_type()
    ));
    let run = |work: usize, bytes: usize| {
        crate::test_allocation_observer::measure(|| {
            let mut remaining = work;
            let mut physical = ReplayPhysicalReservation::new(bytes);
            let mut index = DeclarationIndex::default();
            let result = append(&mut index, record, &mut physical, &mut |units| {
                remaining = remaining.checked_sub(units).ok_or_else(|| {
                    crate::diagnostic::Diagnostic::io("SPX-H006", "exact replay work exhausted")
                })?;
                Ok(())
            });
            let entry = index.type_parameters(&record.id).map(<[_]>::to_vec);
            let admitted = crate::hir::copy_record_collection::admitted(&index, &row_type());
            if result.is_err() {
                // Refusal precedes every insertion: nothing partial survives.
                assert!(entry.is_none() && !admitted);
                assert!(index.declarations.is_empty() && index.record_fields.is_empty());
            }
            drop(index);
            result.map(|()| (work - remaining, physical.charged(), entry, admitted))
        })
    };
    let (wide, allocation) = run(1 << 20, 1 << 20);
    let (work, bytes, entry, admitted) = wide.unwrap();
    assert_eq!(entry.as_deref(), Some(ordinary));
    assert!(admitted, "retained Vec<Row> element classification");
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.live == 0);
    let (exact, allocation) = run(work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes, Some(Vec::new()), true));
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.live == 0);
    assert!(run(work - 1, bytes).0.is_err());
    assert!(run(work, bytes - 1).0.is_err());
}
#[test]
fn flat_record_replay_refuses_generic_compiler_owned_and_prior_parameter_entries() {
    let program = program();
    for attack in 0..5 {
        let mut record = row(&program).clone();
        let mut index = DeclarationIndex::default();
        let mut physical = ReplayPhysicalReservation::new(1 << 20);
        let result = match attack {
            0 => {
                record
                    .type_parameters
                    .push(ResolvedTypeParameterDeclaration {
                        name: "T".into(),
                        index: 0,
                        span: crate::ast::Span::default(),
                    });
                append(&mut index, &record, &mut physical, &mut |_| Ok(()))
            }
            1 => index.append_flat_record_replay_metered(
                &record,
                IdentityOrigin::CompilerOwned,
                |_| Some(IdentityOrigin::Explicit),
                &mut physical,
                &mut |_| Ok(()),
            ),
            2 => index.append_flat_record_replay_metered(
                &record,
                IdentityOrigin::Explicit,
                |_| Some(IdentityOrigin::CompilerOwned),
                &mut physical,
                &mut |_| Ok(()),
            ),
            3 => {
                index.type_parameters.insert(record.id.clone(), Vec::new());
                append(&mut index, &record, &mut physical, &mut |_| Ok(()))
            }
            _ => {
                append(&mut index, &record, &mut physical, &mut |_| Ok(())).unwrap();
                append(&mut index, &record, &mut physical, &mut |_| Ok(()))
            }
        };
        assert_eq!(result.unwrap_err().code, "SPX-H006", "attack {attack}");
    }
}
