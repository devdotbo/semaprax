//! Option metadata admits only replayed monomorphic flat-record entries.
use super::super::flat_record::tests::{append, program, row, row_type};
use super::*;
fn option_u8() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(crate::prelude::OPTION_ID),
        arguments: vec![ResolvedType::U8],
    }
}
fn replayed(program: &crate::hir::ResolvedProgram) -> DeclarationIndex {
    let mut index = DeclarationIndex::default();
    let mut physical = ReplayPhysicalReservation::new(1 << 20);
    append(&mut index, row(program), &mut physical, &mut |_| Ok(())).unwrap();
    index
}
fn run(
    index: &DeclarationIndex,
    work: usize,
    bytes: usize,
) -> (
    Result<(usize, usize), crate::diagnostic::Diagnostic>,
    DeclarationIndex,
) {
    let mut index = index.clone();
    let mut remaining = work;
    let mut physical = ReplayPhysicalReservation::new(bytes);
    let result = index.append_option_u8_replay_metered(&mut physical, &mut |units| {
        remaining = remaining.checked_sub(units).ok_or_else(|| {
            crate::diagnostic::Diagnostic::io("SPX-H006", "exact Option work exhausted")
        })?;
        Ok(())
    });
    (
        result.map(|()| (work - remaining, physical.charged())),
        index,
    )
}
#[test]
fn option_u8_replay_admits_replayed_flat_records_at_exact_and_one_short_work() {
    let program = program();
    let base = replayed(&program);
    let (empty, _) = run(&DeclarationIndex::default(), 1 << 20, 1 << 20);
    let (empty_work, _) = empty.unwrap();
    let (wide, index) = run(&base, 1 << 20, 1 << 20);
    let (work, bytes) = wide.unwrap();
    // Every retained record key, field key and parameter entry is charged.
    assert!(work > empty_work, "{work} <= {empty_work}");
    assert!(crate::hir::borrowed_scalar::option_u8::authenticated(
        &index,
        &option_u8()
    ));
    assert!(crate::hir::copy_record_collection::admitted(
        &index,
        &row_type()
    ));
    assert_eq!(
        index.type_parameters(&DeclarationId::new("app.Row")),
        Some(&[][..])
    );
    let (exact, exact_index) = run(&base, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert_eq!(exact_index, index);
    assert!(run(&base, work - 1, bytes).0.is_err());
    assert!(run(&base, work, bytes - 1).0.is_err());
}
#[test]
fn option_u8_replay_refuses_generic_compiler_owned_orphan_and_unfielded_entries() {
    let program = program();
    let row_id = DeclarationId::new("app.Row");
    for attack in 0..10 {
        let mut index = replayed(&program);
        match attack {
            0 => index.type_parameters.get_mut(&row_id).unwrap().push(
                ResolvedTypeParameterDeclaration {
                    name: "T".into(),
                    index: 0,
                    span: crate::ast::Span::default(),
                },
            ),
            1 => {
                index.declarations.get_mut(&row_id).unwrap().identity_origin =
                    IdentityOrigin::CompilerOwned
            }
            2 => {
                index.record_fields.remove(&row_id).unwrap();
            }
            3 => {
                index
                    .type_parameters
                    .insert(DeclarationId::new("foreign.Row"), Vec::new());
            }
            4 => {
                index
                    .declarations
                    .get_mut(&DeclarationId::new("app.Row.n"))
                    .unwrap()
                    .owner = None
            }
            5 => {
                // Compiler-owned generic Vec metadata is not a record entry.
                let mut physical = ReplayPhysicalReservation::new(1 << 20);
                index
                    .append_vec_replay_metered(&mut physical, &mut |_| Ok(()))
                    .unwrap();
            }
            6 => index.declarations.get_mut(&row_id).unwrap().kind = DeclarationKind::Variant,
            7 => *index.record_fields.get_mut(&row_id).unwrap() = Vec::new(),
            8 => {
                index
                    .declarations
                    .get_mut(&DeclarationId::new("app.Row.flag"))
                    .unwrap()
                    .identity_origin = IdentityOrigin::CompilerOwned
            }
            _ => {
                index.variant_cases.insert(row_id.clone(), Vec::new());
            }
        }
        let (result, after) = run(&index, 1 << 20, 1 << 20);
        assert_eq!(result.unwrap_err().code, "SPX-H006", "attack {attack}");
        // Admission is decided before any Option metadata is inserted.
        assert_eq!(after, index, "attack {attack}");
        assert!(!crate::hir::borrowed_scalar::option_u8::authenticated(
            &after,
            &option_u8()
        ));
    }
}
