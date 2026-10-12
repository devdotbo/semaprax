use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

#[test]
fn paid_substitution_preserves_full_generic_callable_and_hostile_old_verdicts() {
    let owner = DeclarationId::new("template.owner");
    let foreign = DeclarationId::new("template.foreign");
    let parameter = |owner: &DeclarationId, index| ResolvedType::TypeParameter { owner: owner.clone(), index };
    let arguments = [ResolvedType::Function {
        parameters: vec![ResolvedType::String], result: Box::new(ResolvedType::I64),
    }];
    let templates = [
        ResolvedType::I64,
        parameter(&owner, 0),
        parameter(&owner, 1),
        parameter(&foreign, 0),
        ResolvedType::Nominal { declaration: DeclarationId::new("generic.record"), arguments: vec![parameter(&owner, 0)] },
        ResolvedType::Function { parameters: vec![parameter(&owner, 0), ResolvedType::Bytes], result: Box::new(parameter(&owner, 0)) },
    ];
    for template in &templates {
        let old = crate::hir::substitute_type(template, &owner, &arguments).ok();
        let (((), observed), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || measure(|| {
            let payer = CfgReservation::acquire().unwrap();
            let actual = substitute(template, &owner, &arguments, &payer).unwrap();
            assert_eq!(actual, old);
            assert_eq!(payer.first(), None);
            drop(actual);
            drop(payer);
        }));
        assert!(!refused);
        assert_eq!(first, None);
        assert!(observed.allocated <= paid);
        assert_eq!(observed.freed, observed.allocated);
        assert_eq!(observed.live, 0);
        assert!(!observed.invalid);
    }
    // A real generic clone fails before the first paid frame allocation;
    // physical refusal must never turn into the old semantic false verdict.
    let setup = std::mem::size_of::<CfgReservation>() + 46;
    let (((), observed), refused, _, first) = with_limit_usage_refusal(setup, || measure(|| {
        let payer = CfgReservation::acquire().unwrap();
        assert!(substitute(&templates[1], &owner, &arguments, &payer).is_err());
        let reason = payer.first().unwrap();
        assert_eq!(payer.reserve(0), Err(reason));
        drop(payer.diagnostic());
    }));
    assert!(refused);
    assert!(first.is_some());
    assert_eq!(observed.allocations, 1); // only the prepaid report
    assert_eq!(observed.freed, observed.allocated);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
}
