//! A bounded duplicate prefilter over independently validated ASCII identifiers.
//! A fingerprint is never equality authority: matching fingerprints always run
//! the original decoded-token comparison, including arbitrary hash collisions.
use super::*;

pub(super) fn source(record: &TypeDeclaration) -> String {
    let id = &record.stable_id;
    let name = &record.name;
    format!(
        r#"@id("{id}.json.identifier-fingerprint")
fn json_{name}_identifier_fingerprint(input:borrow Slice<u8>,start:usize,end:usize)->usize {{
if !json_{name}_identifier_valid(input,start,end){{0usize}}else{{
let mut fingerprint=0usize;let mut cursor=start+1usize;
while cursor<end-1usize {{
let byte=usize_from_i64(jv_emit_at(input,cursor,0usize));
fingerprint=(fingerprint*257usize+byte)%65521usize;
cursor=jv_token_end(input,cursor);cursor<end-1usize
}}
fingerprint
}}
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    const SCHEMA: &str = r#"module schema;
@id("app.patient") record Patient {@id("app.patient.id") id:string,@id("app.patient.x") x:i64,}
@id("app.request") record Request {@id("app.request.servers") servers:Vec<string>,@id("app.request.patients") patients:Vec<Patient>,}
@id("schema.main") fn main()->i64{0}
"#;
    fn fingerprint(bytes: &[u8]) -> usize {
        bytes
            .iter()
            .fold(0, |value, byte| (value * 257 + usize::from(*byte)) % 65521)
    }
    #[test]
    fn collision_cannot_become_decoded_equality_authority() {
        // 257² - 2*257 - 14 = 65521; both operands are valid identifiers.
        assert_ne!(b"BAA", b"ACO");
        assert_eq!(fingerprint(b"BAA"), fingerprint(b"ACO"));
        let program = crate::parse(SCHEMA, "schema.spx").unwrap();
        let output = request_source(&program, &program.types[1]).unwrap();
        assert!(output.contains("fingerprint==vec_get<usize>(server_fingerprints,prior) && jv_decoded_token_eq(input,other.start,cursor)"));
        assert!(output.contains("fingerprint==vec_get<usize>(patient_fingerprints,prior) && jv_decoded_token_eq(input,other.id_start,adjusted.id_start)"));
        assert!(output.contains("jv_strict_end(input,32usize,0)"));
        assert!(output.contains("vec_with_capacity<usize>(8usize)"));
        assert!(output.contains("vec_with_capacity<usize>(256usize)"));
        let parsed = crate::parse(&output, "fingerprinted-request.spx").unwrap();
        let canonical = crate::format::canonical(&parsed);
        assert_eq!(
            canonical,
            crate::format::canonical(&crate::parse(&canonical, "canonical.spx").unwrap())
        );
    }
    #[test]
    fn decoded_fingerprint_arithmetic_is_bounded_without_wrapping() {
        assert!(65520usize * 257 + 122 < u32::MAX as usize);
        let program = crate::parse(SCHEMA, "schema.spx").unwrap();
        let output = source(&program.types[0]);
        assert!(output.contains("if !json_Patient_identifier_valid(input,start,end){0usize}else{"));
        assert!(output.contains("jv_emit_at(input,cursor,0usize)"));
        assert!(output.contains("cursor=jv_token_end(input,cursor)"));
    }
}
