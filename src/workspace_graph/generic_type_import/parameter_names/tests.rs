//! Large hostile declarations and real acquisition/work frontiers.
use super::{BorrowedNames, Ordinals, INLINE};
use crate::ast::{Span, Type, TypeDeclarationKind, TypeParameterDeclaration};
use crate::bounded_output::{active_refused, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
use crate::workspace_graph::expected_projection::cost::generic_rewrite_test_bridge as cost_bridge;
use crate::workspace_graph::generic_type_import::{
    borrowed_tests::{contexts, record},
    original, rewrite_declaration, rewrite_declaration_runtime_cost, template_is_admitted,
};

fn parameters(count: usize) -> Vec<TypeParameterDeclaration> {
    (0..count)
        .rev()
        .map(|index| TypeParameterDeclaration {
            name: format!("P{index:06}"),
            span: Span::default(),
        })
        .collect()
}

#[test]
fn large_late_missing_duplicate_and_unicode_names_keep_old_membership_with_bounded_comparisons() {
    for count in [0, 1, 2, INLINE, INLINE + 1, 129, 4096] {
        let mut parameters = parameters(count);
        if count > 3 {
            parameters[count / 2].name = parameters[0].name.clone();
        }
        let (result, refused, _, _) =
            with_limit_usage_refusal(usize::MAX, || BorrowedNames::new(&parameters));
        let index = result.unwrap();
        assert!(!refused);
        let baseline: std::collections::BTreeSet<_> =
            parameters.iter().map(|p| p.name.as_str()).collect();
        let probes: [&str; 4] = ["P000000", "P999999", "", "雪"];
        for name in probes {
            assert_eq!(index.contains(name), baseline.contains(name));
        }
        for parameter in &parameters {
            assert!(index.contains(&parameter.name));
        }
        let logarithm = (usize::BITS - count.max(1).leading_zeros()) as usize;
        // Counts actual sort+lookup text comparisons, not a simulated search.
        // The old repeated linear scan exceeds this bound for the 4096 case.
        let bound = 32 * count.max(1) * logarithm.max(1) + (count + 4) * (logarithm + 1);
        assert!(
            index.comparisons.get() <= bound,
            "count={count}, actual={}, bound={bound}",
            index.comparisons.get()
        );
        assert_eq!(index.parameters.as_ptr(), parameters.as_ptr());
        match &index.ordinals {
            Ordinals::Heap(values) => assert_eq!(values.capacity(), count),
            Ordinals::Inline { .. } => assert!(count <= INLINE),
        }
    }
    let names = ["", "雪", "T", "雪", "e\u{301}", "é"];
    let parameters: Vec<_> = names
        .iter()
        .map(|name| TypeParameterDeclaration {
            name: (*name).into(),
            span: Span::default(),
        })
        .collect();
    let index = BorrowedNames::new(&parameters).unwrap();
    for name in ["", "雪", "T", "e\u{301}", "é", "E"] {
        assert_eq!(index.contains(name), names.contains(&name));
    }
}

#[test]
fn spill_backing_is_exact_paid_owned_and_short_zero_refusal_stays_g171_through_drop() {
    for count in [INLINE + 1, 129, 4096] {
        let parameters = parameters(count);
        let observe = |limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let result = BorrowedNames::new(&parameters);
                    let succeeded = result.as_ref().is_ok_and(|index| index.contains("P000000"));
                    if let Err(errors) = &result {
                        assert_eq!(errors.len(), 1);
                        assert_eq!(errors[0].code, "SPX-G171");
                    }
                    if active_refused() {
                        assert!(!crate::bounded_output::reserve_active_required(0));
                    }
                    drop(result);
                    succeeded
                })
            })
        };
        let ((ok, actual), refused, required, first) = observe(usize::MAX);
        assert!(ok && !refused && first.is_none());
        assert_eq!(
            required,
            std::mem::size_of::<BorrowedNames<'_>>() + count * std::mem::size_of::<usize>()
        );
        assert_eq!(actual.allocations, 1);
        assert_eq!(actual.allocated, count * std::mem::size_of::<usize>());
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
        for limit in [required, required - 1, 0] {
            let ((ok, actual), refused, used, first) = observe(limit);
            assert_eq!(ok, limit >= required);
            assert_eq!(refused, limit < required);
            assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
            if let Some(first) = first {
                assert_eq!(used, 0);
                assert_eq!(first.requested, required);
                assert_eq!(first.remaining, limit);
            } else {
                assert_eq!(used, required);
            }
        }
        // All three actual users propagate the storage error, never G172 or false.
        let mut declaration = record();
        declaration.type_parameters = parameters;
        let (caller, programs) = contexts();
        for route in 0..3 {
            let mut input = declaration.clone();
            let (result, refused, used, first) = with_limit_usage_refusal(0, || match route {
                0 => template_is_admitted(&input).map(|_| ()),
                1 => rewrite_declaration(&mut input, "fixture.provider", &caller, &programs),
                _ => rewrite_declaration_runtime_cost(
                    &input,
                    "fixture.provider",
                    &caller,
                    &programs,
                    &mut cost_bridge::new(),
                ),
            });
            let errors = result.unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, "SPX-G171");
            assert!(refused && used == 0 && first.is_some());
        }
    }
}

#[test]
fn complete_large_declared_generic_source_and_original_caller_keep_paid_g171_order() {
    let count = 4096;
    let mut text = String::from("module parameter.wide; @id(\"wide\") record Wide<");
    for index in 0..count {
        if index != 0 {
            text.push(',');
        }
        text.push_str(&format!("P{index:06}"));
    }
    text.push_str("> {");
    // One type + 4094 fields + one function reaches the existing 4096 declaration bound.
    for index in 0..4094 {
        text.push_str(&format!("@id(\"wide.f{index}\") f{index}:P004095,"));
    }
    text.push_str("} @id(\"main\") fn main()->i64 {0}");
    let program = crate::check(&text, "wide.spx").unwrap();
    let declaration = &program.types[0];
    let (result, refused, _, _) =
        with_limit_usage_refusal(usize::MAX, || template_is_admitted(declaration));
    assert_eq!(result.unwrap(), original::template_is_admitted(declaration));
    assert!(!refused);
    let mut missing = declaration.clone();
    let TypeDeclarationKind::Record { fields } = &mut missing.kind else {
        unreachable!()
    };
    fields.last_mut().unwrap().ty = Type::Named {
        name: "Missing".into(),
        arguments: vec![],
    };
    assert_eq!(
        template_is_admitted(&missing).unwrap(),
        original::template_is_admitted(&missing)
    );
    assert!(!template_is_admitted(&missing).unwrap());
    let provider = crate::parse(
        "module parameter.provider; @id(\"row\") record Row<T> { @id(\"row.x\") x:T, }",
        "provider.spx",
    )
    .unwrap();
    let caller=crate::parse("module parameter.caller; use type @id(\"row\") from parameter.provider as Imported; @id(\"main\") fn main()->i64 {0}","caller.spx").unwrap();
    let mut provider = provider;
    provider.types[0].type_parameters = parameters(129);
    let TypeDeclarationKind::Record { fields } = &mut provider.types[0].kind else { unreachable!() };
    fields[0].ty = Type::Named {name:"P000000".into(),arguments:vec![]};
    assert!(original::template_is_admitted(&provider.types[0]));
    let TypeDeclarationKind::Record { fields } = &mut provider.types[0].kind else { unreachable!() };
    fields[0].ty = Type::Named {name:"P000000".into(),arguments:vec![]};
    assert!(original::template_is_admitted(&provider.types[0]));
    let TypeDeclarationKind::Record { fields } = &mut provider.types[0].kind else { unreachable!() };
    fields[0].ty = Type::Named {name:"P000000".into(),arguments:vec![]};
    assert!(original::template_is_admitted(&provider.types[0]));
    let TypeDeclarationKind::Record { fields } = &mut provider.types[0].kind else { unreachable!() };
    fields[0].ty = Type::Named {name:"P000000".into(),arguments:vec![]};
    assert!(original::template_is_admitted(&provider.types[0]));
    let programs = vec![provider, caller];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let target = authored.get("row").unwrap();
    let import = &programs[1].module_uses[0];
    let (result, refused, _, first) = with_limit_usage_refusal(0, || {
        crate::workspace_graph::validate_imported_type(
            &programs[1],
            import,
            target,
            &authored,
            &programs,
        )
    });
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-G171");
    assert!(refused && first.is_some());
}
