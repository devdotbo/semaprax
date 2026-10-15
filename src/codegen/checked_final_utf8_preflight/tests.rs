use super::*;

fn program() -> hir::ResolvedProgram {
    let source = r#"module checked.preflight;
@id("app.copy") fn copy()->string {
 let raw=[65u8,0u8,195u8,169u8];string_from_utf8(array_as_slice(raw))
}
@id("app.unrelated") fn unrelated()->string { "fatal literal" }
@id("app.main") fn main()->i64 {string_len(copy())}
"#;
    hir::resolve(&crate::parse(source, "checked-preflight.spx").unwrap()).unwrap()
}

#[test]
fn direct_constructor_closure_inventory_is_deterministic_and_does_not_select_emission() {
    let program = program();
    let before = super::super::emit_hir_c(&program).unwrap();
    let entry = DeclarationId::new("app.main");
    let inventory = inspect(&program, &entry).unwrap();
    assert_eq!(inventory, inspect(&program, &entry).unwrap());
    assert_eq!(
        inventory.functions,
        vec![entry, DeclarationId::new("app.copy")]
    );
    assert!(!inventory
        .sites
        .iter()
        .any(|site| site.kind == SiteKind::Literal));
    assert_eq!(
        inventory
            .sites
            .iter()
            .filter(|site| matches!(
                site.kind,
                SiteKind::FromUtf8 {
                    existing_propagated_failure: true
                }
            ))
            .count(),
        1
    );
    // The existing UTF8 conversion failure source is reported, never promoted
    // to allocation authority. Inspection leaves legacy emission byte-exact.
    assert_eq!(before, super::super::emit_hir_c(&program).unwrap());
    assert!(!before.contains("SPX_CHECKED_STRING_KERNEL_ONLY_V1"));
    assert!(inventory
        .sites
        .iter()
        .any(|site| matches!(site.kind, SiteKind::UnexpandedCall { .. })));
}

#[test]
fn source_name_and_owned_string_places_do_not_hide_fatal_materialization() {
    let parsed = crate::parse(
        r#"module checked.mixed;
@id("checked.generated.writer") fn writer()->string { let text="";text }
@id("app.main") fn main()->i64 {string_len(writer())}
"#,
        "mixed-preflight.spx",
    )
    .unwrap();
    let program = hir::resolve(&parsed).unwrap();
    let inventory = inspect(&program, &DeclarationId::new("app.main")).unwrap();
    assert_eq!(
        inventory
            .sites
            .iter()
            .filter(|site| site.kind == SiteKind::Literal)
            .count(),
        1
    );
    assert!(inventory
        .sites
        .iter()
        .any(|site| site.kind == SiteKind::StringPlace));
    // A Place may be a checked move or a clone. This report deliberately grants
    // neither; the selected profile must replay the exact transfer/drop facts.
    assert!(!inventory
        .sites
        .iter()
        .any(|site| matches!(site.kind, SiteKind::FromUtf8 { .. })));
}

#[test]
fn stale_hir_is_replayed_before_any_preflight_observation() {
    let mut stale = program();
    let copy = stale
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "app.copy")
        .unwrap();
    copy.body.ty = hir::ResolvedType::Bool;
    assert!(hir::validate(&stale).is_err());
    assert!(inspect(&stale, &DeclarationId::new("app.copy")).is_err());
    assert!(inspect(
        &program(),
        &DeclarationId::new("checked.generated.nonexistent")
    )
    .is_err());
}
