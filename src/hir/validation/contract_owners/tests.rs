use crate::hir::{self, ResolvedExprKind, ResolvedProgram, ResolvedStatement};
const SOURCE: &str = r#"module contract.hir;
@id("inspect") fn inspect(input:own Bytes)->bool {byte_len(bytes_as_slice(input))==2usize}
@id("factory") fn factory(n:usize)->Bytes {bytes_zeroed(2usize)}
@id("probe") fn probe(input:own Bytes)->Bytes
 requires {let local=bytes_zeroed(2usize);inspect(local)}
 ensures {let checked=bytes_zeroed(2usize);byte_len(bytes_as_slice(checked))==2usize}
 {bytes_zeroed(1usize)}
@id("main") fn main()->i64{0}
"#;
fn program() -> ResolvedProgram {
    let ast = crate::check(SOURCE, "contract-hir.spx").unwrap();
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    program
}
fn initializer(program: &mut ResolvedProgram, ensures: bool) -> &mut hir::ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    let clause = if ensures {
        &mut function.ensures[0]
    } else {
        &mut function.requires[0]
    };
    let ResolvedExprKind::Block { statements, .. } = &mut clause.kind else {
        panic!("contract block")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("fresh local")
    };
    value
}
#[test]
fn forged_contract_initializer_cannot_take_preexisting_or_provisional_owner() {
    for ensures in [false, true] {
        let mut forged = program();
        let function = forged
            .functions
            .iter()
            .find(|f| f.id.as_str() == "probe")
            .unwrap();
        let root = if ensures {
            function.result_id.clone()
        } else {
            function.params[0].id.clone()
        };
        initializer(&mut forged, ensures).kind = ResolvedExprKind::Place(hir::Place {
            root,
            projections: Vec::new(),
        });
        assert!(hir::validate(&forged)
            .unwrap_err()
            .message
            .contains("contract cannot transfer ownership into a local binding"));
    }
}
#[test]
fn signature_compatible_user_factory_does_not_mint_constructor_freshness() {
    let mut forged = program();
    let factory = forged
        .functions
        .iter()
        .find(|f| f.id.as_str() == "factory")
        .unwrap()
        .id
        .clone();
    let ResolvedExprKind::Call { callee, .. } = &mut initializer(&mut forged, false).kind else {
        panic!("allocation")
    };
    *callee = factory;
    assert!(hir::validate(&forged)
        .unwrap_err()
        .message
        .contains("contract cannot transfer ownership into a local binding"));
}
#[test]
fn forged_fresh_local_identity_cannot_alias_the_result_domain() {
    let mut forged = program();
    let function = forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    let root = function.result_id.clone();
    let ResolvedExprKind::Block { statements, .. } = &mut function.ensures[0].kind else {
        panic!("block")
    };
    let ResolvedStatement::Let { binding, .. } = &mut statements[0] else {
        panic!("local")
    };
    binding.id = root;
    assert!(hir::validate(&forged)
        .unwrap_err()
        .message
        .contains("non-canonical identity"));
}
