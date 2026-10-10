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
    let sources = [
        ("flat.spx", SOURCE),
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
    let build = build();
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
    assert!(
        build
            .source_graph_schemas()
            .unwrap_err()
            .iter()
            .any(|error| error.code == "SPX-G410"),
        "constructor/index support cannot select a record replay route before its physical closure"
    );
}
#[test]
fn captured_record_layout_namespace_and_constructor_body_drift_are_independently_rejected() {
    for attack in 0..6 {
        let mut build = build();
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
                let hir::ResolvedExprKind::Block { tail, .. } = &mut make.body.kind else {
                    panic!("make block")
                };
                let hir::ResolvedExprKind::ConstructRecord { fields, .. } = &mut tail.kind else {
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
