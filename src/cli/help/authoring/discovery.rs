//! Bounded discovery over existing authoring cards; no separate semantic catalog.
pub(super) fn search(query: &str, budget: usize) -> Result<String, String> {
    let mut parts = query.split(':');
    let (Some("author"), Some("find"), Some(word), Some(offset), None) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return Err("card search requires author:find:<word>:<offset>".to_owned());
    };
    if word.is_empty()
        || word.len() > 64
        || !word
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Err(
            "card search word must be 1..64 ASCII letters, digits, dots, underscores or hyphens"
                .to_owned(),
        );
    }
    let start = offset
        .parse::<usize>()
        .ok()
        .filter(|value| value.to_string() == offset)
        .ok_or_else(|| "card search offset must be canonical unsigned decimal".to_owned())?;
    let mut matches = Vec::new();
    for line in super::ROUTES.lines() {
        let Some(line) = line.strip_prefix("  author:") else {
            continue;
        };
        let (name, summary) = line
            .split_once(' ')
            .ok_or_else(|| "authoring inventory has no route summary".to_owned())?;
        let selector = format!("author:{name}");
        let card = super::lookup(&selector)
            .map_err(|_| format!("authoring inventory has no exact card for `{selector}`"))?;
        if selector.contains(word) || summary.contains(word) || card.contains(word) {
            matches.push((selector, summary.trim()));
        }
    }
    matches.sort_by(|a, b| a.0.cmp(&b.0));
    if matches.is_empty() {
        return Err(format!("card search has no match for `{word}`; library declarations use `help language find:{word}:0`"));
    }
    if start >= matches.len() {
        return Err("card search offset is outside its matching inventory".to_owned());
    }
    let mut output = format!("Authoring cards: {}; start: {start}; budget: {budget} UTF-8 bytes\nDiscovery only; read the exact card for profiles and prerequisites.\n", matches.len());
    let mut next = start;
    while next < matches.len() {
        let (selector, summary) = &matches[next];
        let row = format!("\n{selector} {summary}\nDetails: help language {selector}\n");
        let footer = footer(word, next + 1, matches.len());
        if output.len() + row.len() + footer.len() > budget {
            break;
        }
        output.push_str(&row);
        next += 1;
    }
    if next == start {
        return Err(format!(
            "card search entry exceeds page budget; use `help language {}`",
            matches[start].0
        ));
    }
    output.push_str(&footer(word, next, matches.len()));
    Ok(output)
}

fn footer(word: &str, next: usize, total: usize) -> String {
    let coverage = if next == total {
        "\nCoverage: complete matching authoring cards.\n".to_owned()
    } else {
        format!(
            "\nCoverage: partial authoring cards; next: help language author:find:{word}:{next}\n"
        )
    };
    format!("{coverage}Library declarations: help language find:{word}:0\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_cards_are_discoverable_by_operation_identity_and_route() {
        assert!(search(
            &format!("author:find:{}:0", "a".repeat(65)),
            super::super::MAX_BYTES
        )
        .is_err());
        for (word, expected) in [
            ("string_take", "author:string-take"),
            ("core.string.take", "author:string-take"),
            ("byte-traversal", "author:byte-traversal"),
            ("nested", "author:byte-traversal"),
        ] {
            let page = search(&format!("author:find:{word}:0"), super::super::MAX_BYTES).unwrap();
            assert!(page.contains(expected), "{word}: {page}");
            assert!(page.len() <= super::super::MAX_BYTES);
            assert!(page.contains("read the exact card for profiles and prerequisites"));
            assert!(page.contains(&format!(
                "Library declarations: help language find:{word}:0"
            )));
        }
        for query in [
            "author:find::0",
            "author:find:string_take:00",
            "author:find:string_take:-1",
            "author:find:string_take:1:extra",
            "author:find:string_take:999999",
            "author:find:STRING_TAKE:0",
            "author:find:core.string.no_take:0",
            "author:find:é:0",
            "author:find:string take:0",
        ] {
            assert!(search(query, super::super::MAX_BYTES).is_err(), "{query}");
        }
    }

    #[test]
    fn complete_card_pages_have_exact_coverage_order_and_byte_frontiers() {
        let expected = super::super::ROUTES
            .lines()
            .filter_map(|line| {
                line.strip_prefix("  author:")
                    .map(|tail| format!("author:{}", tail.split_once(' ').unwrap().0))
            })
            .collect::<std::collections::BTreeSet<_>>();
        let mut seen = Vec::new();
        let mut offset = 0;
        loop {
            let query = format!("author:find:author:{offset}");
            let page = search(&query, 700).unwrap();
            assert_eq!(page, search(&query, 700).unwrap());
            assert!(page.len() <= 700);
            let rows = page
                .lines()
                .filter_map(|line| {
                    line.strip_prefix("author:")
                        .map(|tail| format!("author:{}", tail.split_once(' ').unwrap().0))
                })
                .collect::<Vec<_>>();
            assert!(!rows.is_empty());
            offset += rows.len();
            seen.extend(rows);
            if page.contains("Coverage: complete matching authoring cards.") {
                break;
            }
            assert!(page.contains(&format!("next: help language author:find:author:{offset}")));
            assert!(offset < expected.len());
        }
        assert_eq!(seen.len(), expected.len());
        assert!(seen.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            seen.into_iter().collect::<std::collections::BTreeSet<_>>(),
            expected
        );
        let query = "author:find:core.string.take:0";
        let budget = (1..=super::super::MAX_BYTES)
            .find(|b| search(query, *b).is_ok())
            .unwrap();
        let page = search(query, budget).unwrap();
        assert_eq!(page.len(), budget);
        assert!(search(query, budget - 1).is_err());
        assert_eq!(
            search(query, budget + 1)
                .unwrap()
                .lines()
                .skip(1)
                .collect::<Vec<_>>(),
            page.lines().skip(1).collect::<Vec<_>>()
        );
    }
}
