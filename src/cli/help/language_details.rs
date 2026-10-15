//! Exact bounded retrieval of complete full-reference appendix paragraphs.
const MAX_BYTES: usize = 2_048;
const DETAILS: &[(&str, &str)] = &[
    ("records", "Record profile details"),
    ("io", "Network and filesystem details"),
    ("boxes", "Box profile details"),
    ("record-traversal", "Scoped record traversal details"),
    ("field-views", "Project field-view details"),
    ("imports", "Record import visibility details"),
];
pub(super) fn lookup(query: &str, reference: &str) -> Result<String, String> {
    let heading = query
        .strip_prefix("detail:")
        .and_then(|slug| {
            DETAILS
                .iter()
                .find_map(|(key, heading)| (*key == slug).then_some(*heading))
        })
        .ok_or_else(|| format!("language card has no exact detail `{query}`"))?;
    let mut appendix = reference
        .match_indices("## Detailed profile rules\n")
        .filter(|(index, _)| *index == 0 || reference.as_bytes()[*index - 1] == b'\n');
    let start = appendix
        .next()
        .map(|(index, _)| index)
        .filter(|_| appendix.next().is_none())
        .ok_or_else(|| "language detail reference has no unique appendix".to_owned())?;
    let appendix = &reference[start..];
    let appendix = &appendix[..appendix.find("\n## ").unwrap_or(appendix.len())];
    let marker = format!("### {heading}\n");
    let mut found = appendix
        .match_indices(&marker)
        .filter(|(index, _)| *index == 0 || appendix.as_bytes()[*index - 1] == b'\n');
    let start = found
        .next()
        .map(|(index, _)| index)
        .filter(|_| found.next().is_none())
        .ok_or_else(|| format!("language detail `{query}` has no unique heading"))?;
    let section = &appendix[start..];
    let end = section.find("\n### ").unwrap_or(section.len());
    let section = &section[..end];
    if section[marker.len()..].trim().is_empty() {
        return Err(format!(
            "language detail `{query}` has no complete paragraph"
        ));
    }
    let output = super::reference_links::inline_section(section, reference)?;
    if output.len() > MAX_BYTES {
        return Err(format!(
            "language detail `{query}` exceeds {MAX_BYTES} bytes"
        ));
    }
    Ok(output)
}
#[cfg(test)]
mod tests;
