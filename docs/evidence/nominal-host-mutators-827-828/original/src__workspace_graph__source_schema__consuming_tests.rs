use super::tests::build_source;
use super::*;
const SOURCE: &str = r#"module app.main;
@id("row") record Row {@id("row.n") n:i64,@id("row.text") text:string,@id("row.bytes") bytes:Bytes,}
@id("main") fn main()->i64 {
 let mut words=vec_with_capacity<Row>(2usize);
 let later=vec_with_capacity<Row>(1usize);
 let first=vec_push<Row>(vec_with_capacity<Row>(2usize),Row{n:7,text:"ab",bytes:bytes_zeroed(1usize)});
 let inputs=vec_push<Row>(first,Row{n:7,text:"ab",bytes:bytes_zeroed(1usize)});
 for own item in vec_into_iter<Row>(inputs) {words=vec_push<Row>(words,item);0}
 if vec_len<Row>(words)==2usize && vec_len<Row>(later)==0usize {29}else{0}
}
"#;
#[test]
fn retained_consuming_renewal_rederives_record_layout_and_frozen_parts_refuse() {
    let build = build_source(SOURCE);
    assert_eq!(
        build.source_graph_schemas().unwrap()["app/main.spx"],
        "semaprax.graph.v79"
    );
    let module = &build.hir.modules[0];
    assert_eq!(
        graph::graph_schema_from_parts_and_instances(
            &module.interfaces,
            &module.types,
            &module.functions,
            &module.function_templates,
            &module.function_instances
        )
        .unwrap_err()
        .code,
        "SPX-G410"
    );
    assert_eq!(
        build.into_change_view().unwrap().modules[0].source_graph_schema,
        "semaprax.graph.v79"
    );
}
#[test]
fn higher_consuming_schema_cannot_mask_forged_record_origin_layout_or_cleanup() {
    for mutation in 0..5 {
        let mut build = build_source(SOURCE);
        match mutation {
            0 => {
                build.hir.declarations.get_mut("row").unwrap().origin =
                    hir::IdentityOrigin::Automatic
            }
            1 => {
                let hir::ResolvedTypeDeclarationKind::Record { fields } =
                    &mut build.hir.modules[0].types[0].kind
                else {
                    panic!("record")
                };
                fields[1].ty = hir::ResolvedType::I64;
            }
            2 => build.hir.modules[0]
                .signature_types
                .values_mut()
                .find(|(kind, _)| *kind == hir::DeclarationKind::Record)
                .unwrap()
                .1
                .layout_key
                .push('!'),
            3 => {
                build.hir.modules[0].functions[0].cleanup_plan.schema =
                    crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V13
            }
            _ => {
                let record = build.hir.modules[0].types[0].clone();
                build.hir.modules[1].types.push(record);
            }
        }
        assert_eq!(
            build.source_graph_schemas().unwrap_err()[0].code,
            "SPX-G410",
            "mutation {mutation}"
        );
    }
}
