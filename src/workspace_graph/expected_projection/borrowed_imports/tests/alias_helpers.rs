//! Alias-aware import clone helpers against the complete old clone-then-
//! overwrite sequence, observed through the real allocator and caller ledger.
use super::super::alias_clone;
use super::{import_stub, reserve_builder_structure, SCALAR_PROVIDER};
use crate::ast::{
    Expr, ExprKind, Function, Program, ResourceLifecycleDeclaration, ResourceLifecycleKind,
    SessionProtocolFollowsClause, Type, TypeDeclaration, TypeDeclarationKind,
    TypeParameterDeclaration, VariantCaseDeclaration, YieldsClause,
};
use crate::bounded_output::{
    active_refused, budgeted_clone, with_limit_usage_refusal, ReservationRefusal,
};
use crate::test_allocation_observer::{measure, Metrics};

const RECORD_PROVIDER: &str = r#"module imported.provider; @id("row") record Row { @id("row.x") x:i64, } requires x>=0 @id("helper") fn helper(value:i64)->i64 {value}"#;
const GENERIC_PROVIDER: &str =
    r#"module imported.provider; @id("helper") fn helper<T>(value:T)->T {value}"#;
const ALIAS: &str = "Imported\0\u{2603}Alias";

fn program(source: &str) -> Program {
    crate::parse(source, "provider.spx").unwrap()
}

struct Observed {
    same: bool,
    metrics: Metrics,
    refused: bool,
    used: usize,
    first: Option<ReservationRefusal>,
}

// Build, compare and Drop the complete output inside the observer.
fn observe<T: PartialEq>(limit: usize, expected: &T, build: impl Fn() -> T) -> Observed {
    let ((same, metrics), refused, used, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let output = build();
            let same = *expected == output;
            if active_refused() {
                assert!(reserve_builder_structure(0).is_err());
            }
            drop(output);
            same
        })
    });
    Observed {
        same,
        metrics,
        refused,
        used,
        first,
    }
}

fn compare<T: PartialEq>(
    label: &str,
    original_name: &str,
    alias: &str,
    expected: &T,
    old: impl Fn() -> T,
    new: impl Fn() -> T,
) {
    assert_ne!(
        alias, original_name,
        "{label}: alias must replace a distinct name"
    );
    let (cloned, refused, alias_debit, _) =
        with_limit_usage_refusal(usize::MAX, || budgeted_clone(alias));
    drop(cloned);
    assert!(
        !refused && alias_debit > 0,
        "{label}: alias debit {alias_debit}"
    );
    let before = observe(usize::MAX, expected, &old);
    let after = observe(usize::MAX, expected, &new);
    for run in [&before, &after] {
        assert!(
            run.same && !run.refused && !run.metrics.invalid && run.metrics.live == 0,
            "{label}: {:?}",
            run.metrics
        );
        assert_eq!(run.metrics.allocated, run.metrics.freed, "{label}");
        assert_eq!(run.used, alias_debit, "{label}: same alias paid debit");
    }
    let saved = original_name.len();
    assert_eq!(
        before.metrics.allocated,
        after.metrics.allocated + saved,
        "{label}: old {:?} new {:?}",
        before.metrics,
        after.metrics
    );
    assert_eq!(before.metrics.freed, after.metrics.freed + saved, "{label}");
    assert_eq!(
        before.metrics.allocations,
        after.metrics.allocations + usize::from(saved != 0),
        "{label}: one allocation per nonempty original name"
    );
    assert_eq!(
        before.metrics.reallocations, after.metrics.reallocations,
        "{label}"
    );
    assert!(
        after.metrics.highwater <= before.metrics.highwater,
        "{label}"
    );
    // Exact, one-short and zero ledgers: the alias debit is the sticky refusal.
    for limit in [alias_debit, alias_debit - 1, 0] {
        let exact = limit >= alias_debit;
        let before = observe(limit, expected, &old);
        let after = observe(limit, expected, &new);
        for run in [&before, &after] {
            assert!(
                !run.metrics.invalid && run.metrics.live == 0,
                "{label}: {limit}"
            );
            assert_eq!(run.refused, !exact, "{label}: limit {limit}");
            assert_eq!(run.first.is_some(), !exact, "{label}: limit {limit}");
            if exact {
                assert!(run.same, "{label}");
                assert_eq!(run.used, alias_debit, "{label}");
            }
        }
        assert_eq!(before.used, after.used, "{label}: limit {limit}");
        assert_eq!(
            before.first, after.first,
            "{label}: exact sticky refusal receipt"
        );
        if let Some(first) = after.first {
            assert_eq!(first.requested, alias_debit, "{label}");
            assert_eq!(first.remaining, limit, "{label}");
        }
    }
}

fn aliased_type(original: &TypeDeclaration, alias: &str) -> TypeDeclaration {
    let mut copy = alias_clone::type_payload(original);
    copy.name = budgeted_clone(alias);
    copy
}

fn hostile_types() -> Vec<(&'static str, TypeDeclaration)> {
    let provider = program(RECORD_PROVIDER);
    let record = provider.types[0].clone();
    let function = provider.functions[0].clone();
    assert!(
        !record.invariants().is_empty(),
        "source record carries an invariant"
    );
    let TypeDeclarationKind::Record { fields } = record.kind.clone() else {
        panic!("record fixture")
    };
    let mut class = record.clone();
    class.stable_id = "class\0\u{2603}id".into();
    class.explicit_id = false;
    class.name = "Cl\0ass\u{1F600}".into();
    class.name_span = record.span;
    class.type_parameters = vec![TypeParameterDeclaration {
        name: "T\0".into(),
        span: record.span,
    }];
    class.kind = TypeDeclarationKind::Class {
        fields: fields.clone(),
        methods: vec![function],
    };
    class.extends = Some(Type::Named {
        name: "Base\0\u{2603}".into(),
        arguments: vec![Type::I64],
    });
    class.invariants = Some(Box::new(vec![Expr {
        kind: ExprKind::Int(7),
        span: record.span,
    }]));
    let mut variant = record.clone();
    variant.stable_id = "variant\0id".into();
    variant.name = "V\0ariant\u{2603}".into();
    variant.invariants = None;
    variant.kind = TypeDeclarationKind::Variant {
        cases: vec![VariantCaseDeclaration {
            stable_id: "case\0".into(),
            explicit_id: false,
            name: "C\u{2603}".into(),
            name_span: record.span,
            fields,
            span: record.span,
        }],
    };
    let mut resource = record.clone();
    resource.name = "Res\0".into();
    resource.invariants = None;
    resource.kind = TypeDeclarationKind::Resource {
        lifecycles: vec![
            ResourceLifecycleDeclaration {
                stable_id: Some("life\0".into()),
                kind: ResourceLifecycleKind::Imported {
                    import_key: "k\0\u{2603}".into(),
                },
                span: record.span,
            },
            ResourceLifecycleDeclaration {
                stable_id: None,
                kind: ResourceLifecycleKind::Trivial,
                span: record.span,
            },
        ],
    };
    let mut empty = record.clone();
    empty.name = String::new();
    vec![
        ("source record with invariant", record),
        ("hostile class", class),
        ("hostile variant", variant),
        ("hostile resource", resource),
        ("empty original name", empty),
    ]
}

fn hostile_functions() -> Vec<(&'static str, Function)> {
    let scalar = program(SCALAR_PROVIDER).functions[0].clone();
    assert!(!scalar.requires.is_empty() && !scalar.ensures.is_empty());
    let generic = program(GENERIC_PROVIDER).functions[0].clone();
    assert!(!generic.type_parameters.is_empty());
    let mut hostile = scalar.clone();
    hostile.stable_id = "fn\0\u{2603}id".into();
    hostile.explicit_id = false;
    hostile.name = "he\0lper\u{1F600}".into();
    hostile.name_span = scalar.span;
    hostile.yields = Some(YieldsClause {
        request_type: Type::Named {
            name: "Req\0☃".into(),
            arguments: vec![Type::I64],
        },
        response_type: Type::Bytes,
        span: scalar.span,
    });
    hostile.follows = Some(Box::new(SessionProtocolFollowsClause {
        protocol_id: "proto\0☃".into(),
        protocol_id_span: scalar.name_span,
        span: scalar.span,
    }));
    hostile.effects = vec!["io\0\u{2603}".into(), String::new()];
    hostile.type_parameters = generic.type_parameters.clone();
    hostile.body = Expr {
        kind: ExprKind::Int(9),
        span: scalar.span,
    };
    let mut empty = scalar.clone();
    empty.name = String::new();
    vec![
        ("source scalar with contracts", scalar),
        ("source generic", generic),
        ("hostile metadata", hostile),
        ("empty original name", empty),
    ]
}

#[test]
fn type_helper_omits_only_the_discarded_original_name_allocation() {
    let cases = hostile_types();
    assert!(cases.iter().any(|(_, original)| !original.name.is_empty()));
    for (label, original) in cases {
        assert_ne!(ALIAS, original.stable_id, "{label}");
        let mut expected = original.clone();
        expected.name = ALIAS.to_string();
        let helper = aliased_type(&original, ALIAS);
        assert_eq!(helper, expected, "{label}");
        assert_eq!(helper.stable_id, original.stable_id, "{label}");
        assert_eq!(helper.explicit_id, original.explicit_id, "{label}");
        assert_eq!(helper.name_span, original.name_span, "{label}");
        assert_eq!(helper.extends, original.extends, "{label}");
        assert_eq!(helper.invariants, original.invariants, "{label}");
        assert_ne!(helper.name, helper.stable_id, "{label}");
        compare(
            label,
            &original.name,
            ALIAS,
            &expected,
            || {
                let mut copy = original.clone();
                copy.name = budgeted_clone(ALIAS);
                copy
            },
            || aliased_type(&original, ALIAS),
        );
    }
}

#[test]
fn retained_function_helper_omits_only_the_discarded_original_name_allocation() {
    let cases = hostile_functions();
    assert!(cases.iter().any(|(_, original)| !original.name.is_empty()));
    for (label, original) in cases {
        assert_ne!(ALIAS, original.stable_id, "{label}");
        let mut expected = original.clone();
        expected.name = ALIAS.to_string();
        let helper = alias_clone::retained_function(&original, ALIAS);
        assert_eq!(helper, expected, "{label}");
        assert_eq!(helper.stable_id, original.stable_id, "{label}");
        assert_eq!(helper.explicit_id, original.explicit_id, "{label}");
        assert_eq!(helper.name_span, original.name_span, "{label}");
        assert_eq!(helper.requires, original.requires, "{label}");
        assert_eq!(helper.ensures, original.ensures, "{label}");
        assert_eq!(helper.body, original.body, "{label}");
        compare(
            label,
            &original.name,
            ALIAS,
            &expected,
            || {
                let mut copy = original.clone();
                copy.name = budgeted_clone(ALIAS);
                copy
            },
            || alias_clone::retained_function(&original, ALIAS),
        );
    }
}

#[test]
fn signature_helper_drops_contracts_keeps_metadata_and_omits_original_name_allocation() {
    for (label, original) in hostile_functions() {
        let plain = import_stub::signature(&original);
        assert_eq!(
            plain.name, original.name,
            "{label}: plain signature unchanged"
        );
        let mut expected = plain;
        expected.name = ALIAS.to_string();
        let helper = alias_clone::signature_function(&original, ALIAS);
        assert_eq!(helper, expected, "{label}");
        assert_eq!(helper.stable_id, original.stable_id, "{label}");
        assert_eq!(helper.explicit_id, original.explicit_id, "{label}");
        assert_eq!(helper.name_span, original.name_span, "{label}");
        assert_eq!(helper.type_parameters, original.type_parameters, "{label}");
        assert_eq!(helper.params, original.params, "{label}");
        assert_eq!(helper.return_type, original.return_type, "{label}");
        assert_eq!(helper.effects, original.effects, "{label}");
        assert!(
            helper.yields.is_none() && helper.follows.is_none(),
            "{label}"
        );
        assert!(
            helper.requires.is_empty() && helper.ensures.is_empty(),
            "{label}"
        );
        assert_eq!(helper.body.kind, ExprKind::Int(0), "{label}");
        assert_eq!(helper.body.span, original.body.span, "{label}");
        assert_eq!(helper.span, original.span, "{label}");
        compare(
            label,
            &original.name,
            ALIAS,
            &expected,
            || {
                let mut copy = import_stub::signature(&original);
                copy.name = budgeted_clone(ALIAS);
                copy
            },
            || alias_clone::signature_function(&original, ALIAS),
        );
    }
}
