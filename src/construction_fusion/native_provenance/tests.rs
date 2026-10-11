use super::*;
fn inputs(count: u32) -> (ast::Program, hir::ResolvedProgram) {
    super::super::tests::inputs(count)
}
#[test]
fn mixed_string_and_lease_views_keep_exact_producers_and_aliases() {
    for count in [1, 8, 32] {
        let (source, typed) = inputs(count);
        let mapping = derive(&source, &typed).unwrap();
        assert!(mapping.still_held(&source, &typed));
        assert!(!mapping.still_held(&source.clone(), &typed));
        for carrier in [
            Carrier::OrdinaryStr,
            Carrier::OrdinarySlice,
            Carrier::LeasedSlice,
        ] {
            assert!(mapping.facts().iter().any(|fact| fact.carrier() == carrier));
        }
        for fact in mapping.facts() {
            assert!(std::ptr::eq(mapping.fact(fact.expression).unwrap(), fact));
            assert!(mapping.fact(&fact.expression.clone()).is_none());
            assert!(matches!(fact.producer().kind, K::VecFieldRead { .. }));
            assert_eq!(fact.vector(), &mapping.descriptor.typed.vector.id);
            match fact.carrier() {
                Carrier::LeasedSlice => assert_eq!(fact.field().ty, T::Bytes),
                Carrier::OrdinaryStr | Carrier::OrdinarySlice => {
                    assert_eq!(fact.field().ty, T::String)
                }
            }
        }
        let ordinary = mapping
            .facts()
            .iter()
            .find(|f| f.carrier() == Carrier::OrdinaryStr)
            .unwrap();
        assert!(mapping
            .facts()
            .iter()
            .any(|f| f.carrier() == Carrier::OrdinarySlice
                && std::ptr::eq(f.producer(), ordinary.producer())));
        let layout = mapping.layout();
        assert_eq!((layout.bytes, layout.view, layout.row), (24, 48, 40));
        assert_eq!(layout.owned, count as usize * 32);
        assert_eq!(layout.vector, count as usize * 40);
        assert_eq!(layout.view_frame_upper, mapping.facts().len() * 96);
        assert_eq!((layout.context, layout.issuer), (165504, 32));
    }
    assert!(
        crate::check(super::super::tests::ORIGINAL, "unselected-max.spx")
            .unwrap_err()
            .iter()
            .any(|d| d.code == "SPX-T267")
    );
}
#[test]
fn source_alias_field_and_current_hir_producer_drift_refuse() {
    let (source, typed) = inputs(8);
    let mapping = derive(&source, &typed).unwrap();
    let fact = mapping
        .facts()
        .iter()
        .find(|f| f.carrier() == Carrier::LeasedSlice)
        .unwrap();
    let mut work = crate::cache_codec::MAX_NODES;
    let sources = source::inventory(&source, &mut work).unwrap();
    let mut scratch = Scratch {
        frames: [None; DEPTH],
        aliases: [None; LOCALS],
        len: 1,
    };
    let K::VecFieldRead { args, .. } = &fact.producer().kind else {
        panic!("producer")
    };
    let K::Place(place) = &args[0].kind else {
        panic!("root")
    };
    let mut root_binding = mapping.descriptor.typed.vector.clone();
    root_binding.id = place.root.clone();
    root_binding.ty = args[0].ty.clone();
    root_binding.ownership = args[0].ownership;
    scratch.aliases[0] = Some(Alias {
        binding: &root_binding,
        value: Value::Vector(&mapping.descriptor.typed.vector.id),
    });
    assert!(classify(
        &mapping.descriptor,
        &sources,
        &scratch,
        fact.producer(),
        &mut work
    )
    .unwrap()
    .is_some());
    // Direct producer authentication is independent of any attached loan plan.
    for change in 0..4 {
        let mut forged = fact.producer().clone();
        match change {
            0 => forged.ownership = O::Own,
            1 => forged.ty = T::Str,
            2 => {
                let K::VecFieldRead { field, .. } = &mut forged.kind else {
                    panic!("producer")
                };
                *field = hir::DeclarationId::new("app.leaf.row.title");
            }
            _ => {
                let K::VecFieldRead { args, .. } = &mut forged.kind else {
                    panic!("producer")
                };
                let K::Place(place) = &mut args[0].kind else {
                    panic!("vector")
                };
                place.root = mapping.descriptor.typed.counter.id.clone();
            }
        }
        assert!(classify(&mapping.descriptor, &sources, &scratch, &forged, &mut work).is_none());
    }
    let alias_fact = mapping
        .facts()
        .iter()
        .find(|f| matches!(f.expression.kind, K::Place(_)) && f.carrier() == Carrier::LeasedSlice)
        .unwrap();
    let K::Place(alias_place) = &alias_fact.expression.kind else {
        panic!("alias")
    };
    let ast::ExprKind::Var(alias_name) = &sources
        .iter()
        .find(|e| e.span == alias_fact.expression.span)
        .unwrap()
        .kind
    else {
        panic!("source alias")
    };
    let mut alias_binding = root_binding.clone();
    alias_binding.id = alias_place.root.clone();
    alias_binding.name = alias_name.clone();
    alias_binding.ty = T::SliceU8;
    alias_binding.ownership = O::Borrow;
    scratch.aliases[0] = Some(Alias {
        binding: &alias_binding,
        value: Value::View(alias_fact.origin),
    });
    assert!(classify(
        &mapping.descriptor,
        &sources,
        &scratch,
        alias_fact.expression,
        &mut work
    )
    .unwrap()
    .is_some());
    let mut forged_alias_binding = alias_binding.clone();
    forged_alias_binding.name = "foreign-alias".into();
    scratch.aliases[0] = Some(Alias {
        binding: &forged_alias_binding,
        value: Value::View(alias_fact.origin),
    });
    assert!(classify(
        &mapping.descriptor,
        &sources,
        &scratch,
        alias_fact.expression,
        &mut work
    )
    .is_none());
    let original = super::super::tests::ORIGINAL.replace("4096", "8");
    for (from, to) in [
        ("let data = row.payload;", "let data = row.title;"),
        ("str_as_bytes(text)", "str_as_bytes(data)"),
        (
            "let rows = items;",
            "let rows = items;let copy=vec_clone_at<Row>(rows,0usize);",
        ),
    ] {
        let changed = crate::parse(&original.replace(from, to), "fusion.spx").unwrap();
        assert!(derive(&changed, &typed).is_none());
    }
}
#[test]
fn complete_proof_backing_exact_and_one_short_and_native_layout_bounds() {
    let (source, typed) = inputs(8);
    let (mapping, refused, bytes) =
        crate::bounded_output::with_limit_usage(usize::MAX, || derive(&source, &typed));
    assert!(mapping.is_some());
    assert!(!refused);
    assert!(bytes > size_of::<Scratch<'_>>());
    let (exact, refused, used) =
        crate::bounded_output::with_limit_usage(bytes, || derive(&source, &typed));
    assert!(exact.is_some());
    assert!(!refused);
    assert_eq!(used, bytes);
    let (short, refused, _) =
        crate::bounded_output::with_limit_usage(bytes - 1, || derive(&source, &typed));
    assert!(short.is_none());
    assert!(refused);
    // Representation arithmetic is not source admission or runtime evidence.
    let maximum = layout::for_count(4096, 64).unwrap();
    assert_eq!(maximum.owned, 131072);
    assert_eq!(maximum.vector, 163840);
    assert_eq!(maximum.context, 165504);
    assert!(layout::for_count(4097, 64).is_none());
    assert!(layout::for_count(4096, usize::MAX).is_none());
}
