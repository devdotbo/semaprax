//! Saved eager paging oracle from 5bf43b4dd.
use super::*;
pub(super) fn page(
    mut envelope: Value,
    rows: &[Value],
    offset: usize,
    maximum: usize,
) -> Result<String, Vec<Diagnostic>> {
    if offset >= rows.len() {
        return Err(refusal(
            "codec explanation offset is outside the complete profile inventory",
        ));
    }
    let mut accepted = Vec::new();
    for row in &rows[offset..] {
        accepted.push(row.clone());
        let next = offset + accepted.len();
        envelope["profiles"] = json!(accepted);
        envelope["next_offset"] = if next == rows.len() {
            Value::Null
        } else {
            json!(next)
        };
        envelope["coverage"] = json!(if next == rows.len() {
            "complete"
        } else {
            "continued"
        });
        let output =
            serde_json::to_string(&envelope).expect("fixed explanation fields serialize") + "\n";
        if output.len() > maximum {
            accepted.pop();
            break;
        }
    }
    if accepted.is_empty() {
        return Err(refusal(
            "codec explanation byte budget cannot hold one complete profile row and its binding",
        ));
    }
    let next = offset + accepted.len();
    envelope["profiles"] = json!(accepted);
    envelope["next_offset"] = if next == rows.len() {
        Value::Null
    } else {
        json!(next)
    };
    envelope["coverage"] = json!(if next == rows.len() {
        "complete"
    } else {
        "continued"
    });
    let output =
        serde_json::to_string(&envelope).expect("fixed explanation fields serialize") + "\n";
    if output.len() > maximum {
        return Err(refusal(
            "codec explanation envelope exceeds its byte budget",
        ));
    }
    Ok(output)
}
