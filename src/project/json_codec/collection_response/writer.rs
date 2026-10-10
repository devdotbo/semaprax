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
    // Keep the owned renewal itself an exact direct Call. A Block-valued RHS
    // would consume the owner without carrying that ordinary renewal authority.
    format!("{{let key=[{bytes}];{output}={prefix}_writer_bytes(array_as_slice(key),{position},{output});0}};{position}={position}+{}usize;\n",value.len())
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
    fn scoped_literal_writes_admit_exact_helpers_inside_and_outside_loops() {
        let schema = crate::parse(
            "module schema;@id(\"response.row\") record Row{@id(\"response.row.x\")x:i64,}@id(\"schema.main\") fn main()->i64{0}",
            "schema.spx",
        ).unwrap();
        let helpers = source(&schema.types[0], "collection-response", "response");
        let bytes_helper = helpers
            .split("@id(\"response.row.json.collection-response.writer.digit\")")
            .next()
            .unwrap();
        let first = literal_write("json_Row_response", "output", "cursor", "\"first\":");
        let second = literal_write("json_Row_response", "output", "cursor", ",\"second\":");
        let text = format!(
            "module writer;\n{bytes_helper}\n@id(\"writer.main\") fn main()->i64{{
let mut output=bytes_zeroed(128usize);let mut cursor=0usize;
{first}let mut at=0usize;
while at<2usize{{{first}{second}at=at+1usize;0}}
if cursor==44usize{{1}}else{{0}}}}\n"
        );
        let checked = crate::check(&text, "scoped-writer.spx").unwrap();
        let canonical = crate::format::canonical(&checked);
        let checked_again = crate::check(&canonical, "scoped-writer-canonical.spx").unwrap();
        assert_eq!(canonical, crate::format::canonical(&checked_again));
        let helper = checked_again
            .functions
            .iter()
            .find(|f| f.name == "json_Row_response_writer_bytes")
            .unwrap();
        assert!(crate::loop_calls::byte_writer::source_target(
            &checked_again,
            helper
        ));
        let resolved = crate::hir::resolve(&checked_again).unwrap();
        crate::hir::validate(&resolved).unwrap();
        crate::cleanup_plan::validate_program(&resolved).unwrap();
        let main = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == "writer.main")
            .unwrap();
        assert_eq!(
            main.cleanup_plan.schema,
            crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
        );
        assert_eq!(
            crate::loop_calls::byte_writer::checked_bindings(&resolved, main)
                .unwrap()
                .len(),
            3
        );
        // Arbitrary Block-valued ownership assignment still has no direct-call
        // authority; correcting the generator must not broaden that boundary.
        let old_rhs = first
            .replace("{let key=", "output={let key=")
            .replace(
                ";output=json_Row_response_writer_bytes",
                ";json_Row_response_writer_bytes",
            )
            .replace(";0};", "};");
        let hostile = text.replacen(&first, &old_rhs, 1);
        let errors = crate::check(&hostile, "block-rhs-writer.spx").unwrap_err();
        assert!(errors.iter().any(|e| e.code == "SPX-U105"));
        assert!(errors.iter().any(|e| e.code == "SPX-O101"));
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
