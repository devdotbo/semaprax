//! Memo controls against the complete original owned-text classifiers.
use super::*;
use crate::bounded_output::{with_limit, with_limit_usage};
use crate::hir::owned_text_record::oracle;
use crate::hir::owned_text_record::paid::tests::{add_record, check, fixture, nominal, LIMIT};
use crate::test_allocation_observer::measure;

fn root() -> ResolvedType {
    nominal("text.row", Vec::new())
}

fn set_row(index: &mut DeclarationIndex, ty: ResolvedType) {
    index
        .record_fields
        .get_mut(&DeclarationId::new("text.row"))
        .unwrap()[0]
        .ty = ty;
}

fn plain() -> DeclarationIndex {
    let mut index = fixture();
    set_row(&mut index, ResolvedType::I64);
    index
}

fn texty() -> DeclarationIndex {
    let mut index = fixture();
    set_row(&mut index, ResolvedType::String);
    index
}

fn invariant() -> DeclarationIndex {
    let mut index = texty();
    let id = DeclarationId::new("text.row");
    let mut declaration = index.declarations[&id].clone();
    declaration.id = DeclarationId::new("text.row#invariant");
    index
        .declarations
        .insert(declaration.id.clone(), declaration);
    index
}

// A String plus Vec<i64> record: only the collection profile can admit it, and
// only when every field's index, owner, name and origin metadata is exact.
fn mixed(reorder: bool) -> DeclarationIndex {
    let mut index = fixture();
    let id = DeclarationId::new("text.row");
    let template = index.record_fields[&id][0].clone();
    let metadata = index.declarations[&template.id].clone();
    let types = [
        ResolvedType::String,
        nominal(crate::prelude::VEC_ID, vec![ResolvedType::I64]),
    ];
    let fields: Vec<_> = types
        .into_iter()
        .enumerate()
        .map(|(position, ty)| {
            let mut field = template.clone();
            field.id = DeclarationId::new(format!("text.row.mixed.{position}"));
            field.name = format!("mixed_{position}");
            field.index = if reorder { 1 - position } else { position }
                .try_into()
                .unwrap();
            field.ty = ty;
            let mut declaration = metadata.clone();
            declaration.id = field.id.clone();
            declaration.owner = Some(id.clone());
            declaration.name = field.name.clone();
            index.declarations.insert(field.id.clone(), declaration);
            field
        })
        .collect();
    index.record_fields.insert(id, fields);
    index
}

fn foreign_origin() -> DeclarationIndex {
    let mut index = mixed(false);
    let first = index.record_fields[&DeclarationId::new("text.row")][0]
        .id
        .clone();
    index.declarations.get_mut(&first).unwrap().identity_origin =
        crate::hir::IdentityOrigin::CompilerOwned;
    index
}

fn missing() -> DeclarationIndex {
    let mut index = texty();
    index.record_fields.remove(&DeclarationId::new("text.row"));
    index
}

#[test]
fn memo_matches_original_verdicts_across_indexes_with_equal_ids_cold_and_warm() {
    let indexes = [
        plain(),
        texty(),
        invariant(),
        mixed(false),
        mixed(true),
        missing(),
        foreign_origin(),
    ];
    // (validate_use is Ok, runtime_admitted)
    let expected = [
        (true, false),
        (true, true),
        (false, false),
        (true, true),
        (false, false),
        (true, false),
        (false, false),
    ];
    let root = root();
    let lookups: Vec<_> = indexes
        .iter()
        .map(|index| Lookup::new(index).unwrap())
        .collect();
    for round in 0..3 {
        for ((lookup, index), (valid, runtime)) in lookups.iter().zip(&indexes).zip(expected) {
            let actual = lookup.validate_use(&root);
            let original = oracle::validate_use(&root, index);
            assert_eq!(actual.is_ok(), valid, "round {round}");
            assert_eq!(
                actual.as_ref().err().map(Diagnostic::json),
                original.as_ref().err().map(Diagnostic::json),
                "round {round}"
            );
            assert_eq!(
                lookup.runtime_admitted(&root).unwrap(),
                runtime,
                "round {round}"
            );
            assert_eq!(runtime, oracle::runtime_admitted(&root, index));
        }
    }
}

#[test]
fn runtime_observation_is_evaluated_only_when_text_is_present() {
    let index = plain();
    let lookup = Lookup::new(&index).unwrap();
    lookup.validate_use(&root()).unwrap();
    let state = lookup.state.borrow();
    assert_eq!(state.entries.len(), 1);
    assert_eq!(
        (state.entries[0].contains, state.entries[0].runtime),
        (Some(false), None)
    );
}

#[test]
fn constructor_pays_exactly_the_rc_control_block_and_refuses_one_short() {
    let index = fixture();
    let paid = check(
        true,
        || {
            let lookup = Lookup::new(&index)?;
            drop(lookup);
            Ok(true)
        },
        "constructor",
    );
    assert_eq!(paid, Lookup::control_bytes());
    let ((), overflow) = with_limit(0, || {
        assert!(Lookup::new(&index).is_err());
        assert!(Lookup::new(&index).is_err());
        assert!(!crate::bounded_output::reserve_active_required(0));
    });
    assert!(overflow);
}

#[test]
fn cold_request_is_funded_before_acquisition_and_physically_bounded() {
    let index = texty();
    let root = root();
    let paid = check(
        true,
        || {
            let lookup = Lookup::new(&index)?;
            lookup.validate_use(&root)?;
            Ok(true)
        },
        "cold",
    );
    assert!(paid > Lookup::control_bytes());
}

#[test]
fn refused_cold_request_keeps_only_successful_observations_and_stays_sticky() {
    let index = texty();
    let root = root();
    let probe = Lookup::new(&index).unwrap();
    let (_, overflow, used) = with_limit_usage(LIMIT, || probe.validate_use(&root).unwrap());
    assert!(!overflow && used > 0);
    for limit in 0..used {
        let lookup = Lookup::new(&index).unwrap();
        let (refused, overflow) = with_limit(limit, || lookup.validate_use(&root).is_err());
        assert!(refused && overflow, "limit {limit} of {used}");
        // Containment may have completed and been paid before runtime admission
        // refused. That pure observation remains valid; no failed runtime result
        // may enter the directory.
        let state = lookup.state.borrow();
        for entry in &state.entries {
            assert_eq!(entry.contains, Some(true));
            assert_eq!(entry.runtime, None);
        }
        drop(state);
    }
    let lookup = Lookup::new(&index).unwrap();
    assert!(lookup.validate_use(&root).is_ok());
}

#[test]
fn warm_hit_is_zero_debit_allocation_free_and_still_sticky() {
    let index = texty();
    let root = root();
    let lookup = Lookup::new(&index).unwrap();
    let (_, overflow, used) = with_limit_usage(LIMIT, || lookup.validate_use(&root).unwrap());
    assert!(!overflow && used > 0);
    let (((), metrics), overflow, used) =
        with_limit_usage(LIMIT, || measure(|| lookup.validate_use(&root).unwrap()));
    assert!(!overflow && !metrics.invalid);
    assert_eq!((used, metrics.allocated), (0, 0));
    let ((), overflow) = with_limit(1, || {
        assert!(!crate::bounded_output::reserve_active_required(2));
        // Both observations are cached, yet neither hit succeeds after refusal.
        assert!(lookup.validate_use(&root).is_err());
        assert!(lookup.runtime_admitted(&root).is_err());
    });
    assert!(overflow);
    assert!(lookup.validate_use(&root).is_ok());
}

#[test]
fn lookup_clones_share_one_memo() {
    let index = texty();
    let root = root();
    let lookup = Lookup::new(&index).unwrap();
    let clone = lookup.clone();
    assert!(Rc::ptr_eq(&lookup.state, &clone.state));
    clone.validate_use(&root).unwrap();
    assert_eq!(lookup.owned_capacity(), clone.owned_capacity());
    let (((), metrics), overflow, used) =
        with_limit_usage(LIMIT, || measure(|| lookup.validate_use(&root).unwrap()));
    assert!(!overflow && !metrics.invalid);
    assert_eq!((used, metrics.allocated), (0, 0));
}

#[test]
fn directory_growth_is_funded_sorted_and_hits_cost_nothing() {
    let mut index = texty();
    for n in 0..40 {
        add_record(
            &mut index,
            &format!("text.many.{n:02}"),
            vec![ResolvedType::String],
        );
    }
    let roots: Vec<_> = (0..40)
        .map(|n| nominal(&format!("text.many.{n:02}"), Vec::new()))
        .collect();
    let mut classifier_paid = 0;
    for ty in &roots {
        let (_, overflow, used) = with_limit_usage(LIMIT, || {
            assert!(paid::contains_string(ty, &index).unwrap());
            assert!(paid::runtime_admitted(ty, &index).unwrap());
        });
        assert!(!overflow);
        classifier_paid += used;
    }
    let expected = Lookup::control_bytes()
        + classifier_paid
        + (4 + 8 + 16 + 32 + 64) * std::mem::size_of::<Entry<'_>>();
    let used = check(
        true,
        || {
            let lookup = Lookup::new(&index)?;
            for n in 0..40 {
                lookup.validate_use(&roots[(n * 7) % 40])?;
            }
            let state = lookup.state.borrow();
            assert_eq!(state.entries.len(), 40);
            assert!(state.entries.windows(2).all(|pair| pair[0].id < pair[1].id));
            drop(state);
            drop(lookup);
            Ok(true)
        },
        "all directory growth through Drop",
    );
    assert_eq!(used, expected);
    let lookup = Lookup::new(&index).unwrap();
    for ty in &roots {
        lookup.validate_use(ty).unwrap();
    }
    let ((), overflow, used) = with_limit_usage(LIMIT, || {
        for ty in &roots {
            lookup.validate_use(ty).unwrap();
        }
    });
    assert_eq!((overflow, used), (false, 0));
}

#[test]
fn generic_callable_and_unknown_types_stay_uncached_and_exact() {
    let index = texty();
    let lookup = Lookup::new(&index).unwrap();
    let base = lookup.owned_capacity();
    let function = ResolvedType::Function {
        parameters: vec![ResolvedType::String],
        result: Box::new(ResolvedType::Bool),
    };
    for ty in [
        nominal("text.box", vec![ResolvedType::String]),
        nominal("text.box", vec![]),
        nominal("missing.id", vec![]),
        function,
        ResolvedType::String,
    ] {
        let actual = lookup.validate_use(&ty);
        let original = oracle::validate_use(&ty, &index);
        assert_eq!(
            actual.as_ref().err().map(Diagnostic::json),
            original.as_ref().err().map(Diagnostic::json)
        );
        assert_eq!(
            lookup.runtime_admitted(&ty).unwrap(),
            oracle::runtime_admitted(&ty, &index)
        );
    }
    assert_eq!(lookup.owned_capacity(), base);
    assert!(lookup.state.borrow().entries.is_empty());
}

#[test]
fn actual_validator_clone_and_nested_constructor_share_exact_index_memo() {
    let source = crate::parse("module text.validator; @id(\"text.row\") record Row { @id(\"text.row.field\") field: String, } @id(\"text.main\") fn main() -> i64 { 0 }", std::path::Path::new("text-validator.spx")).unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let other = program.clone();
    let validator = HirValidator::new(&program).unwrap();
    let clone = validator.clone();
    assert!(Rc::ptr_eq(
        &validator.executable_profiles.state,
        &clone.executable_profiles.state
    ));
    clone.executable_profiles.validate_use(&root()).unwrap();
    let nested =
        HirValidator::with_profiles(&program, None, None, validator.executable_profiles.clone())
            .unwrap();
    assert!(Rc::ptr_eq(
        &validator.executable_profiles.state,
        &nested.executable_profiles.state
    ));
    assert!(
        HirValidator::with_profiles(&other, None, None, validator.executable_profiles.clone())
            .is_err()
    );
}

#[test]
fn retained_opaque_identity_with_utf8_and_nul_matches_full_original() {
    let mut index = fixture();
    let id = "text.é\0row";
    add_record(&mut index, id, vec![ResolvedType::String]);
    let ty = nominal(id, Vec::new());
    let lookup = Lookup::new(&index).unwrap();
    for _ in 0..2 {
        assert_eq!(
            lookup.observe(&ty, Observation::Contains).unwrap(),
            oracle::contains_string(&ty, &index)
        );
        assert_eq!(
            lookup.runtime_admitted(&ty).unwrap(),
            oracle::runtime_admitted(&ty, &index)
        );
    }
    let key = &lookup.state.borrow().entries[0];
    assert_eq!(key.id.as_str(), id);
}
