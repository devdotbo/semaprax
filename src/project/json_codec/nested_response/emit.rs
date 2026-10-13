//! Every size is checked before arithmetic; rendering starts after complete preflight.
use super::super::collection_response::writer;
use super::*;

const INVALID: &str = "18446744073709551615usize";
const LIMIT: &str = "131072usize";

pub(super) fn source(
    root: &TypeDeclaration,
    record: &Record<'_>,
    bound: usize,
    version: Version,
) -> String {
    let result = version.result();
    let mut out = object(root, record, bound, false, Some(""), version);
    let namespace = version.namespace();
    let name = &root.name;
    let family = version.family();
    let id = &root.stable_id;
    writeln!(out, "@id(\"{id}.json.{family}.encode-result\") variant {name}{result} {{
@id(\"{id}.json.{family}.encoded\") Encoded{{@id(\"{id}.json.{family}.text\") text:string,}},
@id(\"{id}.json.{family}.refused\") Refused{{@id(\"{id}.json.{family}.required\") required:usize,}},
}}
@id(\"{id}.json.{family}.encoded-len\") fn json_{name}_{namespace}_encoded_len(value:borrow {name})->usize {{json_{name}_{namespace}_object_len_0(value)}}
@id(\"{id}.json.{family}.encode\") fn json_{name}_{namespace}_encode(value:borrow {name},output_limit:usize)->{name}{result} {{
let required=json_{name}_{namespace}_encoded_len(value);
if required=={INVALID} || required>output_limit {{{name}{result}::Refused{{required:required}}}}else{{
let output=bytes_zeroed_bounded(required,131072usize);let finished=json_{name}_{namespace}_object_render_0(value,0usize,output);
{name}{result}::Encoded{{text:string_from_utf8(bytes_as_slice(finished))}}
}}
}}" ).unwrap();
    out
}

fn object(
    root: &TypeDeclaration,
    record: &Record<'_>,
    bound: usize,
    indexed: bool,
    root_path: Option<&str>,
    version: Version,
) -> String {
    let mut out = String::new();
    for field in &record.fields {
        match &field.kind {
            Kind::Record(child) => {
                let path = format!(
                    "{}.{}",
                    root_path.expect("flat vector rows have no child records"),
                    field.declaration.name
                );
                out.push_str(&object(root, child, bound, false, Some(&path), version));
            }
            Kind::Vector { element, kind } => {
                if let Kind::Record(child) = &**kind {
                    out.push_str(&object(root, child, bound, owns(child), None, version));
                }
                out.push_str(&vector(root, element, kind, field.ordinal, bound, version));
            }
            _ => {}
        }
    }
    let namespace = version.namespace();
    let name = &root.name;
    let family = version.family();
    let id = &root.stable_id;
    let n = record.ordinal;
    let parameters = if root_path.is_some() {
        format!("value:borrow {}", root.name)
    } else if indexed {
        format!("values:borrow Vec<{}>,at:usize", record.declaration.name)
    } else {
        format!(
            "value:{}{}",
            if owns(record) { "borrow " } else { "" },
            record.declaration.name
        )
    };
    let punctuation = 2 + record.fields.len() - 1
        + record
            .fields
            .iter()
            .map(|field| field.declaration.name.len() + 3)
            .sum::<usize>();
    writeln!(out, "@id(\"{id}.json.{family}.object-len.{n}\") fn json_{name}_{namespace}_object_len_{n}({parameters})->usize {{let mut total={punctuation}usize;let mut valid=true;").unwrap();
    for field in &record.fields {
        let access = access(record, field, indexed, root_path);
        let operand = if matches!(field.kind, Kind::Record(_)) && root_path.is_some() {
            "value"
        } else {
            &access
        };
        let size = length(root, &field.kind, field.ordinal, operand, version);
        // Lazy guards ensure a refused child cannot overflow or underflow the sum.
        writeln!(out, "let _ = if valid{{let size={size};if size=={INVALID} || size>{LIMIT}-total{{valid=false;false}}else{{total=total+size;true}}}}else{{false}};").unwrap();
    }
    let arguments = if indexed { "values,at" } else { "value" };
    writeln!(out, "if valid{{total}}else{{{INVALID}}}\n}}
@id(\"{id}.json.{family}.object-render.{n}\") fn json_{name}_{namespace}_object_render_{n}({parameters},position:usize,output:own Bytes)->Bytes
requires json_{name}_{namespace}_object_len_{n}({arguments})!={INVALID} && position<=byte_len(bytes_as_slice(output)) && json_{name}_{namespace}_object_len_{n}({arguments})<=byte_len(bytes_as_slice(output))-position {{let mut next=output;let mut cursor=position;").unwrap();
    out.push_str(&writer::literal_write(
        &format!("json_{name}_{namespace}"),
        "next",
        "cursor",
        "{",
        0,
    ));
    for (index, field) in record.fields.iter().enumerate() {
        let key = format!(
            "{}\"{}\":",
            if index == 0 { "" } else { "," },
            field.declaration.name
        );
        let access = access(record, field, indexed, root_path);
        let operand = if matches!(field.kind, Kind::Record(_)) && root_path.is_some() {
            "value"
        } else {
            &access
        };
        out.push_str(&writer::literal_write(
            &format!("json_{name}_{namespace}"),
            "next",
            "cursor",
            &key,
            index,
        ));
        let rendered = render(root, &field.kind, field.ordinal, operand, version);
        writeln!(
            out,
            "next={rendered};cursor=cursor+({});",
            length(root, &field.kind, field.ordinal, operand, version)
        )
        .unwrap();
    }
    out.push_str(&writer::literal_write(
        &format!("json_{name}_{namespace}"),
        "next",
        "cursor",
        "}",
        record.fields.len(),
    ));
    out.push_str("next\n}\n");
    out
}

fn access(
    record: &Record<'_>,
    field: &descriptor::Field<'_>,
    indexed: bool,
    root_path: Option<&str>,
) -> String {
    if indexed {
        format!(
            "vec_field<{}>(values,at,{})",
            record.declaration.name,
            literal(&field.declaration.name)
        )
    } else {
        let place = format!(
            "value{}.{}",
            root_path.unwrap_or(""),
            field.declaration.name
        );
        if matches!(field.kind, Kind::Text) {
            format!("string_as_str({place})")
        } else {
            place
        }
    }
}

fn length(
    root: &TypeDeclaration,
    kind: &Kind<'_>,
    ordinal: usize,
    access: &str,
    version: Version,
) -> String {
    let namespace = version.namespace();
    let name = &root.name;
    match kind {
        Kind::Text => format!("json_{name}_{namespace}_utf8_quoted_len({access})"),
        Kind::Scalar(Type::Bool) => format!("if {access}{{4usize}}else{{5usize}}"),
        Kind::Scalar(Type::Usize) => format!("jv_usize_len({access})"),
        Kind::Scalar(Type::U8) => format!("usize_from_i64(jv_i64_len(i64_from_u8({access})))"),
        Kind::Scalar(Type::I64) => format!("usize_from_i64(jv_i64_len({access}))"),
        Kind::Record(child) => format!(
            "json_{name}_{namespace}_object_len_{}({access})",
            child.ordinal
        ),
        Kind::Vector { .. } => format!("json_{name}_{namespace}_array_len_{ordinal}({access})"),
        Kind::Scalar(_) => unreachable!("validated scalar"),
    }
}
fn render(
    root: &TypeDeclaration,
    kind: &Kind<'_>,
    ordinal: usize,
    access: &str,
    version: Version,
) -> String {
    let namespace = version.namespace();
    let name = &root.name;
    match kind {
        Kind::Text => format!("json_{name}_{namespace}_writer_quote({access},cursor,next)"),
        Kind::Scalar(Type::Bool) => {
            format!("json_{name}_{namespace}_writer_bool({access},cursor,next)")
        }
        Kind::Scalar(Type::Usize) => {
            format!("json_{name}_{namespace}_writer_usize({access},cursor,next)")
        }
        Kind::Scalar(Type::U8) => {
            format!("json_{name}_{namespace}_writer_i64(i64_from_u8({access}),cursor,next)")
        }
        Kind::Scalar(Type::I64) => {
            format!("json_{name}_{namespace}_writer_i64({access},cursor,next)")
        }
        Kind::Record(child) => format!(
            "json_{name}_{namespace}_object_render_{}({access},cursor,next)",
            child.ordinal
        ),
        Kind::Vector { .. } => {
            format!("json_{name}_{namespace}_array_render_{ordinal}({access},cursor,next)")
        }
        Kind::Scalar(_) => unreachable!("validated scalar"),
    }
}

fn vector(
    root: &TypeDeclaration,
    element: &Type,
    kind: &Kind<'_>,
    ordinal: usize,
    bound: usize,
    version: Version,
) -> String {
    let namespace = version.namespace();
    let name = &root.name;
    let family = version.family();
    let id = &root.stable_id;
    let ty = type_name(element);
    let (bindings, size, rendered) = match kind {
        Kind::Text => (
            "let value=vec_str_at(values,at);".to_owned(),
            length(root, kind, ordinal, "value", version),
            render(root, kind, ordinal, "value", version),
        ),
        Kind::Record(child) if owns(child) => (
            String::new(),
            format!(
                "json_{name}_{namespace}_object_len_{}(values,at)",
                child.ordinal
            ),
            format!(
                "json_{name}_{namespace}_object_render_{}(values,at,cursor,next)",
                child.ordinal
            ),
        ),
        _ => (
            format!("let value=vec_get<{ty}>(values,at);"),
            length(root, kind, ordinal, "value", version),
            render(root, kind, ordinal, "value", version),
        ),
    };
    format!("@id(\"{id}.json.{family}.array-len.{ordinal}\") fn json_{name}_{namespace}_array_len_{ordinal}(values:borrow Vec<{ty}>)->usize {{
let count=vec_len<{ty}>(values);let mut valid=count<={bound}usize;let mut total=2usize;let mut at=0usize;
while valid && at<count {{{bindings}let size={size};let comma=if at>0usize{{1usize}}else{{0usize}};
let _ = if size=={INVALID} || total>{LIMIT}-comma || size>{LIMIT}-total-comma{{valid=false;false}}else{{total=total+size+comma;true}};
at=at+1usize;valid && at<count
}}if valid{{total}}else{{{INVALID}}}
}}
@id(\"{id}.json.{family}.array-render.{ordinal}\") fn json_{name}_{namespace}_array_render_{ordinal}(values:borrow Vec<{ty}>,position:usize,output:own Bytes)->Bytes
requires json_{name}_{namespace}_array_len_{ordinal}(values)!={INVALID} && position<=byte_len(bytes_as_slice(output)) && json_{name}_{namespace}_array_len_{ordinal}(values)<=byte_len(bytes_as_slice(output))-position {{
let mut next=output;let mut cursor=position;next=bytes_set(next,cursor,91u8);cursor=cursor+1usize;let count=vec_len<{ty}>(values);let mut at=0usize;
while at<count {{{bindings}let _ = if at>0usize{{next=bytes_set(next,cursor,44u8);cursor=cursor+1usize;true}}else{{true}};
next={rendered};cursor=cursor+({size});at=at+1usize;at<count
}}next=bytes_set(next,cursor,93u8);next
}}\n")
}
