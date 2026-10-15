//! Complete existing native-resource fixture constructors, copied verbatim.
use super::*;
use crate::ast::Span;
use crate::hir::{
    OwnershipMode, ResolvedImport, ResolvedImportFailure, ResolvedImportParameter,
    ResolvedImportResult, ResolvedImportResultKind, ResolvedInterface, ResolvedResourceDrop,
    ResolvedResourceDropKind, ResolvedTypeDeclaration, ResolvedTypeDeclarationKind,
};
fn import(id: &str, interface: &str) -> ResolvedImport {
    ResolvedImport {
        id: DeclarationId::new(id),
        name: id.rsplit('.').next().unwrap().to_owned(),
        interface: DeclarationId::new(interface),
        import_key: format!("{id}.key"),
        native_rust: false,
        index_selected: false,
        selected_index_digest: None,
        selected_receiver: None,
        rust_path: None,
        parameters: Vec::new(),
        result: ResolvedImportResult {
            kind: ResolvedImportResultKind::Unit,
            ownership: OwnershipMode::Value,
            producer: "",
            out_slot_initialization: "",
            ownership_transfer: "",
        },
        effects: Vec::new(),
        required_authority: Vec::new(),
        failure: ResolvedImportFailure::Infallible,
        span: Span::default(),
    }
}

fn parameter(ty: ResolvedType, ownership: OwnershipMode) -> ResolvedImportParameter {
    ResolvedImportParameter {
        name: "value".to_owned(),
        ty,
        ownership,
        consumes_on_failure: ownership == OwnershipMode::Own,
    }
}

fn resource(id: &str, name: &str, finalizer: &ResolvedImport) -> ResolvedTypeDeclaration {
    ResolvedTypeDeclaration {
        id: DeclarationId::new(id),
        name: name.to_owned(),
        type_parameters: Vec::new().into(),
        kind: ResolvedTypeDeclarationKind::Resource {
            drop: ResolvedResourceDrop {
                id: DeclarationId::new(format!("{id}.drop")),
                kind: ResolvedResourceDropKind::Imported {
                    import: finalizer.id.clone(),
                    import_key: finalizer.import_key.clone(),
                },
            },
        },
        span: Span::default(),
    }
}

fn nominal(id: &DeclarationId) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: id.clone(),
        arguments: Vec::new(),
    }
}

/// An RI-06 Regex owner and an indexed native owner whose constructor,
/// method, and finalizer imports are split across two module interfaces.
fn resource_fixture() -> (Vec<ResolvedTypeDeclaration>, Vec<ResolvedInterface>) {
    let interface = |id: &str, imports| ResolvedInterface {
        id: DeclarationId::new(id),
        name: id.to_owned(),
        permits: Vec::new(),
        imports,
        span: Span::default(),
    };
    let mut regex_drop = import("reach.native.regex_drop", "reach.native.first");
    let mut owner_drop = import("reach.native.owner_drop", "reach.native.first");
    let regex = resource("reach.regex", "Regex", &regex_drop);
    let owner = resource("reach.owner", "Owner", &owner_drop);
    regex_drop.parameters = vec![parameter(nominal(&regex.id), OwnershipMode::Own)];
    owner_drop.parameters = vec![parameter(nominal(&owner.id), OwnershipMode::Own)];
    let native = |id: &str, path: &str, receiver: Option<&str>| {
        let mut native = import(id, "reach.native.second");
        native.native_rust = true;
        native.index_selected = true;
        native.selected_index_digest = Some("digest".to_owned());
        native.selected_receiver = receiver.map(str::to_owned);
        native.rust_path = Some(path.to_owned());
        native
    };
    let mut regex_new = native("reach.native.regex_new", "regex_alias::Regex::new", None);
    regex_new.parameters = vec![parameter(ResolvedType::String, OwnershipMode::Borrow)];
    regex_new.result.kind = ResolvedImportResultKind::OwnedResultResourceI64 {
        resource: regex.id.clone(),
    };
    let mut regex_match = native(
        "reach.native.regex_match",
        "regex_alias::Regex::is_match",
        Some("shared"),
    );
    regex_match.parameters = vec![
        parameter(nominal(&regex.id), OwnershipMode::Borrow),
        parameter(ResolvedType::String, OwnershipMode::Borrow),
    ];
    regex_match.result.kind = ResolvedImportResultKind::Bool;
    let mut owner_new = native("reach.native.owner_new", "owner_alias::Owner::new", None);
    owner_new.parameters = vec![parameter(ResolvedType::I64, OwnershipMode::Value)];
    owner_new.result.kind = ResolvedImportResultKind::OwnedResource {
        resource: owner.id.clone(),
    };
    let mut owner_step = native(
        "reach.native.owner_step",
        "owner_alias::Owner::step",
        Some("owned"),
    );
    owner_step.parameters = vec![
        parameter(nominal(&owner.id), OwnershipMode::Own),
        parameter(ResolvedType::I64, OwnershipMode::Value),
    ];
    owner_step.result.kind = ResolvedImportResultKind::Bool;
    // Canonical identity order, matching the closure's publication order.
    (
        vec![owner, regex],
        vec![
            interface("reach.native.first", vec![regex_drop, owner_drop]),
            interface(
                "reach.native.second",
                vec![regex_new, regex_match, owner_new, owner_step],
            ),
        ],
    )
}

#[test]
fn original_native_constructor_universe_split_interfaces_and_late_digest_drift_keep_exact_bool() {
    let (types, interfaces) = resource_fixture();
    let mut program = super::fixtures().remove(0);
    program.types.extend(types);
    program.interfaces = interfaces;
    let regex = program
        .types
        .iter()
        .find(|item| item.id.as_str() == "reach.regex")
        .unwrap()
        .id
        .clone();
    let root = ResolvedType::Nominal {
        declaration: DeclarationId::new(crate::prelude::RESULT_ID),
        arguments: vec![nominal(&regex), ResolvedType::I64],
    };
    assert!(crate::hir::admitted_ri06_regex_result(&program, &root));
    for change in 0..5 {
        let mut subject = program.clone();
        if change == 1 {
            subject.interfaces[0].imports.clear();
        }
        if change == 2 {
            subject.interfaces[1].imports.retain(|import| {
                import.rust_path.as_deref() != Some("regex_alias::Regex::is_match")
            });
        }
        if change == 3 {
            for import in &mut subject.interfaces[1].imports {
                if import.rust_path.as_deref() == Some("regex_alias::Regex::is_match") {
                    import.selected_index_digest = Some("foreign".to_owned());
                }
            }
        }
        if change == 4 {
            let imports = &mut subject.interfaces[1].imports;
            let position = imports
                .iter()
                .position(|import| import.rust_path.as_deref() == Some("regex_alias::Regex::new"))
                .unwrap();
            let mut conflicting = imports[position].clone();
            conflicting.id = DeclarationId::new("conflicting.constructor");
            conflicting.selected_index_digest = Some("foreign".to_owned());
            imports.insert(position, conflicting);
        }
        let expected = crate::hir::admitted_ri06_regex_result(&subject, &root);
        assert_eq!(
            paid(&subject, |payer| super::super::native::result(
                &subject, &root, payer
            )),
            expected
        );
        assert_eq!(expected, change == 0);
    }
}
