//! Compare optimized ordinary source with the original independent scalar path.
use super::*;

#[test]
fn ascii_fast_path_matches_original_scalar_validation_and_keeps_utf8_boundaries() {
    let mut app = imports().to_owned();
    app.push_str(
        r#"
use function @id("unicode.row.json.utf8.valid") from consumer.schema as optimized;
use function @id("unicode.row.json.utf8.owned-valid") from consumer.schema as optimized_owned;
use function @id("unicode.row.json.utf8.quoted-len") from consumer.schema as optimized_quoted;
use function @id("std.data.json.query.scan_string") from std.data.json.query as jv_scan_string;
use function @id("std.data.json.query.decoded_len") from std.data.json.query as jv_decoded_len;
use function @id("std.data.json.query.token_end") from std.data.json.query as jv_token_end;
use function @id("std.data.json.query.scalar_at") from std.data.json.query as ju_escape_scalar;
use function @id("std.data.json.utf8.scalar_at") from std.data.json.utf8 as ju_raw_scalar;
use function @id("std.data.json.utf8.sequence_end") from std.data.json.utf8 as ju_raw_end;
use function @id("std.data.json.utf8.utf8_end") from std.data.json.utf8 as ju_raw_utf8_end;
use function @id("std.data.json.utf8.bounded_valid") from std.data.json.utf8 as shared_valid;
use function @id("std.data.json.utf8.quoted_len") from std.data.json.utf8 as shared_quoted;
use function @id("std.data.json.digits.i64_len") from std.data.json.digits as legacy_i64_len;
"#,
    );
    app.push_str(include_str!("ascii_legacy_validator.txt"));
    app.push_str(include_str!("utf8_end_legacy.txt"));
    app.push_str(include_str!("quoted_legacy_kernels.txt"));
    app.push_str(include_str!("encoded_len_legacy.txt"));
    app.push_str(
        r#"
@id("consumer.compare_raw_utf8") fn compare_raw_utf8(input:borrow Slice<u8>,start:usize,wanted:usize)->i64 {
 let old=legacy_utf8_end(input,start);
 let current=ju_raw_utf8_end(input,start);
 if current==old && current==wanted {0} else {1}
}
@id("consumer.compare") fn compare(input:borrow Slice<u8>,start:usize,end:usize,wanted:bool)->i64 {
 let old=json_Legacy_identifier_valid(input,start,end);
 let current=optimized(input,start,end);
 if current==old && current==wanted {0} else {1}
}
@id("consumer.compare_owned") fn compare_owned(text:borrow str,wanted:bool)->i64 {
 let bytes=str_as_bytes(text);
 let old=byte_len(bytes)<=64usize && legacy_utf8_end(bytes,0usize)==byte_len(bytes);
 let current=optimized_owned(text);
 let previous_length=json_Legacy_utf8_quoted_len(text);
 let current_length=optimized_quoted(text);
 let words=vec_push<string>(vec_with_capacity<string>(1usize),string_from_str(text));
 let rows=vec_push<Row>(vec_with_capacity<Row>(1usize),Row{number:-9223372036854775808,text:string_from_str(text)});
 let old_size=legacy_encoded_len(words,rows);
 let new_size=encoded_len(words,rows);
 let wanted_size=if wanted{62usize+previous_length+previous_length}else{18446744073709551615usize};
 if current==old && current==wanted && current_length==previous_length && old_size==new_size && new_size==wanted_size {0} else {1}
}
@id("consumer.main") fn main()->i64 {
 let mut bad=0;
"#,
    );
    // Every ASCII byte, including raw forbidden controls, quote and backslash.
    // The immutable Rust input and truth table are independent of both decoders.
    let all_ascii: Vec<u8> = (0..=255).flat_map(|byte| [b'"', byte, b'"']).collect();
    app.push_str(&format!("let ascii={};let input=array_as_slice(ascii);let mut index=0usize;while index<256usize{{let start=index*3usize;let wanted=index>=32usize && index<128usize && index!=34usize && index!=92usize;bad=bad+compare(input,start,start+3usize,wanted);index=index+1usize;index<256usize}}\n", array(&all_ascii)));
    // Each of the 256 original bytes also exercises the raw byte kernels.
    // High single bytes are invalid UTF-8; ASCII quote cost is independent.
    app.push_str("let mut byte_index=0usize;while byte_index<256usize{let start=byte_index*3usize+1usize;let one=byte_range(input,start,start+1usize);let valid=shared_valid(one,64usize);let quoted=shared_quoted(one,64usize);let expected=if byte_index>=128usize{18446744073709551615usize}else{2usize+(if byte_index==34usize || byte_index==92usize || byte_index==8usize || byte_index==12usize || byte_index==10usize || byte_index==13usize || byte_index==9usize{2usize}else{if byte_index<32usize{6usize}else{1usize}})};bad=bad+(if valid==(byte_index<128usize) && quoted==expected{0}else{1});byte_index=byte_index+1usize;byte_index<256usize}\n");
    // UTF-8 includes every ASCII control/NUL; JSON-token validity is separate.
    app.push_str("let mut raw_index=0usize;while raw_index<256usize{let start=raw_index*3usize+1usize;let one=byte_range(input,start,start+1usize);let wanted=if raw_index<128usize{1usize}else{2usize};bad=bad+compare_raw_utf8(one,0usize,wanted);bad=bad+compare_raw_utf8(one,1usize,1usize);bad=bad+compare_raw_utf8(one,18446744073709551615usize,1usize);raw_index=raw_index+1usize;raw_index<256usize}\n");
    let raw_cases: &[(&[u8], usize, usize)] = &[
        (b"", 0, 0),
        (b"", usize::MAX, 0),
        (b"A\0\x7f", 0, 3),
        (b"A\0\x7f", 1, 3),
        ("é😀".as_bytes(), 0, 6),
        ("é😀".as_bytes(), 2, 6),
        ("é😀".as_bytes(), 1, 8),
        ("é😀".as_bytes(), usize::MAX, 6),
        (b"A\xe2\x82", 0, 5),
        (b"A\xe2\x82", 2, 6),
        (b"\xc0\xaf", 0, 3),
        (b"\xed\xa0\x80", 0, 4),
        (b"\xf4\x90\x80\x80", 0, 5),
        (b"A\x80B", 0, 5),
        (b"A\x80B", 2, 3),
        (b"A\x80B", 4, 3),
    ];
    for (i, (bytes, start, wanted)) in raw_cases.iter().enumerate() {
        app.push_str(&format!("let raw_utf8_{i}={};bad=bad+compare_raw_utf8(array_as_slice(raw_utf8_{i}),{start}usize,{wanted}usize);\n",array(bytes)));
    }
    app.push_str("bad=bad+(if !shared_valid(input,18446744073709551615usize) && shared_quoted(input,65usize)==18446744073709551615usize{0}else{1});\n");
    app.push_str("bad=bad+compare(input,0usize,0usize,false);bad=bad+compare(input,768usize,769usize,false);bad=bad+compare(input,0usize,769usize,false);\n");
    app.push_str("bad=bad+compare(input,18446744073709551615usize,18446744073709551615usize,false);bad=bad+compare(input,0usize,18446744073709551615usize,false);\n");
    let cases: &[(&[u8], bool)] = &[
        (br#""A"B""#, false),
        (b"A\"", false),
        (b"\"A", false),
        (br#""\u0000""#, true),
        (br#""\n\t\"\\""#, true),
        ("\"é😀\"".as_bytes(), true),
        (br#""\u00e9\ud83d\ude00""#, true),
        (b"\"\xc0\xaf\"", false),
        (b"\"\xed\xa0\x80\"", false),
        (b"\"\xf4\x90\x80\x80\"", false),
        (b"\"A\xe2\x82\"", false),
        (br#""\ud800""#, false),
        (br#""\udc00""#, false),
        (br#""\x41""#, false),
    ];
    for (i, (bytes, wanted)) in cases.iter().enumerate() {
        app.push_str(&format!(
            "let raw_{i}={};bad=bad+compare(array_as_slice(raw_{i}),0usize,{}usize,{wanted});\n",
            array(bytes),
            bytes.len()
        ));
    }
    for (i, (payload, wanted)) in [
        (String::new(), true),
        ("A".to_owned(), true),
        ("A".repeat(64), true),
        ("A".repeat(65), false),
    ]
    .iter()
    .enumerate()
    {
        let raw = format!("\"{payload}\"");
        app.push_str(&format!("let token_{i}={};bad=bad+compare(array_as_slice(token_{i}),0usize,{}usize,{wanted});\n", array(raw.as_bytes()), raw.len()));
    }
    for (i, (text, wanted)) in [
        (String::new(), true),
        ("\0\n\t\u{7f}".to_owned(), true),
        ("é😀".to_owned(), true),
        ("A".repeat(64), true),
        ("A".repeat(65), false),
        (format!("{}é", "A".repeat(62)), true),
        (format!("{}é", "A".repeat(63)), false),
    ]
    .iter()
    .enumerate()
    {
        // SEMAPRAX source uses braced scalar escapes, independently of JSON.
        let mut literal = String::from("\"");
        for ch in text.chars() {
            match ch {
                '\\' => literal.push_str("\\\\"),
                '"' => literal.push_str("\\\""),
                ch if ch.is_control() => literal.push_str(&format!("\\u{{{:x}}}", u32::from(ch))),
                ch => literal.push(ch),
            }
        }
        literal.push('"');
        app.push_str(&format!(
            "let owned_{i}={literal};bad=bad+compare_owned(string_as_str(owned_{i}),{wanted});\n"
        ));
    }
    // Include actual decoder ownership/canonical call paths in every backend.
    let request = br#"{"labels":[],"rows":[]}"#;
    app.push_str(&format!("let request={};let decoded=decode(array_as_slice(request));match own decoded{{Outcome::Error{{code,offset,field}}=>0,Outcome::Decoded{{labels,rows}}=>if bad==0 && vec_len<string>(labels)==0usize && vec_len<Row>(rows)==0usize{{727}}else{{0}},}}}}\n", array(request)));
    let root = install("utf8-ascii-differential", &app, 64);
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let graph = snapshot.semantic_graph();
        assert!(graph.contains("unicode.row.json.utf8.valid"));
        assert!(graph.contains("consumer.legacy.json.utf8.valid"));
        Ok(())
    })
    .unwrap();
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}
