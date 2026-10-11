use super::*;
use crate::cleanup::ReplayContext;
const SOURCE: &str = r#"module static.flat_records;
@id("flat.Packet") record Packet {
 @id("flat.Packet.n") n:i64,@id("flat.Packet.text") text:string,@id("flat.Packet.data") data:Bytes,
}
@id("flat.identity") fn identity(value:own Packet)->Packet {value}
@id("flat.make") fn make(input:borrow Slice<u8>)->Packet {
 Packet{n:7,text:"label",data:bytes_copy(input)}
}
@id("flat.read") fn read(value:borrow Packet)->i64 {value.n}
@id("flat.main") fn main()->i64 {let key=str"x";return str_len_bytes(key);}
"#;
fn build() -> WorkspaceGraphBuild {
    build_source(SOURCE)
}
fn lexical_source() -> String {
    SOURCE
        .replace("fn identity(value:own Packet)->Packet {value}",
            "fn identity(value:own Packet,flag:bool)->Packet {let mut next=0;while next<1 {next=next+1;if flag {return value;} else {0} 0} value}")
        .replace("Packet{n:7,text:\"label\",data:bytes_copy(input)}",
            "let packet=Packet{n:7,text:\"label\",data:bytes_copy(input)};let mut next=0;while next<1 {next=next+1;if byte_len(input)>0usize {return packet;} else {0} 0} packet")
        .replace("{let key=str\"x\";return str_len_bytes(key);}",
            "{let key=str\"x\";let input=[65u8];let packet=identity(make(array_as_slice(input)),false);let mut next=0;while next<1 {next=next+1;if str_len_bytes(key)==1 {return read(packet);} else {0} 0} read(packet)}")
}
fn build_lexical() -> WorkspaceGraphBuild {
    build_source(&lexical_source())
}
fn build_source(source: &str) -> WorkspaceGraphBuild {
    let sources = [
        ("flat.spx", source),
        (
            "support.spx",
            "module support.flat;@id(\"support.ready\") fn ready()->i64{0}",
        ),
    ]
    .into_iter()
    .map(|(path, source)| WorkspaceSource {
        path: path.to_owned(),
        source: format::canonical(&crate::parse(source, path).unwrap()),
    })
    .collect();
    build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES)
        .unwrap()
        .0
}
fn module(build: &WorkspaceGraphBuild) -> &WorkspaceResolvedModule {
    build
        .hir
        .modules
        .iter()
        .find(|module| module.module == "static.flat_records")
        .unwrap()
}
#[test]
fn captured_flat_layout_reconstructs_minimal_index_at_exact_and_one_short_without_admission() {
    let nonlexical = build();
    assert_eq!(
        nonlexical.source_graph_schemas().unwrap()["flat.spx"],
        "semaprax.graph.v77"
    );
    let mut reservation = crate::cleanup_plan::ReplayPhysicalReservation::new(usize::MAX);
    let refused = super::super::authenticate(
        module(&nonlexical),
        &nonlexical.hir.declarations,
        &mut reservation,
    )
    .err()
    .expect("ordinary graph admission grants no retained lexical authority");
    assert!(refused.iter().any(|error| error.code == "SPX-G410"));
    let build = build_lexical();
    assert_eq!(
        build.source_graph_schemas().unwrap()["flat.spx"],
        "semaprax.graph.v77"
    );
    let module = module(&build);
    let inventory = module
        .static_text
        .as_ref()
        .unwrap()
        .lexical
        .as_ref()
        .unwrap();
    assert_eq!(inventory.records.as_ref().unwrap().records.len(), 1);
    let mut physical = crate::cleanup_plan::ReplayPhysicalReservation::new(usize::MAX);
    let context =
        super::super::authenticate(module, &build.hir.declarations, &mut physical).unwrap();
    let record = module
        .types
        .iter()
        .find(|record| record.id.as_str() == "flat.Packet")
        .unwrap();
    let hir::ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        panic!("record")
    };
    assert_eq!(
        context.declarations().record_fields(&record.id).unwrap(),
        fields
    );
    for field in fields {
        let declared = context.declarations().declaration(&field.id).unwrap();
        assert_eq!(declared.kind, hir::DeclarationKind::Field);
        assert_eq!(declared.owner.as_ref(), Some(&record.id));
    }
    let required = physical.charged();
    assert!(required > 0);
    let (exact, refused, used) = crate::bounded_output::with_limit_usage(required, || {
        super::super::authenticate(
            module,
            &build.hir.declarations,
            &mut crate::cleanup_plan::ReplayPhysicalReservation::new(required),
        )
    });
    assert!(!refused);
    assert_eq!(used, required);
    assert!(exact.is_ok());
    let mut short = crate::cleanup_plan::ReplayPhysicalReservation::new(required - 1);
    assert!(super::super::authenticate(module, &build.hir.declarations, &mut short).is_err());
    assert!(short.reserve(0).is_err());
}
#[test]
fn captured_record_layout_namespace_and_constructor_body_drift_are_independently_rejected() {
    for attack in 0..6 {
        let mut build = build_lexical();
        assert_eq!(
            build.source_graph_schemas().unwrap()["flat.spx"],
            "semaprax.graph.v77"
        );
        super::super::authenticate(
            module(&build),
            &build.hir.declarations,
            &mut crate::cleanup_plan::ReplayPhysicalReservation::new(usize::MAX),
        )
        .unwrap();
        let mut facts = build.hir.declarations.clone();
        let module = build
            .hir
            .modules
            .iter_mut()
            .find(|module| module.module == "static.flat_records")
            .unwrap();
        let record = module
            .types
            .iter_mut()
            .find(|record| record.id.as_str() == "flat.Packet")
            .unwrap();
        let hir::ResolvedTypeDeclarationKind::Record { fields } = &mut record.kind else {
            panic!("record")
        };
        match attack {
            0 => fields.swap(0, 1),
            1 => fields[1].ty = hir::ResolvedType::Bytes,
            2 => fields[1].index = 0,
            3 => facts.get_mut("flat.Packet.text").unwrap().owner = None,
            4 => record.span.end = record.span.end.checked_add(1).unwrap(),
            _ => {
                let make = module
                    .functions
                    .iter_mut()
                    .find(|f| f.id.as_str() == "flat.make")
                    .unwrap();
                let hir::ResolvedExprKind::Block { statements, .. } = &mut make.body.kind else {
                    panic!("make block")
                };
                let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
                    panic!("source packet binding")
                };
                let hir::ResolvedExprKind::ConstructRecord { fields, .. } = &mut value.kind else {
                    panic!("source constructor")
                };
                let field = fields
                    .iter_mut()
                    .find(|f| f.field.as_str() == "flat.Packet.n")
                    .unwrap();
                field.value.kind = hir::ResolvedExprKind::Int(8);
            }
        }
        let result = super::super::authenticate(
            module,
            &facts,
            &mut crate::cleanup_plan::ReplayPhysicalReservation::new(usize::MAX),
        );
        let errors = match result {
            Ok(_) => panic!("stale record/source body authenticated"),
            Err(errors) => errors,
        };
        assert!(errors.iter().any(|error| error.code == "SPX-G410"));
    }
}

#[test]
fn source_bound_nominal_make_identity_read_with_real_lexical_results_selects() {
    let build = build_lexical();
    let module = module(&build);
    for id in ["flat.make", "flat.identity"] {
        let function = module
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap();
        assert!(matches!(
            function.return_type,
            hir::ResolvedType::Nominal { .. }
        ));
        assert!(hir::lexical_exit::requires(function).unwrap());
    }
    assert_eq!(
        build.source_graph_schemas().unwrap()["flat.spx"],
        "semaprax.graph.v77"
    );
}
#[test]
fn direct_record_replay_rejects_retained_field_epoch_and_finalizer_mutations() {
    let source=SOURCE.replace("{let key=str\"x\";return str_len_bytes(key);}",
        "{let key=str\"x\";let input=[65u8];let packet=identity(make(array_as_slice(input)));let mut next=0;while next<1 {next=next+1;if str_len_bytes(key)==1 {return read(packet);} else {0} 0} read(packet)}");
    for attack in 0..3 {
        let mut build = build_source(&source);
        assert_eq!(
            build.source_graph_schemas().unwrap()["flat.spx"],
            "semaprax.graph.v77"
        );
        let module = build
            .hir
            .modules
            .iter_mut()
            .find(|module| module.module == "static.flat_records")
            .unwrap();
        let function = module
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "flat.main")
            .unwrap();
        match attack {
            0 => {
                let slot = function
                    .cleanup_plan
                    .slots
                    .iter_mut()
                    .find(|slot| {
                        matches!(
                            slot.field_liveness_shape,
                            crate::cleanup::FieldLivenessShape::Record { .. }
                        )
                    })
                    .unwrap();
                let crate::cleanup::FieldLivenessShape::Record { fields, .. } =
                    &mut slot.field_liveness_shape
                else {
                    unreachable!()
                };
                fields[1].field_index = 0;
            }
            1 => {
                let slot = function
                    .cleanup_plan
                    .slots
                    .iter_mut()
                    .find(|slot| {
                        matches!(
                            slot.storage,
                            crate::cleanup_plan::StorageId::CallArgument { .. }
                        ) && matches!(slot.ty, hir::ResolvedType::Nominal { .. })
                    })
                    .unwrap();
                let crate::cleanup_plan::StorageId::CallArgument {
                    parameter_index, ..
                } = &mut slot.storage
                else {
                    unreachable!()
                };
                *parameter_index += 1;
            }
            _ => {
                let exit = function
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .find(|exit| exit.finalize_in_order.len() >= 2)
                    .unwrap();
                exit.finalize_in_order.swap(0, 1);
            }
        }
        assert!(
            build
                .source_graph_schemas()
                .unwrap_err()
                .iter()
                .any(|error| error.code == "SPX-G410"),
            "attack {attack}"
        );
    }
}

mod prepared;
