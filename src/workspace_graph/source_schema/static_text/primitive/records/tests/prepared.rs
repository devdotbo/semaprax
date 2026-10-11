//! Independent owning controls for source-bound type/signature preparation.
use super::*;
use crate::cleanup_plan::SelectedReplayPreparation;
fn prepare<'a>(
    module: &'a WorkspaceResolvedModule,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    work: usize,
    bytes: usize,
) -> Result<(super::super::super::Context<'a>, usize, usize), Vec<Diagnostic>> {
    let anchor = module.functions.first().unwrap();
    let mut resources =
        SelectedReplayPreparation::with_limits(anchor, work, bytes).map_err(|e| vec![e])?;
    resources
        .reserve_array::<super::super::super::Context<'_>>(anchor, 1, "owning prepared context")
        .map_err(|e| vec![e])?;
    let context =
        super::super::super::helpers::local::authenticate(module, facts, &mut resources, anchor)?;
    let spent = work - resources.remaining();
    let bytes = resources
        .construct(anchor, |physical, _| Ok(physical.charged()))
        .map_err(|e| vec![e])?;
    Ok((context, spent, bytes))
}
#[test]
fn prepared_string_vec_and_borrowed_record_signatures_keep_current_type_pointers_and_canonical_vec()
{
    let source = format!("{}\n@id(\"flat.count\") fn count(values:borrow Vec<i64>)->i64 {{i64_from_usize(vec_len<i64>(values))}}\n@id(\"flat.texts\") fn texts(values:borrow Vec<string>)->i64 {{0}}\n@id(\"flat.rows\") fn rows(values:borrow Vec<Packet>)->i64 {{0}}", lexical_source());
    let build = build_source(&source);
    let module = module(&build);
    assert!(module.static_text.as_ref().unwrap().lexical.is_some());
    let (context, _, _) = prepare(module, &build.hir.declarations, 32_000_000, usize::MAX).unwrap();
    assert_eq!(context.type_count(), module.types.len());
    for (i, current) in module.types.iter().enumerate() {
        assert!(std::ptr::eq(context.type_at(i).unwrap(), current));
        assert!(!std::ptr::eq(context.type_at(i).unwrap(), &current.clone()));
    }
    assert!(context.type_at(module.types.len()).is_none());
    let id = hir::DeclarationId::new(crate::prelude::VEC_ID);
    let declaration = context.declarations().declaration(&id).unwrap();
    assert_eq!(declaration.kind, hir::DeclarationKind::Record);
    assert_eq!(
        declaration.identity_origin,
        hir::IdentityOrigin::CompilerOwned
    );
    assert_eq!(declaration.name, "Vec");
    assert!(declaration.owner.is_none());
    let [parameter] = context.declarations().type_parameters(&id).unwrap() else {
        panic!("canonical Vec parameter");
    };
    assert_eq!(parameter.name, "T");
    assert_eq!(parameter.index, 0);
    assert_eq!(context.declarations().record_fields(&id).unwrap(), &[]);
}
#[test]
fn prepared_current_layout_namespace_and_signature_drift_are_independent_and_sticky() {
    let build = build_lexical();
    let source = module(&build);
    let anchor = source.functions.first().unwrap();
    let inventory = source
        .static_text
        .as_ref()
        .unwrap()
        .lexical
        .as_ref()
        .unwrap();
    for mutation in 0..6 {
        let mut facts = build.hir.declarations.clone();
        let key = if mutation < 3 {
            "flat.Packet"
        } else {
            "flat.Packet.text"
        };
        let fact = facts.get_mut(key).unwrap();
        match mutation {
            0 => fact.origin = hir::IdentityOrigin::CompilerOwned,
            1 => fact.path = Some("wrong.spx".into()),
            2 => fact.module = Some("wrong.module".into()),
            3 => fact.kind = hir::DeclarationKind::Function,
            4 => fact.owner = Some("wrong.owner".into()),
            5 => fact.origin = hir::IdentityOrigin::Automatic,
            _ => unreachable!(),
        }
        let mut resources =
            SelectedReplayPreparation::with_limits(anchor, 32_000_000, usize::MAX).unwrap();
        let mut index = hir::DeclarationIndex::default();
        assert!(super::super::authenticate_prepared(
            source,
            inventory.records.as_deref(),
            &facts,
            &mut index,
            &mut resources,
            anchor
        )
        .is_err());
        assert!(resources.charge(anchor, 0, "later current record").is_err());
    }
    for mutation in 0..3 {
        let mut stale = build_lexical();
        let source = stale
            .hir
            .modules
            .iter_mut()
            .find(|m| m.module == "static.flat_records")
            .unwrap();
        match mutation {
            0 => source.functions[0].params[0].ownership = hir::OwnershipMode::Borrow,
            1 => source.functions[0].return_type = hir::ResolvedType::Bool,
            2 => source.functions[0].name.push_str("_drift"),
            _ => unreachable!(),
        }
        let anchor = source.functions.first().unwrap();
        let mut resources =
            SelectedReplayPreparation::with_limits(anchor, 32_000_000, usize::MAX).unwrap();
        assert!(super::super::super::helpers::local::authenticate(
            source,
            &stale.hir.declarations,
            &mut resources,
            anchor
        )
        .is_err());
        assert!(resources.charge(anchor, 0, "later signature").is_err());
    }
    let separate = build_lexical();
    let mut resources =
        SelectedReplayPreparation::with_limits(anchor, 32_000_000, usize::MAX).unwrap();
    super::super::super::current_module(source, &build.hir.modules, &mut resources, anchor)
        .unwrap();
    assert!(super::super::super::current_module(
        source,
        &separate.hir.modules,
        &mut resources,
        anchor
    )
    .is_err());
    assert!(resources.charge(anchor, 0, "detached module").is_err());
    let mut duplicate = build_lexical();
    let another = build_lexical();
    let repeated = another
        .hir
        .modules
        .into_iter()
        .find(|m| m.module == "static.flat_records")
        .unwrap();
    duplicate.hir.modules.push(repeated);
    let repeated = module(&duplicate);
    let repeated_anchor = repeated.functions.first().unwrap();
    let mut resources =
        SelectedReplayPreparation::with_limits(repeated_anchor, 32_000_000, usize::MAX).unwrap();
    assert!(super::super::super::current_module(
        repeated,
        &duplicate.hir.modules,
        &mut resources,
        repeated_anchor
    )
    .is_err());
    let (context, _, _) = prepare(source, &build.hir.declarations, 32_000_000, usize::MAX).unwrap();
    // Attached schema is not source-signature authority; the actual borrowed
    // member universe remains independently reconstructible.
    assert!(std::ptr::eq(
        context.function_at(0).unwrap(),
        &source.functions[0]
    ));
}
#[test]
fn prepared_full_context_authentication_work_is_cumulative_exact_and_one_short() {
    let build = build_lexical();
    let source = module(&build);
    let (_, spent, bytes) =
        prepare(source, &build.hir.declarations, 32_000_000, usize::MAX).unwrap();
    assert_eq!(
        prepare(source, &build.hir.declarations, spent, bytes)
            .unwrap()
            .2,
        bytes
    );
    assert!(prepare(source, &build.hir.declarations, spent, bytes - 1).is_err());
    assert!(spent > 0);
    assert_eq!(
        prepare(source, &build.hir.declarations, spent, usize::MAX)
            .unwrap()
            .1,
        spent
    );
    assert!(prepare(source, &build.hir.declarations, spent - 1, usize::MAX).is_err());
    // Two source-authentication invocations cannot obtain fresh work from one
    // held owner; the second refusal remains sticky even for a zero debit.
    let anchor = source.functions.first().unwrap();
    let mut resources =
        SelectedReplayPreparation::with_limits(anchor, spent * 2 - 1, usize::MAX).unwrap();
    resources
        .reserve_array::<super::super::super::Context<'_>>(anchor, 1, "owning prepared context")
        .unwrap();
    let first = super::super::super::helpers::local::authenticate(
        source,
        &build.hir.declarations,
        &mut resources,
        anchor,
    )
    .unwrap();
    drop(first);
    assert!(super::super::super::helpers::local::authenticate(
        source,
        &build.hir.declarations,
        &mut resources,
        anchor
    )
    .is_err());
    assert!(resources.charge(anchor, 0, "after refusal").is_err());
}
