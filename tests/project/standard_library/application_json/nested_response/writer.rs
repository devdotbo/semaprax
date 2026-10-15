//! OPT-737: independent bytes and physical quote phase; definitions, not evidence.
use super::*;

#[test]
fn writer_all_control_escapes_and_numeric_extrema_preserve_original_wire_bytes() {
    let schema = SCHEMA.replace("retry:usize", "retry:i64");
    // Independent standard JSON encoder: every control, quote, backslash,
    // non-ASCII scalar and NUL appears, without borrowing generator logic.
    let payload: String = (0..32)
        .map(char::from)
        .chain(['"', '\\', 'é', '😀'])
        .collect();
    let escaped = serde_json::to_string(&payload).unwrap();
    let expected=format!("{{\"config\":{{\"label\":{escaped},\"retry\":-9223372036854775808}},\"rows\":[],\"ok\":false}}");
    let scalars = payload
        .chars()
        .map(|value| {
            format!(
                "text=string_concat(text,string_from_char(char_from_i64({})));",
                u32::from(value)
            )
        })
        .collect::<String>();
    let app = imports()
        + &format!(
            r#"
@id("consumer.main") fn main()->i64{{
let mut text="";{scalars}
let report=Report{{config:Config{{label:text,retry:-9223372036854775808}},rows:vec_with_capacity<Row>(0usize),ok:false}};
let required=encoded_len(report);let short=encode(report,required-1usize);
let refused=match own short{{Encoded::Refused{{required:n}}=>n==required,Encoded::Encoded{{text:t}}=>false,}};
let full=encode(report,required);let expected={};
let same=match own full{{Encoded::Refused{{required:n}}=>false,Encoded::Encoded{{text:t}}=>equal(string_as_str(t),string_as_str(expected)),}};
if refused && same && required=={}usize{{724}}else{{0}}
}}
"#,
            quote(&expected),
            expected.len()
        );
    let root = install_policy(
        "response-writer-controls",
        &schema,
        &app,
        project::JsonCodecProfile::NestedResponse {
            max_string_bytes: 64,
            max_array_items: 8,
        },
    );
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

fn symbol(id: &str) -> String {
    format!(
        "spx_decl_{}",
        id.bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

#[test]
fn derived_quote_writer_reserves_exact_bytes_and_copies_final_string_once() {
    let app = r#"module consumer.app;
use function @id("r.root.json.nested-response.owned-valid") from consumer.schema as valid;
use function @id("r.root.json.nested-response.quoted-len") from consumer.schema as quoted_len;
use function @id("r.root.json.nested-response.writer.quote") from consumer.schema as quote_write;
@id("consumer.writer") fn render(input:borrow str,capacity:usize)->string
requires valid(input) && quoted_len(input)==capacity {
let buffer=bytes_zeroed_bounded(capacity,131072usize);
let written=quote_write(input,0usize,buffer);
string_from_utf8(bytes_as_slice(written))
}
@id("consumer.main") fn main()->i64{let input="";let text=render(string_as_str(input),2usize);if byte_len(str_as_bytes(string_as_str(text)))==2usize{724}else{0}}
"#;
    let root = install_policy(
        "response-writer-physical",
        SCHEMA,
        app,
        project::JsonCodecProfile::NestedResponse {
            max_string_bytes: 64,
            max_array_items: 8,
        },
    );
    let generated = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])
    })
    .unwrap();
    let mut cases = String::new();
    for payload in [
        String::new(),
        "a".repeat(16),
        "a".repeat(64),
        "é".repeat(32),
        (0..32).map(char::from).collect(),
        "\"\\".repeat(32),
    ] {
        let expected = serde_json::to_string(&payload).unwrap();
        let bytes = |value: &[u8]| {
            value
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(",")
        };
        // A trailing sentinel gives empty input a nonempty independent array.
        cases.push_str(&format!("{{static const uint8_t input[]={{ {} }};static const uint8_t expected[]={{ {} }};CHECK_CASE(input,{},expected,{});}}\n",if payload.is_empty() { "0".to_owned() } else { format!("{},0", bytes(payload.as_bytes())) },bytes(expected.as_bytes()),payload.len(),expected.len()));
    }
    let observer = include_str!("../../../../native_owned_utf8_settlement_v1/allocations.c");
    let copies = include_str!("../../../../native/string_settlement/bulk_utf8/copies.c");
    let probe = include_str!("writer.c");
    let source=format!("#define FIXTURE_TRACK_CALLOC\n{observer}\n{copies}\n#undef calloc\nstatic size_t writer_reserved_size,writer_reservations;\nstatic void *writer_calloc(size_t count,size_t size){{writer_reserved_size=count*size;++writer_reservations;return fixture_calloc(count,size);}}\n#define calloc writer_calloc\n#define main fixture_generated_main\n{generated}\n#undef main\n#undef malloc\n#undef calloc\n#undef free\n#undef memcpy\n#define WRITER {}\n{probe}\nint main(void){{struct spx_status_entry entries[16];struct spx_context context={{0}};REQUIRE(spx_context_init(&context,737,entries,16,NULL,NULL,NULL));{cases}(void)puts(\"724\");return 0;}}",symbol("consumer.writer"));
    for optimization in ["-O0", "-O2"] {
        super::super::super::compile_and_run_c(&source, &root, optimization, "724");
    }
    std::fs::remove_dir_all(root).unwrap();
}
