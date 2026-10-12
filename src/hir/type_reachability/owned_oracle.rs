//! OPT784 controls: the saved owned-map reachability algorithm as an
//! independent oracle for the borrowed exact-current inventory.
//!
//! `owned_reachable_types` is the pre-OPT784 body kept verbatim: it consumes a
//! fully cloned `BTreeMap` of every declaration, owned interface slices, and a
//! per-declaration field pointer vector. Only its storage is under test; the
//! shared expression/type walkers are the same production code.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::*;

pub(crate) fn owned_reachable_types(
    functions: &[LinkedScalarFunction],
    instances: &[ResolvedFunctionInstance],
    interfaces: &[ResolvedInterface],
    available: &BTreeMap<DeclarationId, ResolvedTypeDeclaration>,
    roots: &[DeclarationId],
    owner_interfaces: &[ResolvedInterface],
) -> Result<Vec<ResolvedTypeDeclaration>, Diagnostic> {
    let mut selected = roots.iter().cloned().collect::<BTreeSet<_>>();
    for linked in functions {
        collect_function(&linked.function, &mut selected);
    }
    for instance in instances {
        collect_function(&instance.function, &mut selected);
    }
    for interface in interfaces {
        for import in &interface.imports {
            for parameter in &import.parameters {
                collect_type(&parameter.ty, &mut selected);
            }
        }
    }

    let mut pending = selected.iter().cloned().collect::<VecDeque<_>>();
    while let Some(id) = pending.pop_front() {
        if crate::prelude::is_compiler_owned_id(id.as_str()) {
            continue;
        }
        let declaration = available.get(&id).ok_or_else(|| {
            Diagnostic::io(
                "SPX-G173",
                format!("owned-data closure references unknown type `{id}`"),
            )
        })?;
        let fields = match &declaration.kind {
            ResolvedTypeDeclarationKind::Record { fields } => fields.iter().collect::<Vec<_>>(),
            ResolvedTypeDeclarationKind::Variant { cases } => {
                cases.iter().flat_map(|case| &case.fields).collect()
            }
            ResolvedTypeDeclarationKind::Resource { .. }
                if workspace_link::native_owner::admitted_resource(
                    declaration,
                    owner_interfaces,
                ) || workspace_link::native_owner::admitted_ri06_regex_resource(
                    declaration,
                    if owner_interfaces.is_empty() {
                        interfaces
                    } else {
                        owner_interfaces
                    },
                ) =>
            {
                Vec::new()
            }
            ResolvedTypeDeclarationKind::Class { .. }
            | ResolvedTypeDeclarationKind::Resource { .. } => {
                return Err(Diagnostic::io(
                    "SPX-G172",
                    format!(
                        "owned-data closure type `{id}` is outside the shared record/variant target profile"
                    ),
                ));
            }
        };
        for field in fields {
            let mut referenced = BTreeSet::new();
            collect_type(&field.ty, &mut referenced);
            for referenced_id in referenced {
                if selected.insert(referenced_id.clone()) {
                    pending.push_back(referenced_id);
                }
            }
        }
    }

    selected
        .into_iter()
        .filter(|id| !crate::prelude::is_compiler_owned_id(id.as_str()))
        .map(|id| {
            available.get(&id).cloned().ok_or_else(|| {
                Diagnostic::io(
                    "SPX-G173",
                    format!("owned-data type closure lost declaration `{id}`"),
                )
            })
        })
        .collect()
}

fn owned_map(
    types: &[ResolvedTypeDeclaration],
) -> BTreeMap<DeclarationId, ResolvedTypeDeclaration> {
    let mut available = BTreeMap::new();
    for declaration in types {
        assert!(available
            .insert(declaration.id.clone(), declaration.clone())
            .is_none());
    }
    available
}

const DENSE: &str = r#"module reach.dense;
@id("reach.leaf") record Leaf {
  @id("reach.leaf.value") value: i64,
}
@id("reach.wrap") record Wrap<T> {
  @id("reach.wrap.value") value: T,
}
@id("reach.pair") record Pair<T, U> {
  @id("reach.pair.left") left: T,
  @id("reach.pair.right") right: U,
}
@id("reach.mode") variant Mode {
  @id("reach.mode.held") Held {
    @id("reach.mode.held.value") value: Wrap<Pair<Leaf, i64>>,
  },
  @id("reach.mode.empty") Empty,
}
@id("reach.outer") record Outer {
  @id("reach.outer.mode") mode: Mode,
  @id("reach.outer.wraps") wraps: Pair<Wrap<Leaf>, Wrap<bool>>,
}
@id("reach.unused") record Unused {
  @id("reach.unused.value") value: i64,
}
@id("reach.main") fn main() -> i64 { 0 }
"#;

fn dense_program() -> ResolvedProgram {
    let source = crate::parse(DENSE, std::path::Path::new("reach-dense.spx")).unwrap();
    crate::hir::resolve(&source).unwrap()
}

fn linked_functions(program: &ResolvedProgram) -> Vec<LinkedScalarFunction> {
    program
        .functions
        .iter()
        .map(|function| LinkedScalarFunction {
            function: function.clone(),
            origin: program
                .declarations
                .declaration(&function.id)
                .unwrap()
                .identity_origin,
        })
        .collect()
}

#[test]
fn borrowed_reachability_matches_owned_oracle_for_nested_generic_dense_roots() {
    let program = dense_program();
    let functions = linked_functions(&program);
    let owned = owned_map(&program.types);
    let borrowed = BorrowedTypeInventory::collect(&program.types).unwrap();
    assert_eq!(borrowed.len(), owned.len());
    let mut root_sets = vec![Vec::new()];
    root_sets.extend(
        program
            .types
            .iter()
            .map(|declaration| vec![declaration.id.clone()]),
    );
    root_sets.push(program.types.iter().map(|d| d.id.clone()).collect());
    for roots in &root_sets {
        let actual = reachable_authored_types_with_roots(
            &functions,
            &program.function_instances,
            &[],
            &borrowed,
            roots,
        );
        let expected = owned_reachable_types(
            &functions,
            &program.function_instances,
            &[],
            &owned,
            roots,
            &[],
        );
        assert_eq!(actual, expected, "roots {roots:?}");
    }
    // Independent of both implementations: the exact transitive record,
    // variant-case, and nested generic-argument closure of `Outer`.
    let outer = reachable_authored_types_with_roots(
        &functions,
        &program.function_instances,
        &[],
        &borrowed,
        &[DeclarationId::new("reach.outer")],
    )
    .unwrap();
    assert_eq!(
        outer.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
        [
            "reach.leaf",
            "reach.mode",
            "reach.outer",
            "reach.pair",
            "reach.wrap"
        ]
    );
    for declaration in &outer {
        assert_eq!(Some(declaration), owned.get(&declaration.id));
    }
}

#[test]
fn unknown_root_and_duplicate_identity_refusals_match_owned_oracle() {
    let program = dense_program();
    let functions = linked_functions(&program);
    let owned = owned_map(&program.types);
    let borrowed = BorrowedTypeInventory::collect(&program.types).unwrap();
    let roots = [
        DeclarationId::new("reach.outer"),
        DeclarationId::new("reach.absent"),
    ];
    let actual = reachable_authored_types_with_roots(
        &functions,
        &program.function_instances,
        &[],
        &borrowed,
        &roots,
    )
    .unwrap_err();
    let expected = owned_reachable_types(
        &functions,
        &program.function_instances,
        &[],
        &owned,
        &roots,
        &[],
    )
    .unwrap_err();
    assert_eq!(actual, expected);
    assert_eq!(actual.code, "SPX-G173");
    assert!(actual.message.contains("`reach.absent`"));

    // An unreachable repeated identity is refused before any closure walk.
    let unused = program
        .types
        .iter()
        .find(|declaration| declaration.id.as_str() == "reach.unused")
        .unwrap();
    let repeated = program.types.iter().chain(std::iter::once(unused));
    assert_eq!(
        BorrowedTypeInventory::collect(repeated).err(),
        Some(super::inventory::DuplicateTypeIdentity)
    );
}

#[test]
fn class_profile_refusal_matches_owned_oracle() {
    let source = crate::parse(
        r#"module reach.class;
@id("reach.class.held") class Held {
  @id("reach.class.held.value") value: i64,
}
@id("reach.class.main") fn main() -> i64 { 0 }
"#,
        std::path::Path::new("reach-class.spx"),
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let functions = linked_functions(&program);
    let owned = owned_map(&program.types);
    let borrowed = BorrowedTypeInventory::collect(&program.types).unwrap();
    let roots = [DeclarationId::new("reach.class.held")];
    let actual = reachable_authored_types_with_roots(
        &functions,
        &program.function_instances,
        &[],
        &borrowed,
        &roots,
    )
    .unwrap_err();
    let expected = owned_reachable_types(
        &functions,
        &program.function_instances,
        &[],
        &owned,
        &roots,
        &[],
    )
    .unwrap_err();
    assert_eq!(actual, expected);
    assert_eq!(actual.code, "SPX-G172");
}

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
        type_parameters: Vec::new(),
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
fn split_owner_interfaces_admit_exactly_like_concatenated_owned_oracle() {
    let (types, interfaces) = resource_fixture();
    let owned = owned_map(&types);
    let borrowed = BorrowedTypeInventory::collect(&types).unwrap();
    let roots = types.iter().map(|d| d.id.clone()).collect::<Vec<_>>();
    // Owner interfaces arrive as separate module slices in production.
    let (first, second) = interfaces.split_at(1);
    let split = first.iter().chain(second);
    let actual = reachable_types(&[], &[], &[], &borrowed, &roots, split).unwrap();
    let expected = owned_reachable_types(&[], &[], &[], &owned, &roots, &interfaces).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual, types);
    // No owner inventory: RI-06 falls back to the seeding interfaces, while
    // the indexed native owner (seeded by its import parameters) remains
    // outside the shared profile.
    let actual = reachable_types(&[], &[], &interfaces, &borrowed, &roots, std::iter::empty());
    let expected = owned_reachable_types(&[], &[], &interfaces, &owned, &roots, &[]);
    assert_eq!(actual, expected);
    assert_eq!(actual.unwrap_err().code, "SPX-G172");
    let regex_interfaces = interfaces
        .iter()
        .cloned()
        .map(|mut interface| {
            interface
                .imports
                .retain(|import| import.id.as_str().contains("regex"));
            interface
        })
        .collect::<Vec<_>>();
    let regex_only = [DeclarationId::new("reach.regex")];
    let actual = reachable_types(
        &[],
        &[],
        &regex_interfaces,
        &borrowed,
        &regex_only,
        std::iter::empty(),
    );
    let expected = owned_reachable_types(&[], &[], &regex_interfaces, &owned, &regex_only, &[]);
    assert_eq!(actual, expected);
    assert_eq!(actual.unwrap(), types[1..]);
    // Removing the second module's constructors refuses both owners alike.
    let actual = reachable_types(&[], &[], &[], &borrowed, &roots, first.iter());
    let expected = owned_reachable_types(&[], &[], &[], &owned, &roots, first);
    assert_eq!(actual, expected);
    assert_eq!(actual.unwrap_err().code, "SPX-G172");
}
