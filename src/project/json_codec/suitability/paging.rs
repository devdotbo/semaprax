//! Render a whole row once, moving it into the envelope. Earlier profiles
//! were already inspected; paging cannot hide their generation failures.
use super::*;
pub(super) fn page(
    mut envelope: Value,
    count: usize,
    offset: usize,
    maximum: usize,
    mut render: impl FnMut(usize) -> Value,
) -> Result<String, Vec<Diagnostic>> {
    if offset >= count {
        return Err(refusal(
            "codec explanation offset is outside the complete profile inventory",
        ));
    }
    envelope["profiles"] = Value::Array(Vec::new());
    let mut accepted = 0;
    for index in offset..count {
        envelope["profiles"]
            .as_array_mut()
            .expect("profile array")
            .push(render(index));
        bind(&mut envelope, index + 1, count);
        let output =
            serde_json::to_string(&envelope).expect("fixed explanation fields serialize") + "\n";
        if output.len() > maximum {
            envelope["profiles"]
                .as_array_mut()
                .expect("profile array")
                .pop();
            break;
        }
        accepted += 1;
    }
    if accepted == 0 {
        return Err(refusal(
            "codec explanation byte budget cannot hold one complete profile row and its binding",
        ));
    }
    bind(&mut envelope, offset + accepted, count);
    let output =
        serde_json::to_string(&envelope).expect("fixed explanation fields serialize") + "\n";
    if output.len() > maximum {
        return Err(refusal(
            "codec explanation envelope exceeds its byte budget",
        ));
    }
    Ok(output)
}
fn bind(envelope: &mut Value, next: usize, count: usize) {
    envelope["next_offset"] = if next == count {
        Value::Null
    } else {
        json!(next)
    };
    envelope["coverage"] = json!(if next == count {
        "complete"
    } else {
        "continued"
    });
}
