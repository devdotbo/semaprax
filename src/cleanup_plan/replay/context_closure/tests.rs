use super::{derive, Inventory};
use crate::cleanup_plan::replay::{
    physical_storage::PhysicalReservation, AdmissionMode, AllocationPayer, ReplayBudget,
};
use crate::hir::{ResolvedExprKind as K, ResolvedProgram, ResolvedType};

const SOURCE: &str = r#"module closure;
@id("closure.Leaf") record Leaf { @id("closure.Leaf.n") n:i64, }
@id("closure.Packet") record Packet { @id("closure.Packet.leaf") leaf:Leaf, @id("closure.Packet.data") data:Bytes, @id("closure.Packet.text") text:string, }
@id("closure.helper") fn helper(n:i64)->i64 {n+1}
@id("closure.read") fn read(packet:borrow Packet)->i64 {helper(1)}
@id("closure.unused") fn unused()->i64 {7}
@id("closure.main") fn main()->i64 {helper(41)}
"#;
fn checked() -> ResolvedProgram {
    let source = crate::check(SOURCE, "inactive-context-closure.spx").unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    crate::hir::validate(&program).unwrap();
    program
}
fn budget(bytes: usize) -> ReplayBudget {
    let mut b = ReplayBudget::new();
    b.admission = AdmissionMode::SelectedRetained;
    b.payer = AllocationPayer::SelectedReservation;
    b.physical = Some(Box::new(PhysicalReservation::new(bytes)));
    b
}
fn ids<'a>(i: &Inventory<'a>) -> (Vec<&'a str>, Vec<&'a str>) {
    (
        i.functions.iter().map(|f| f.id.as_str()).collect(),
        i.types.iter().map(|t| t.id.as_str()).collect(),
    )
}
// Independent bounded-fixture oracle intentionally uses the old allocating DFS.
// It is not the production constructor and grants no validation authority.
fn oracle<'a>(p: &'a ResolvedProgram, root: &'a crate::hir::ResolvedFunction) -> Vec<&'a str> {
    fn visit<'a>(
        p: &'a ResolvedProgram,
        f: &'a crate::hir::ResolvedFunction,
        found: &mut Vec<&'a str>,
    ) {
        if found.contains(&f.id.as_str()) {
            return;
        }
        found.push(f.id.as_str());
        let mut pending = Vec::new();
        pending.extend(f.ensures.iter().rev());
        pending.push(&f.body);
        pending.extend(f.requires.iter().rev());
        while let Some(e) = pending.pop() {
            if let K::Call { callee, .. } = &e.kind {
                visit(
                    p,
                    p.functions.iter().find(|g| g.id == *callee).unwrap(),
                    found,
                );
            }
            let children: Vec<_> = crate::hir::lexical_exit::ChildCursor::new(e).collect();
            pending.extend(children.into_iter().rev());
        }
    }
    let mut found = Vec::new();
    visit(p, root, &mut found);
    found
}
#[test]
fn borrowed_closure_matches_full_fixture_walk_without_granting_context_authority() {
    let p = checked();
    let root = p
        .functions
        .iter()
        .find(|f| f.id.as_str() == "closure.read")
        .unwrap();
    let i = derive(&p, root, &mut budget(usize::MAX)).unwrap();
    assert_eq!(ids(&i).0, oracle(&p, root));
    assert_eq!(ids(&i).1, ["closure.Packet", "closure.Leaf"]);
    assert!(i
        .functions
        .iter()
        .all(|f| p.functions.iter().any(|g| std::ptr::eq(*f, g))));
    assert!(i
        .types
        .iter()
        .all(|t| p.types.iter().any(|u| std::ptr::eq(*t, u))));
    let main = p
        .functions
        .iter()
        .find(|f| f.id.as_str() == "closure.main")
        .unwrap();
    assert!(derive(&p, main, &mut budget(usize::MAX))
        .unwrap()
        .types
        .is_empty());
    // A forged unrelated declaration can be outside discovery, but the required
    // ordinary validator still rejects it; Inventory is not a validation lease.
    let mut forged = p.clone();
    let unrelated = forged
        .types
        .iter_mut()
        .find(|t| t.id.as_str() == "closure.Packet")
        .unwrap();
    unrelated.id = crate::hir::DeclarationId::new("closure.absent");
    let main = forged
        .functions
        .iter()
        .find(|f| f.id.as_str() == "closure.main")
        .unwrap();
    assert!(derive(&forged, main, &mut budget(usize::MAX)).is_ok());
    assert!(crate::hir::validate(&forged).is_err());
}
#[test]
fn reachable_stale_namespace_layout_call_and_cycle_mutations_refuse() {
    let p = checked();
    let root = p
        .functions
        .iter()
        .find(|f| f.id.as_str() == "closure.read")
        .unwrap();
    let detached = root.clone();
    assert!(derive(&p, &detached, &mut budget(usize::MAX)).is_err());
    for mutation in 0..4 {
        let mut q = p.clone();
        if mutation < 2 {
            let t = q
                .types
                .iter_mut()
                .find(|t| t.id.as_str() == "closure.Packet")
                .unwrap();
            let crate::hir::ResolvedTypeDeclarationKind::Record { fields } = &mut t.kind else {
                panic!()
            };
            if mutation == 0 {
                fields[0].ty = ResolvedType::Bool;
            } else {
                fields[0].id = crate::hir::DeclarationId::new("foreign.field");
            }
        } else {
            let f = q
                .functions
                .iter_mut()
                .find(|f| f.id.as_str() == "closure.helper")
                .unwrap();
            if mutation == 2 {
                f.name = "stale".into();
            } else {
                let mut call = root.body.clone();
                // Preserve exact signature/argument types while introducing a
                // reachable self-call; the active function colors must refuse.
                if let K::Block { tail, .. } = &mut call.kind {
                    call = *tail.clone();
                }
                let K::Call { callee, .. } = &mut call.kind else {
                    panic!()
                };
                *callee = f.id.clone();
                f.body = call;
            }
        }
        let root = q
            .functions
            .iter()
            .find(|f| f.id.as_str() == "closure.read")
            .unwrap();
        assert!(
            derive(&q, root, &mut budget(usize::MAX)).is_err(),
            "mutation {mutation}"
        );
    }
}
#[test]
fn complete_inventory_backing_is_exact_one_short_and_sticky() {
    let p = checked();
    let root = p
        .functions
        .iter()
        .find(|f| f.id.as_str() == "closure.read")
        .unwrap();
    let mut measured = budget(usize::MAX);
    let inventory = derive(&p, root, &mut measured).unwrap();
    let bytes = measured.physical.as_ref().unwrap().charged();
    assert!(bytes > 0);
    // Keep earlier inventories live: each invocation pays its complete backing.
    let mut exact = budget(bytes);
    let second = derive(&p, root, &mut exact).unwrap();
    assert_eq!(ids(&inventory), ids(&second));
    assert_eq!(exact.physical.as_ref().unwrap().charged(), bytes);
    let mut short = budget(bytes - 1);
    assert!(derive(&p, root, &mut short).is_err());
    assert!(short.physical.as_deref_mut().unwrap().reserve(0).is_err());
    let mut work = budget(usize::MAX);
    work.remaining = 0;
    assert!(derive(&p, root, &mut work).is_err());
}
