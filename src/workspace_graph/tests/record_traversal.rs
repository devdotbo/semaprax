use super::*;

pub(super) fn canonical_source(path: &str, source: &str) -> WorkspaceSource {
    let program = crate::parse(source, Path::new(path)).expect("test source must parse");
    WorkspaceSource {
        path: path.to_owned(),
        source: format::canonical(&program),
    }
}

fn sources() -> Vec<WorkspaceSource> {
    vec![
        canonical_source(
            "library.spx",
            r#"module library;
@id("library.row") record Row { @id("library.row.left") left:i64, @id("library.row.right") right:i64, }
@id("library.zero") fn zero()->i64 { 0 }
"#,
        ),
        canonical_source(
            "app.spx",
            r#"module app;
use type @id("library.row") from library as Row;
@id("app.walk") fn walk(rows:borrow Vec<Row>)->i64 {
 for borrow row at index in rows { let selected=row.left; let numbered=index; 0 }
 0
}
@id("app.main") fn main()->i64 {0}
"#,
        ),
    ]
}
#[test]
fn imported_record_row_length_and_read_edges_replay_source_exactly() {
    let build = build_owned(sources()).unwrap();
    let sites = build
        .edges
        .iter()
        .filter(|edge| edge.caller == "app.walk" && edge.kind == "type_reference")
        .map(|edge| edge.ast_path.as_str())
        .collect::<Vec<_>>();
    assert!(sites.contains(&"body.s0.value.s0.value.type_argument.0"));
    assert!(sites.contains(&"body.s0.value.s2.body.s1.value.s0.value.base.type_argument.0"));
    let source_programs = sources()
        .iter()
        .map(|source| crate::parse(&source.source, Path::new(&source.path)).unwrap())
        .collect::<Vec<_>>();
    super::super::record_traversal::validate(&source_programs, &build.hir.modules).unwrap();
}
#[test]
fn same_typed_field_substitution_cannot_rebind_authored_row_projection() {
    let source_programs = sources()
        .iter()
        .map(|source| crate::parse(&source.source, Path::new(&source.path)).unwrap())
        .collect::<Vec<_>>();
    let mut build = build_owned(sources()).unwrap();
    let function = build
        .hir
        .modules
        .iter_mut()
        .find(|module| module.module == "app")
        .unwrap()
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.walk")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("body")
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("wrapper")
    };
    let hir::ResolvedExprKind::Block { statements, .. } = &mut value.kind else {
        panic!("region")
    };
    let hir::ResolvedStatement::While { body, .. } = &mut statements[2] else {
        panic!("while")
    };
    let hir::ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("iteration")
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[1] else {
        panic!("authored")
    };
    let hir::ResolvedExprKind::Block { statements, .. } = &mut value.kind else {
        panic!("authored block")
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("selected")
    };
    let hir::ResolvedExprKind::Project { field, .. } = &mut value.kind else {
        panic!("row field")
    };
    *field = hir::DeclarationId::new("library.row.right");
    assert_eq!(
        super::super::record_traversal::validate(&source_programs, &build.hir.modules).unwrap_err()
            [0]
        .code,
        "SPX-G173"
    );
}
