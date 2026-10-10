use super::*;
const SOURCE: &str = r#"module recipe.lexical;
@id("recipe.lexical.main") fn main() -> i64 {
    let mut index = 0;
    while index < 4 {
        let inner = 7;
        index = index + 1;
        if index == 1 { continue; };
        if index == 3 { break; };
        0
    }
    return index;
}
"#;
fn program(source: &str) -> ResolvedProgram {
    let ast = crate::check(source, "recipe-lexical.spx").unwrap();
    crate::hir::resolve(&ast).unwrap()
}
#[test]
fn lexical_selection_replays_statement_guards_and_exact_wasm() {
    let linked = program(SOURCE);
    crate::hir::validate(&linked).unwrap();
    let recipe = render(&linked).unwrap();
    assert!(recipe.starts_with(HEADER));
    assert!(
        recipe.contains("continue;") && recipe.contains("break;") && recipe.contains("return ")
    );
    let replayed = replay_against(&linked, &recipe).unwrap();
    assert_eq!(render(&replayed).unwrap(), recipe);
    assert!(legacy::render(&linked).is_err());
    assert!(legacy::replay(recipe.strip_prefix(HEADER).unwrap()).is_err());
    assert_eq!(
        crate::wasm::emit_resolved_module(&linked).unwrap(),
        crate::wasm::emit_resolved_module(&replayed).unwrap()
    );
}
#[test]
fn selection_marker_cannot_grant_legacy_or_forged_control_authority() {
    let ordinary = program("module recipe.ordinary; @id(\"ordinary.main\") fn main() -> i64 { 7 }");
    let recipe = render(&ordinary).unwrap();
    assert_eq!(recipe, legacy::render(&ordinary).unwrap());
    assert!(replay(&format!("{HEADER}{recipe}")).is_err());
    let mut linked = program(SOURCE);
    fn forge(expression: &mut crate::hir::ResolvedExpr) -> bool {
        if let crate::hir::ResolvedExprKind::LexicalExit { target, .. } = &mut expression.kind {
            *target =
                crate::hir::ControlTarget::Function(crate::hir::FunctionExecutionId::Monomorphic(
                    crate::hir::DeclarationId::new("forged.target"),
                ));
            return true;
        }
        // The final direct return is the owning mutation seam in this fixture.
        if let crate::hir::ResolvedExprKind::Block { tail, .. } = &mut expression.kind {
            return forge(tail);
        }
        false
    }
    let entry = linked.entrypoint.clone();
    let function = linked
        .functions
        .iter_mut()
        .find(|function| function.id == entry)
        .unwrap();
    assert!(forge(&mut function.body));
    assert!(render(&linked).is_err());
}
