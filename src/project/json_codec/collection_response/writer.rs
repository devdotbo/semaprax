//! Ordinary functional Bytes writer. No new dependencies or generated-source authority.
use super::*;

pub(in crate::project::json_codec) fn source(
    root: &TypeDeclaration,
    profile: &str,
    namespace: &str,
) -> String {
    super::super::template::expand(
        include_str!("writer.spx"),
        &[
            ("__PREFIX__", &format!("json_{}_{}", root.name, namespace)),
            ("__ID__", &format!("{}.json.{profile}", root.stable_id)),
        ],
    )
}

/// Keys and punctuation are compile-time UTF-8 bytes, never temporary Strings.
pub(in crate::project::json_codec) fn literal_write(
    prefix: &str,
    output: &str,
    position: &str,
    value: &str,
) -> String {
    if value.len() == 1 {
        return format!(
            "{output}=bytes_set({output},{position},{}u8);{position}={position}+1usize;\n",
            value.as_bytes()[0]
        );
    }
    let bytes = value
        .bytes()
        .map(|byte| format!("{byte}u8"))
        .collect::<Vec<_>>()
        .join(",");
    // A fixed Copy array is ordinary stack data, not an owned String/Bytes
    // payload. Its borrow remains inside this statement's checked call.
    format!("let _={{let key=[{bytes}];{output}={prefix}_writer_bytes(array_as_slice(key),{position},{output});{position}={position}+{}usize;true}};\n",value.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn functional_writer_is_ordinary_deterministic_and_minimum_safe() {
        let program = crate::parse(
            "module schema;@id(\"response.row\") record Row{@id(\"response.row.x\")x:i64,}@id(\"schema.main\") fn main()->i64{0}",
            "schema.spx",
        )
        .unwrap();
        let output = source(&program.types[0], "collection-response", "response");
        assert_eq!(
            output,
            source(&program.types[0], "collection-response", "response")
        );
        assert!(!output.contains("string_concat"));
        assert!(!output.contains("string_from_"));
        assert!(!output.contains("bytes_zeroed"));
        assert!(!output.contains("use "));
        assert!(output.contains("if value>0{0-value}else{value}"));
        assert!(output.contains("usize_from_i64(0-remaining%10)"));
        assert!(output.contains("position<=byte_len(bytes_as_slice(output)) &&"));
        assert!(output.contains("next=bytes_set(next,cursor,byte)"));
        let parsed = crate::parse(&format!("module schema;\n{output}"), "writer.spx").unwrap();
        assert_eq!(parsed.functions.len(), 7);
        assert!(parsed
            .functions
            .iter()
            .all(|function| function.effects.is_empty()));
        assert!(parsed.functions.iter().all(|function| function
            .stable_id
            .starts_with("response.row.json.collection-response.writer.")));
    }
    #[test]
    fn literal_keys_use_copy_arrays_without_generated_source_expansion() {
        let key = format!("\"{}\":", "k".repeat(64));
        let output = literal_write("json_Root_response", "next", "cursor", &key);
        assert!(output.contains("let key=[34u8,107u8"));
        assert!(output.contains("writer_bytes(array_as_slice(key),cursor,next)"));
        assert!(output.contains("cursor=cursor+67usize"));
        assert_eq!(output.matches("bytes_set").count(), 0);
        assert!(!output.contains("string_"));
        assert!(output.len() < 512);
        assert_eq!(
            literal_write("unused", "next", "cursor", "}"),
            "next=bytes_set(next,cursor,125u8);cursor=cursor+1usize;\n"
        );
    }
}
