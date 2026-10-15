//! Declaration-order object length and rendering; no schema-name special cases.
use super::*;

fn value(record: &TypeDeclaration, field: &FieldDeclaration, indexed: bool) -> String {
    if indexed {
        format!(
            "vec_field<{}>(values,at,{})",
            record.name,
            literal(&field.name)
        )
    } else if field.ty == Type::String {
        format!("string_as_str(value.{})", field.name)
    } else {
        format!("value.{}", field.name)
    }
}

fn length(record: &TypeDeclaration, field: &FieldDeclaration, indexed: bool) -> String {
    let value = value(record, field, indexed);
    match field.ty {
        Type::String => format!("json_{}_response_utf8_quoted_len({value})", record.name),
        Type::Bool => format!("if {value}{{4usize}}else{{5usize}}"),
        Type::Usize => format!("jv_usize_len({value})"),
        Type::U8 => format!("usize_from_i64(jv_i64_len(i64_from_u8({value})))"),
        Type::I64 => format!("usize_from_i64(jv_i64_len({value}))"),
        _ => unreachable!("validated scalar/string"),
    }
}

pub(super) fn source(record: &TypeDeclaration, writer: &TypeDeclaration) -> String {
    object_source(record, writer, false)
}

pub(super) fn indexed_source(record: &TypeDeclaration, writer: &TypeDeclaration) -> String {
    object_source(record, writer, true)
}

fn object_source(record: &TypeDeclaration, writer: &TypeDeclaration, indexed: bool) -> String {
    let fs = fields(record);
    let name = &record.name;
    let id = &record.stable_id;
    // Copy metrics use Value parameters; only the owning Row needs a borrow.
    // StringAsStr authenticates this named borrow root and exact field path;
    // no synthetic String local, record match or clone grants view authority.
    let ownership = if fs.iter().any(|field| field.ty == Type::String) {
        "borrow "
    } else {
        ""
    };
    // Indexed helpers retain the exact borrowed Vec generation. The selector
    // is a compile-time field literal; String reads return a scoped Str directly.
    let (suffix, identity_suffix, parameters) = if indexed {
        ("_at", "-at", format!("values:borrow Vec<{name}>,at:usize"))
    } else {
        ("", "", format!("value:{ownership}{name}"))
    };
    let punctuation = 2 + fs.iter().map(|f| f.name.len() + 3).sum::<usize>() + fs.len() - 1;
    let string_fields: Vec<_> = fs.iter().filter(|field| field.ty == Type::String).collect();
    let (validity, validity_bindings) = match string_fields.as_slice() {
        [] => ("true".to_owned(), String::new()),
        [field] => (
            format!(
                "json_{name}_response_owned_valid({})",
                value(record, field, indexed)
            ),
            String::new(),
        ),
        _ => (
            (0..string_fields.len())
                .map(|index| format!("response_string_valid_{index}"))
                .collect::<Vec<_>>()
                .join("&&"),
            string_fields
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    format!(
                        "let response_string_valid_{index}=json_{name}_response_owned_valid({});",
                        value(record, field, indexed)
                    )
                })
                .collect::<String>(),
        ),
    };
    let mut out = format!(
        "@id(\"{id}.json.collection-response.object-len{identity_suffix}\")
fn json_{name}_response_object_len{suffix}({parameters})->usize {{
{validity_bindings}if !({validity}){{18446744073709551615usize}}else{{{punctuation}usize"
    );
    for field in fs {
        write!(out, "+({})", length(record, field, indexed)).unwrap();
    }
    let arguments = if indexed { "values,at" } else { "value" };
    writeln!(out, "}}}}\n@id(\"{id}.json.collection-response.object-render{identity_suffix}\")
fn json_{name}_response_object_render{suffix}({parameters},position:usize,output:own Bytes)->Bytes
requires json_{name}_response_object_len{suffix}({arguments})!=18446744073709551615usize && position<=byte_len(bytes_as_slice(output)) && json_{name}_response_object_len{suffix}({arguments})<=byte_len(bytes_as_slice(output))-position {{
let mut next=output;let mut cursor=position;").unwrap();
    let prefix = format!("json_{}_response", writer.name);
    out.push_str(&writer::literal_write(&prefix, "next", "cursor", "{", 0));
    for (index, field) in fs.iter().enumerate() {
        out.push_str(&writer::literal_write(
            &prefix,
            "next",
            "cursor",
            &format!("{}\"{}\":", if index == 0 { "" } else { "," }, field.name),
            index,
        ));
        let value = value(record, field, indexed);
        let kind = match field.ty {
            Type::String => "quote",
            Type::Bool => "bool",
            Type::Usize => "usize",
            Type::U8 | Type::I64 => "i64",
            _ => unreachable!("validated scalar"),
        };
        let operand = if field.ty == Type::U8 {
            format!("i64_from_u8({value})")
        } else {
            value
        };
        writeln!(
            out,
            "next=json_{}_response_writer_{kind}({operand},cursor,next);cursor=cursor+({});",
            writer.name,
            length(record, field, indexed)
        )
        .unwrap();
    }
    out.push_str(&writer::literal_write(
        &prefix,
        "next",
        "cursor",
        "}",
        fs.len(),
    ));
    out.push_str("next}\n");
    out
}
