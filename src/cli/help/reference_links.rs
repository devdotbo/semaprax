use std::collections::BTreeMap;
use std::fmt::Write as _;

// Exact isolated cards retain their prose but make only referenced links
// self-contained. Full-card bytes and unrelated definitions remain untouched.
pub(super) fn inline_section(section: &str, reference: &str) -> Result<String, String> {
    let mut definitions = BTreeMap::new();
    let mut fence = None;
    for line in reference.lines() {
        if code_fence(line, &mut fence) {
            continue;
        }
        if let Some((name, target)) = definition(line) {
            if definitions.insert(name, target).is_some() {
                return Err(format!(
                    "language card has duplicate link definition `{name}`"
                ));
            }
        }
    }
    let mut output = String::with_capacity(section.len());
    let mut fence = None;
    let mut inline_ticks = 0;
    for line in section.split_inclusive('\n') {
        if code_fence(line, &mut fence) {
            output.push_str(line);
            continue;
        }
        if inline_ticks == 0 && definition(line.trim_end_matches('\n')).is_some() {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find(['[', '`']) {
            output.push_str(&rest[..start]);
            rest = &rest[start..];
            if rest.starts_with('`') {
                let width = rest.bytes().take_while(|byte| *byte == b'`').count();
                if inline_ticks == 0 {
                    inline_ticks = width;
                } else if inline_ticks == width {
                    inline_ticks = 0;
                }
                output.push_str(&rest[..width]);
                rest = &rest[width..];
                continue;
            }
            let follows_index = output
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_alphanumeric() || ch == '_' || ch == ']');
            if inline_ticks != 0 || follows_index {
                output.push('[');
                rest = &rest[1..];
                continue;
            }
            let Some(close) = rest.find("][") else {
                break;
            };
            if rest[1..close].contains('[') {
                output.push('[');
                rest = &rest[1..];
                continue;
            }
            let name_start = close + 2;
            let Some(end) = rest[name_start..].find(']') else {
                break;
            };
            let end = name_start + end;
            let name = &rest[name_start..end];
            let target = definitions
                .get(name)
                .ok_or_else(|| format!("language card has no exact link definition `{name}`"))?;
            write!(output, "[{}]({target})", &rest[1..close])
                .expect("writing to a string cannot fail");
            rest = &rest[end + 1..];
        }
        output.push_str(rest);
    }
    Ok(output)
}

fn code_fence(line: &str, fence: &mut Option<(char, usize)>) -> bool {
    let trimmed = line.trim_start();
    let Some(character) = trimmed.chars().next() else {
        return fence.is_some();
    };
    if character != '`' && character != '~' {
        return fence.is_some();
    }
    let width = trimmed.chars().take_while(|ch| *ch == character).count();
    if width < 3 {
        return fence.is_some();
    }
    if let Some((opened, minimum)) = *fence {
        if character == opened && width >= minimum && trimmed[width..].trim().is_empty() {
            *fence = None;
        }
    } else {
        *fence = Some((character, width));
    }
    true
}

fn definition(line: &str) -> Option<(&str, &str)> {
    let (name, target) = line.strip_prefix('[')?.split_once("]: ")?;
    (!name.is_empty() && !name.contains(']') && !target.is_empty()).then_some((name, target))
}

#[cfg(test)]
mod tests {
    use super::inline_section;

    #[test]
    fn isolated_topic_expands_only_used_links_and_preserves_unicode_and_inline_links() {
        let reference = "[vec]: OWNED-BOUNDED-VEC-V1.md\n[unused]: PRIVATE.md\n";
        assert_eq!(
            inline_section(
                "## Vec\n\n命 [Vector][vec]; [inline](RULE.md).\n",
                reference
            )
            .unwrap(),
            "## Vec\n\n命 [Vector](OWNED-BOUNDED-VEC-V1.md); [inline](RULE.md).\n"
        );
        assert_eq!(
            inline_section("## Last\n[vec]: OWNED-BOUNDED-VEC-V1.md\n", reference).unwrap(),
            "## Last\n"
        );
    }

    #[test]
    fn missing_and_duplicate_definitions_refuse_instead_of_rendering_dangling_links() {
        assert!(inline_section("[Vector][vec]", "")
            .unwrap_err()
            .contains("no exact link definition"));
        assert!(
            inline_section("[Vector][vec]", "[vec]: A.md\n[vec]: B.md\n")
                .unwrap_err()
                .contains("duplicate link definition")
        );
        assert_eq!(
            inline_section("ordinary [bracket] text", "").unwrap(),
            "ordinary [bracket] text"
        );
        let literal = "matrix[0][1] ` [x][absent] `\n```spx\n[x]: Y\n[x][absent]\n```\n";
        assert_eq!(inline_section(literal, literal).unwrap(), literal);
        assert_eq!(
            inline_section("``[x][absent] ` literal``", "").unwrap(),
            "``[x][absent] ` literal``"
        );
    }
}
