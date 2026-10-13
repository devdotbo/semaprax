use super::*;

fn index() -> DeclarationIndex {
    let mut index = DeclarationIndex::default();
    let ast = crate::parse(
        "module lookup; @id(\"root\") fn main()->i64 {7}",
        "lookup.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let template = program
        .declarations
        .declaration(&DeclarationId::new("root"))
        .unwrap();
    for text in [
        "",
        "a",
        "a\0tail",
        "a#invariant",
        "a#invariant\0tail",
        "é",
        "é#invariant",
        "🦀suffix",
    ] {
        let id = DeclarationId::new(text);
        let mut value = template.clone();
        value.id = id.clone();
        index.declarations.insert(id, value);
    }
    index
}
#[test]
fn complete_root_suffix_utf8_null_order_and_first_refusal_withhold_read_results() {
    let index = index();
    for text in ["", "a", "a\0tail", "é", "🦀suffix", "absent", "é\0missing"] {
        let key = DeclarationId::new(text);
        for suffix in [&[][..], &b"#invariant"[..]] {
            let full =
                DeclarationId::new(format!("{text}{}", std::str::from_utf8(suffix).unwrap()));
            let expected = index.declaration(&full);
            let mut used = 0usize;
            let actual = get(&index.declarations, &key, suffix, &mut |units| {
                used += units;
                true
            })
            .unwrap();
            assert_eq!(actual, expected);
            for limit in [0, used.saturating_sub(1), used] {
                let mut remaining = limit;
                let mut refused = false;
                let mut callbacks_after_refusal = 0;
                let actual = get(&index.declarations, &key, suffix, &mut |units| {
                    if refused {
                        callbacks_after_refusal += 1;
                        return false;
                    }
                    match remaining.checked_sub(units) {
                        Some(next) => {
                            remaining = next;
                            true
                        }
                        None => {
                            refused = true;
                            false
                        }
                    }
                });
                assert_eq!(callbacks_after_refusal, 0);
                if limit < used {
                    assert!(refused && actual.is_err());
                } else {
                    assert_eq!(actual.unwrap(), expected);
                }
            }
        }
    }
    assert!(get(
        &DeclarationIndex::default().declarations,
        &DeclarationId::new("root"),
        &[],
        &mut |_| false
    )
    .is_err());
}

#[test]
fn full_string_directory_keys_preserve_value_precedence_and_withhold_refused_values() {
    let map: BTreeMap<String, usize> = ["", "a", "a\0tail", "nominal:2:é:0:", "🦀suffix"]
        .into_iter()
        .enumerate()
        .map(|(index, text)| (text.to_owned(), index))
        .collect();
    for query in [
        "",
        "a",
        "a\0tail",
        "nominal:2:é:0:",
        "🦀suffix",
        "absent",
        "🦀",
    ] {
        let expected = map.get(query);
        let mut work = 0usize;
        assert_eq!(
            get_text(&map, query, &mut |units| {
                work += units;
                true
            })
            .unwrap(),
            expected
        );
        for limit in [0, work - 1, work] {
            let mut remaining = limit;
            let mut first = false;
            let mut late = 0;
            let result = get_text(&map, query, &mut |units| {
                if first {
                    late += 1;
                    return false;
                }
                match remaining.checked_sub(units) {
                    Some(value) => {
                        remaining = value;
                        true
                    }
                    None => {
                        first = true;
                        false
                    }
                }
            });
            assert_eq!(late, 0);
            if limit < work {
                assert!(first && result.is_err());
            } else {
                assert_eq!(result.unwrap(), expected);
            }
        }
    }
}
