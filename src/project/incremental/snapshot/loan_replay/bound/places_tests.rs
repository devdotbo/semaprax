use std::mem::size_of;

use crate::hir::{Place, PlaceProjection, ResolvedExpr, ValueId};
use crate::loan_plan::{Loan, LoanEndpoint, LoanId, LoanPlan, LoanProgramPoint, MAX_LOANS_PER_FUNCTION_V1};

use super::{add, capacity, function_bytes, product, sum, tree, vector, walk, Inventory, Result};

fn original_function_bytes(inventory: &Inventory) -> Result<usize> {
    let expressions = inventory.expressions;
    let points = product(&[2, expressions])?;
    let edges = inventory.edges;
    let drafts = inventory.drafts;
    let loans = drafts.min(MAX_LOANS_PER_FUNCTION_V1);
    let bindings = inventory.bindings;
    // Alias resolution follows at most one link per distinct binding; a link
    // may carry any source/native-pattern projection prefix. Growth and two
    // simultaneously live old/new prefixes are retained, never refunded.
    let projection_payload = inventory
        .projection_text
        .checked_add(vector(inventory.projections, size_of::<PlaceProjection>())?)
        .ok_or_else(|| capacity("decoded HIR loan place bound overflows"))?;
    let place = product(&[2, sum(&[bindings, 1])?, projection_payload])?
        .checked_add(inventory.root_bytes)
        .ok_or_else(|| capacity("decoded HIR loan place bound overflows"))?;
    let mut bytes = size_of::<LoanPlan>();
    add(&mut bytes, inventory.identity_bytes)?;
    add(
        &mut bytes,
        product(&[4, sum(&[product(&[8, inventory.diagnostic_bytes])?, 1024])?])?,
    )?;
    // CFG vectors, both adjacency directions, maps/sets, traversal queues and
    // classification temporaries. Eight map-shaped inventories cover seen,
    // root, point, alias, bound, ownership, binding-loan and match-parent keys.
    add(
        &mut bytes,
        product(&[
            8,
            tree(
                sum(&[points, bindings, drafts])?,
                size_of::<(LoanProgramPoint, Place, ValueId)>(),
            )?,
        ])?,
    )?;
    add(&mut bytes, vector(points, size_of::<LoanProgramPoint>())?)?;
    add(
        &mut bytes,
        vector(expressions, size_of::<(&ResolvedExpr, &ResolvedExpr)>())?,
    )?;
    add(&mut bytes, vector(edges, size_of::<(u16, u16)>())?)?;
    add(
        &mut bytes,
        product(&[4, vector(points, size_of::<Vec<u16>>())?])?,
    )?;
    add(&mut bytes, product(&[5, vector(edges, size_of::<u16>())?])?)?;
    add(&mut bytes, vector(points, size_of::<u16>())?)?;
    add(
        &mut bytes,
        product(&[
            expressions,
            crate::loan_plan::REACHABILITY_BYTES_PER_EXPRESSION,
        ])?,
    )?;
    add(&mut bytes, crate::loan_plan::REACHABILITY_FIXED_BYTES)?;
    // Unfiltered drafts/aliases coexist with returned loan origins. Planner
    // drafts contain a place, optional roots/binding, point, cause, seed set
    // and span; two Loan-sized carriers cover that fixed representation.
    add(
        &mut bytes,
        product(&[2, vector(drafts, size_of::<Loan>())?])?,
    )?;
    add(
        &mut bytes,
        product(&[4, sum(&[bindings, drafts, loans, 1])?, place])?,
    )?;
    // Seeds, live sets and a replaced parent seed/live set can all coexist.
    add(
        &mut bytes,
        product(&[3, loans, tree(points, size_of::<u16>())?])?,
    )?;
    // Termination/liveness rows and returned endpoint/edge proof rows. Charge
    // growth on each nested vector, not only its outer carrier.
    add(&mut bytes, vector(loans, size_of::<Loan>())?)?;
    // The returned endpoint has compact slices, but construction retains
    // temporary Vec rows and may shrink them while their old storage is live.
    // Preserve the prior four-Vec carrier allowance for that transient peak.
    let endpoint_carrier = sum(&[
        size_of::<LoanProgramPoint>(),
        product(&[4, size_of::<Vec<LoanId>>()])?,
    ])?;
    add(
        &mut bytes,
        vector(points, endpoint_carrier.max(size_of::<LoanEndpoint>()))?,
    )?;
    add(
        &mut bytes,
        vector(
            edges,
            sum(&[size_of::<crate::loan_plan::LoanEdge>(), size_of::<usize>()])?,
        )?,
    )?;
    add(
        &mut bytes,
        product(&[8, points, vector(loans, size_of::<LoanId>())?])?,
    )?;
    add(
        &mut bytes,
        product(&[3, edges, vector(loans, size_of::<LoanId>())?])?,
    )?;
    add(
        &mut bytes,
        product(&[2, loans, vector(edges, size_of::<u16>())?])?,
    )?;
    add(
        &mut bytes,
        product(&[loans, vector(edges, size_of::<LoanProgramPoint>())?])?,
    )?;
    add(
        &mut bytes,
        product(&[sum(&[loans, edges, points])?, size_of::<Vec<u16>>()])?,
    )?;
    // Physical-capacity verification inventories every point reference plus
    // each loan site/start/end; it charges this allocation after the prebound.
    add(
        &mut bytes,
        vector(
            sum(&[points, product(&[loans, sum(&[edges, 2])?])?])?,
            size_of::<&crate::hir::ExpressionId>(),
        )?,
    )?;
    Ok(bytes)
}


#[test]
fn single_materialization_discount_changes_only_place_term_and_native_keeps_old_formula() {
    for (bindings, drafts, projections, text) in [
        (0usize, 0usize, 0usize, 0usize),
        (3, 2, 0, 0), // empty-prefix chains
        (8, 3, 2, 17), // initial multi-field and synthetic assignment
        (16, 8, 11, 201), // independent roots and distinct prefix occurrences
    ] {
        let mut inventory = Inventory {
            expressions: 32, edges: 48, diagnostic_bytes: 32,
            bindings, drafts, projections, projection_text: text,
            identity_bytes: 1024, root_bytes: 64,
            native_prefix_replication: false,
        };
        let old = original_function_bytes(&inventory).unwrap();
        let new = function_bytes(&inventory).unwrap();
        let payload = text + 4 * projections.max(4) * size_of::<PlaceProjection>()
            * usize::from(projections != 0);
        let recipients = 4 * (bindings + drafts + drafts.min(256) + 1);
        assert_eq!(old - new, recipients * 2 * bindings * payload);
        inventory.native_prefix_replication = true;
        assert_eq!(function_bytes(&inventory).unwrap(), old);
    }
}

#[test]
fn borrowed_variant_shape_uses_full_fallback_without_trusting_interface_or_proof() {
    let source = r#"
module prefix_forecast;
@id("prefix.main") fn main() -> i64 {
    let owned = bytes_zeroed(1usize);
    let view = bytes_as_slice(owned);
    let candidate = byte_get(view, 0usize);
    match candidate { Option::Some { value } => if value == 0u8 { 0 } else { 1 }, Option::None {} => 0 }
}
"#;
    let ast = crate::check(source, "prefix-forecast.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    let mut expression = program.functions.iter().find(|f| f.id.as_str() == "prefix.main")
        .unwrap().body.clone();
    let crate::hir::ResolvedExprKind::Block { tail, .. } = &mut expression.kind else {
        panic!("fixture requires its actual Block");
    };
    let crate::hir::ResolvedExprKind::Match { mode, arms, scrutinee } = &mut tail.kind else {
        panic!("fixture requires its actual Match");
    };
    assert!(arms.iter().any(|arm| matches!(&arm.pattern,
        crate::hir::ResolvedMatchPattern::Variant { fields, .. } if !fields.is_empty())));
    // This forged mode is deliberately not authorized native borrowing. The
    // forecast must conservatively fall back BEFORE full validation refuses it.
    *mode = crate::hir::ResolvedMatchMode::Borrow;
    let crate::hir::ResolvedExprKind::Place(place) = &mut scrutinee.kind else {
        panic!("fixture requires its exact candidate Place");
    };
    // A long shared prefix, including foreign native field names, cannot
    // evade the fallback merely because interfaces do not authorize it.
    for index in 0..16 {
        place.projections.push(PlaceProjection::Field(crate::hir::DeclarationId::new(
            format!("foreign.native.é.\0.field.{index}"),
        )));
    }
    let mut inventory = Inventory::default();
    walk(&program, &expression, &mut inventory, 0).unwrap();
    assert!(inventory.native_prefix_replication);
    assert_eq!(function_bytes(&inventory).unwrap(), original_function_bytes(&inventory).unwrap());
    assert!(program.interfaces.is_empty());
}
