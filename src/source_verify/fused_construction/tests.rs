use super::*;
use crate::ast::ExprKind as K;

const SOURCE: &str = include_str!("../../../tests/fixtures/record-traversal-maximum.spx");

/// Independent shape: Bytes-first two-field record, swapped fresh bindings,
/// an unrelated helper and a single-move suffix.
const CELLS: &str = r#"module app.fused;

@id("app.fused.cell")
record Cell {
    @id("app.fused.cell.data")
    data: Bytes,
    @id("app.fused.cell.slot")
    slot: i64,
}

@id("app.fused.helper")
fn helper() -> i64
{
    1
}

@id("app.fused.build")
fn build() -> i64
{
    let mut step = 0;
    let mut cells = vec_with_capacity<Cell>(64usize);
    while step < 64 {
        let cell = Cell { data: bytes_zeroed(7usize), slot: step };
        cells = vec_push<Cell>(cells, cell);
        step = step + 1;
        0
    }
    let owned = cells;
    i64_from_usize(vec_len<Cell>(owned))
}
"#;

fn parsed(source: &str) -> Program {
    crate::parse(source, "fused-construction-source.spx").unwrap()
}

fn named<'p>(program: &'p Program, name: &str) -> &'p Function {
    program.functions.iter().find(|f| f.name == name).unwrap()
}

fn outcome(program: &Program) -> Result<Derivation<'_>, Refusal> {
    derive(program, named(program, "main"))
}

fn selected(program: &Program) -> SourceProof<'_> {
    match outcome(program) {
        Ok(Derivation::Selected(proof)) => proof,
        Ok(Derivation::Legacy32 { rows }) => panic!("legacy {rows} rows"),
        Err(refusal) => panic!("refused: {refusal:?}"),
    }
}

fn refusal(source: &str) -> Refusal {
    match outcome(&parsed(source)) {
        Err(refusal) => refusal,
        Ok(_) => panic!("unexpected derivation"),
    }
}

fn loop_nodes(function: &Function) -> (&Expr, &Expr) {
    let K::Block { statements, .. } = &function.body.kind else {
        panic!("body block")
    };
    let Statement::While {
        condition, body, ..
    } = &statements[2]
    else {
        panic!("construction loop")
    };
    (condition, body)
}

fn body_statement(function: &Function, index: usize) -> &Statement {
    let K::Block { statements, .. } = &function.body.kind else {
        panic!("body block")
    };
    &statements[index]
}

#[test]
fn original_maximum_fixture_derives_exact_borrowed_provisional_selection() {
    let program = parsed(SOURCE);
    let main = named(&program, "main");
    let proof = selected(&program);
    assert!(std::ptr::eq(proof.function(), main));
    assert!(std::ptr::eq(proof.record(), &program.types[0]));
    assert_eq!(
        (
            proof.rows(),
            proof.width(),
            proof.extent(),
            proof.string_bytes()
        ),
        (4096, 3, 12_288, 12_288)
    );
    assert_eq!(proof.destination().name, "items");
    assert!(std::ptr::eq(
        proof.destination().binding,
        body_statement(main, 0)
    ));
    assert!(matches!(proof.destination().capacity.kind, K::Usize(4096)));
    assert_eq!(proof.induction().name, "next");
    assert!(std::ptr::eq(
        proof.induction().binding,
        body_statement(main, 1)
    ));
    assert!(matches!(proof.induction().bound.kind, K::Int(4096)));
    let (condition, body) = loop_nodes(main);
    let construction = proof.construction();
    assert_eq!(construction.index, 2);
    assert!(std::ptr::eq(
        construction.statement,
        body_statement(main, 2)
    ));
    assert!(std::ptr::eq(construction.condition, condition));
    assert!(std::ptr::eq(construction.body, body));
    assert!(matches!(
        construction.constructor.kind,
        K::ConstructRecord { .. }
    ));
    let producer = proof.producer();
    assert_eq!(producer.initializer, 2);
    assert_eq!(producer.field.stable_id, "app.leaf.row.payload");
    assert!(matches!(producer.width_literal.kind, K::Usize(3)));
    assert!(matches!(&producer.call.kind, K::Call { name, .. } if name == "bytes_zeroed"));

    let site = "app.main.body.s2.body.s0.value.field.2";
    assert_eq!(proof.producer_site(), site);
    assert!(proof.producer_site_matches(site));
    assert!(!proof.producer_site_matches("app.main.body.s2.body.s0.value.field.20"));
    assert!(!proof.producer_site_matches("app.main.body.s2.body.s0.value.field."));
    assert!(!proof.producer_site_matches(""));

    let events: Vec<_> = proof.owner_events().collect();
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], OwnerEvent::Move(s) if std::ptr::eq(s, body_statement(main, 3))));
    assert!(matches!(events[1], OwnerEvent::Borrow(s) if std::ptr::eq(s, body_statement(main, 7))));
    assert!(matches!(events[2], OwnerEvent::Sort(s) if std::ptr::eq(s, body_statement(main, 8))));

    let key = proof.raw_join_key();
    assert_eq!(
        (
            key.module.as_str(),
            key.function.as_str(),
            key.record.as_str(),
            key.byte_field.as_str(),
            key.destination.as_str(),
            key.induction.as_str(),
            key.producer_site.as_str(),
        ),
        (
            "app.leaf",
            "app.main",
            "app.leaf.row",
            "app.leaf.row.payload",
            "items",
            "next",
            site,
        )
    );
    assert_eq!((key.rows, key.width, key.extent), (4096, 3, 12_288));
    assert_eq!(key.loop_span, statement_span(body_statement(main, 2)));
    assert_eq!(key.producer_span, producer.call.span);
    // A re-parsed module yields an equal raw key yet does not authenticate
    // this proof: the key is comparison data, never authority.
    let reparsed = parsed(SOURCE);
    assert_eq!(key, selected(&reparsed).raw_join_key());
    assert!(!proof.authenticate(&reparsed));
}

#[test]
fn proof_identity_is_bound_to_the_held_module_and_exact_loop_nodes() {
    let program = parsed(SOURCE);
    let proof = selected(&program);
    assert!(proof.still_held(&program));
    assert!(proof.authenticate(&program));
    let copy = program.clone();
    assert!(!proof.still_held(&copy));
    assert!(!proof.authenticate(&copy));
    let fresh = selected(&copy);
    assert!(!proof.same_identities(&fresh));

    let (condition, body) = loop_nodes(named(&program, "main"));
    for node in [condition, body] {
        let chosen = select(&program, node).expect("exact loop node selects");
        assert!(chosen.same_identities(&proof));
        assert!(select(&program, &node.clone()).is_none());
        assert!(select(&copy, node).is_none());
    }
    let main = named(&program, "main");
    assert!(select(&program, &main.body).is_none());

    let foreign = parsed(SOURCE);
    assert!(matches!(
        derive(&program, &foreign.functions[0]),
        Err(Refusal::FunctionShape)
    ));
}

#[test]
fn legacy32_boundary_keeps_finite_owned_loop_and_selection_starts_at_33() {
    assert_eq!(
        LEGACY_FINITE_ROWS,
        crate::byte_data_capacity::MAX_BYTES_COPY_SITES
    );
    assert_eq!(
        MAX_SELECTED_EXTENT_BYTES,
        crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES
    );
    assert_eq!(
        u64::from(MAX_SELECTED_ROWS),
        crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES
            / (2 * crate::vec_ops::OWNED_PAYLOAD_BYTES_PER_ELEMENT)
    );
    for rows in [1u32, 8, 32] {
        let program = parsed(&SOURCE.replace("4096", &rows.to_string()));
        assert!(matches!(
            outcome(&program),
            Ok(Derivation::Legacy32 { rows: actual }) if actual == rows
        ));
        let (condition, body) = loop_nodes(named(&program, "main"));
        assert!(select(&program, condition).is_none());
        assert!(select(&program, body).is_none());
        // The unchanged ordinary route still owns these counts.
        assert_eq!(
            crate::source_verify::finite_row_loop(&program, body),
            Some(rows)
        );
    }
    for rows in [33u32, 127, 2048, 4096] {
        let program = parsed(&SOURCE.replace("4096", &rows.to_string()));
        let proof = selected(&program);
        assert_eq!(proof.rows(), rows);
        assert_eq!(proof.extent(), u64::from(rows) * 3);
        assert_eq!(proof.string_bytes(), u64::from(rows) * 3);
        let (_, body) = loop_nodes(named(&program, "main"));
        assert_eq!(crate::source_verify::finite_row_loop(&program, body), None);
    }
    assert_eq!(refusal(&SOURCE.replace("4096", "4097")), Refusal::Rows);
    assert_eq!(refusal(&SOURCE.replace("4096", "0")), Refusal::Rows);

    let exact = SOURCE.replace("bytes_zeroed(3usize)", "bytes_zeroed(32usize)");
    assert_eq!(
        selected(&parsed(&exact)).extent(),
        MAX_SELECTED_EXTENT_BYTES
    );
    let over = SOURCE.replace("bytes_zeroed(3usize)", "bytes_zeroed(33usize)");
    assert_eq!(refusal(&over), Refusal::Extent);
    let wide = SOURCE
        .replace("4096", "33")
        .replace("bytes_zeroed(3usize)", "bytes_zeroed(3971usize)");
    assert_eq!(selected(&parsed(&wide)).extent(), 33 * 3971);
    let wider = wide.replace("bytes_zeroed(3971usize)", "bytes_zeroed(3972usize)");
    assert_eq!(refusal(&wider), Refusal::Extent);
}

#[test]
fn independent_shapes_spellings_and_record_envelopes_derive_without_fixture_constants() {
    let program = parsed(CELLS);
    let build = named(&program, "build");
    let Ok(Derivation::Selected(proof)) = derive(&program, build) else {
        panic!("independent shape selects")
    };
    assert_eq!(
        (
            proof.rows(),
            proof.width(),
            proof.extent(),
            proof.string_bytes()
        ),
        (64, 7, 448, 0)
    );
    assert_eq!(proof.producer().initializer, 0);
    assert_eq!(proof.producer().field.stable_id, "app.fused.cell.data");
    assert_eq!(
        proof.producer_site(),
        "app.fused.build.body.s2.body.s0.value.field.0"
    );
    assert!(std::ptr::eq(
        proof.destination().binding,
        body_statement(build, 1)
    ));
    assert!(std::ptr::eq(
        proof.induction().binding,
        body_statement(build, 0)
    ));
    assert!(matches!(
        proof.owner_events().collect::<Vec<_>>().as_slice(),
        [OwnerEvent::Move(_)]
    ));
    let (condition, _) = loop_nodes(build);
    assert!(select(&program, condition).is_some());
    assert!(matches!(
        derive(&program, named(&program, "helper")),
        Err(Refusal::BodyShape)
    ));
    let literal = parsed(&CELLS.replace("slot: step }", "slot: 5 }"));
    assert!(matches!(
        derive(&literal, named(&literal, "build")),
        Ok(Derivation::Selected(_))
    ));

    let renamed = SOURCE
        .replace("items", "construction")
        .replace("next", "index_before")
        .replace("app.main", "different.entry");
    let program = parsed(&renamed);
    let proof = selected(&program);
    assert_eq!(proof.destination().name, "construction");
    assert_eq!(proof.induction().name, "index_before");
    assert!(proof.producer_site_matches("different.entry.body.s2.body.s0.value.field.2"));

    // The owned-leaf Vec slice has no envelope for a third owned field, at
    // any row count.
    assert_eq!(
        MAX_ROW_OWNED_FIELDS,
        crate::hir::owned_leaf_collection::MAX_OWNED_FIELDS
    );
    assert_eq!(
        MAX_ROW_FIELDS,
        crate::hir::owned_leaf_collection::MAX_FIELDS
    );
    let noted = SOURCE
        .replace(
            "    payload: Bytes,\n",
            "    payload: Bytes,\n    @id(\"app.leaf.row.note\")\n    note: string,\n",
        )
        .replace(
            "payload: bytes_zeroed(3usize) }",
            "payload: bytes_zeroed(3usize), note: \"n\" }",
        );
    assert_ne!(noted, SOURCE);
    for rows in ["4096", "2730", "33"] {
        assert_eq!(
            refusal(&noted.replace("4096", rows)),
            Refusal::Record,
            "{rows}"
        );
    }
}

#[test]
fn scalar_storage_envelope_bounds_two_owner_rows_exactly() {
    // `scalars` i64 fields beside the unchanged String and Bytes owners. The
    // existing scalar-storage envelope is 8192 / scalars; one and two scalars
    // stay at the original 4096-row ceiling.
    let widened = |scalars: usize, rows: u64| {
        let mut declared = String::from("    payload: Bytes,\n");
        let mut initial = String::from("payload: bytes_zeroed(3usize)");
        for extra in 1..scalars {
            declared.push_str(&format!(
                "    @id(\"app.leaf.row.extra{extra}\")\n    extra{extra}: i64,\n"
            ));
            initial.push_str(&format!(", extra{extra}: 0"));
        }
        SOURCE
            .replace("    payload: Bytes,\n", &declared)
            .replace("payload: bytes_zeroed(3usize) }", &format!("{initial} }}"))
            .replace("4096", &rows.to_string())
    };
    for scalars in 1..=6usize {
        let exact =
            (crate::vec_ops::MAX_CAPACITY / scalars as u64).min(u64::from(MAX_SELECTED_ROWS));
        let held = parsed(&widened(scalars, exact));
        let proof = selected(&held);
        assert_eq!(u64::from(proof.rows()), exact, "{scalars} scalars");
        assert_eq!(proof.extent(), exact * 3);
        assert_eq!(proof.string_bytes(), exact * 3);
        assert_eq!(
            refusal(&widened(scalars, exact + 1)),
            Refusal::Rows,
            "{scalars} scalars, one past"
        );
    }
    // Eight declared fields is the existing bound; a ninth refuses.
    assert_eq!(refusal(&widened(7, 33)), Refusal::Record);
}

#[test]
fn forged_destination_induction_condition_body_and_producer_refuse_by_step() {
    for (from, to, expected) in [
        (
            "while next < 4096",
            "while next <= 4096",
            Refusal::Condition,
        ),
        (
            "while next < 4096",
            "while next < 4096 && true",
            Refusal::Condition,
        ),
        ("while next < 4096", "while 0 < next", Refusal::Condition),
        ("while next < 4096", "while next < 4095", Refusal::Rows),
        ("let mut next = 0;", "let mut next = 1;", Refusal::Induction),
        ("let mut next = 0;", "let next = 0;", Refusal::Induction),
        (
            "vec_with_capacity<Row>(4096usize)",
            "vec_new<Row>()",
            Refusal::Destination,
        ),
        (
            "let mut items = vec_with_capacity",
            "let items = vec_with_capacity",
            Refusal::Destination,
        ),
        ("next = next + 1", "next = next + 2", Refusal::LoopBody),
        (
            "next = next + 1;\n        0",
            "next = next + 1;\n        1",
            Refusal::LoopBody,
        ),
        (
            "items = vec_push<Row>(items, item);",
            "let escaped = item; items = vec_push<Row>(items, item);",
            Refusal::LoopBody,
        ),
        (
            "title: \"é\\u{0}\"",
            "title: string_clone(\"é\\u{0}\")",
            Refusal::RowConstructor,
        ),
        (
            "priority: next",
            "priority: next + 1",
            Refusal::RowConstructor,
        ),
        (
            "payload: bytes_zeroed(3usize)",
            "payload: bytes_zeroed_bounded(3usize, 3usize)",
            Refusal::Producer,
        ),
        (
            "payload: bytes_zeroed(3usize)",
            "payload: bytes_copy(bytes_zeroed(3usize))",
            Refusal::Producer,
        ),
        (
            "bytes_zeroed(3usize)",
            "bytes_zeroed(0usize)",
            Refusal::Producer,
        ),
        (
            "bytes_zeroed(3usize)",
            "bytes_zeroed(3usize + 0usize)",
            Refusal::Producer,
        ),
        (
            "bytes_zeroed(3usize)",
            "bytes_zeroed(18446744073709551615usize)",
            Refusal::Extent,
        ),
    ] {
        let source = SOURCE.replace(from, to);
        assert_ne!(source, SOURCE, "hostile mutation must be real: {from}");
        assert_eq!(refusal(&source), expected, "{from} -> {to}");
    }

    // A same-shaped helper is not a selected producer; no helper name is.
    let helper = format!(
        "{}\n@id(\"app.make\")\nfn make_payload() -> Bytes\n{{\n    bytes_zeroed(3usize)\n}}\n",
        SOURCE.replace("bytes_zeroed(3usize) }", "make_payload() }")
    );
    assert_eq!(refusal(&helper), Refusal::Producer);

    let doubled = SOURCE
        .replace(
            "    payload: Bytes,\n",
            "    payload: Bytes,\n    @id(\"app.leaf.row.extra\")\n    extra: Bytes,\n",
        )
        .replace(
            "payload: bytes_zeroed(3usize) }",
            "payload: bytes_zeroed(3usize), extra: bytes_zeroed(1usize) }",
        );
    assert_eq!(refusal(&doubled), Refusal::Record);
}

#[test]
fn open_modules_shadowed_builtins_and_function_contracts_refuse_before_shape() {
    let mut program = parsed(SOURCE);
    program.permits.push("clock.read".to_owned());
    assert!(matches!(outcome(&program), Err(Refusal::OpenModule)));

    for builtin in shape::BUILTINS {
        let mut program = parsed(SOURCE);
        let mut shadow = program.functions[0].clone();
        shadow.name = (*builtin).to_owned();
        shadow.stable_id = format!("app.shadow.{builtin}");
        program.functions.push(shadow);
        assert!(
            matches!(outcome(&program), Err(Refusal::ShadowedBuiltin)),
            "{builtin}"
        );
    }

    let mut program = parsed(SOURCE);
    program.functions[0].effects.push("clock.read".to_owned());
    assert!(matches!(outcome(&program), Err(Refusal::FunctionShape)));
    let mut program = parsed(SOURCE);
    program.functions[0].explicit_id = false;
    assert!(matches!(outcome(&program), Err(Refusal::FunctionShape)));
    let mut program = parsed(SOURCE);
    let guard = program.functions[0].body.clone();
    program.functions[0].requires.push(guard);
    assert!(matches!(outcome(&program), Err(Refusal::FunctionShape)));
}

#[test]
fn suffix_escape_clone_shadow_and_use_after_transfer_close_the_owner_domain() {
    for (from, to) in [
        (
            "let rows = items;",
            "let rows = items; let cloned = vec_clone_at<Row>(rows, 0usize);",
        ),
        ("let rows = items;", "let rows = items; let items = 0;"),
        ("let rows = items;", "let mut rows = items;"),
        (
            "let renewed = vec_sort_owned<Row>(rows);",
            "let renewed = vec_sort_owned<Row>(rows); let again = vec_len<Row>(rows);",
        ),
        (
            "let utf8 = str_as_bytes(text);",
            "let utf8 = foreign_read(text);",
        ),
        ("total = total + row.priority;", "total = row.title;"),
        ("let text = row.title;", "let text = row;"),
        ("in rows {", "in items {"),
        (
            "total = total + row.priority;",
            "total = total + row.priority; let inner = rows;",
        ),
        ("let mut count = 0;", "let mut count = 0; let byte_len = 0;"),
    ] {
        let source = SOURCE.replace(from, to);
        assert_ne!(source, SOURCE, "hostile mutation must be real: {from}");
        assert_eq!(refusal(&source), Refusal::Suffix, "{from} -> {to}");
    }

    let locals: String = (0..80)
        .map(|index| format!("let extra_{index} = 0; "))
        .collect();
    let flat = SOURCE.replace("let rows = items;", &format!("let rows = items; {locals}"));
    assert_eq!(
        refusal(&flat),
        Refusal::Suffix,
        "bounded scope refuses before growth"
    );

    // Three base events plus thirteen relays fill the event table exactly.
    let relays = |count: usize| {
        let mut chain = String::from("let r0 = items; ");
        for index in 1..count {
            chain.push_str(&format!("let r{index} = r{}; ", index - 1));
        }
        SOURCE.replace(
            "let rows = items;",
            &format!("{chain}let rows = r{};", count - 1),
        )
    };
    let full = parsed(&relays(13));
    assert_eq!(selected(&full).owner_events().count(), MAX_OWNER_EVENTS);
    assert_eq!(refusal(&relays(14)), Refusal::Suffix);
}

#[test]
fn source_selection_leaves_default_admission_and_legacy_refusal_unchanged() {
    let errors = crate::check(SOURCE, "fused-construction-source.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-T267"),
        "the selected source proof is not an admission"
    );
    for rows in [33u32, 4096] {
        let source = SOURCE.replace("4096", &rows.to_string());
        assert!(selected(&parsed(&source)).rows() == rows);
        let errors = crate::check(&source, "fused-construction-source.spx").unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == "SPX-T267"),
            "{rows}"
        );
    }
}
