//! Explicit generation-stage projection. Never a runtime reachability filter.
use super::*;
use crate::ast::{Expr, ExprKind, MatchPattern, RecordMatchFieldPattern, Statement};
use std::collections::{BTreeMap, BTreeSet};

struct Inventory {
    functions: BTreeMap<String, String>,
    retained_functions: BTreeMap<String, String>,
    retained_types: BTreeMap<String, String>,
    variant_id: String,
    variant_name: String,
    imports: BTreeSet<&'static str>,
    all_imports: BTreeMap<String, String>,
}
fn inventory(root: &TypeDeclaration, row: &TypeDeclaration, utf8: bool, stream: bool) -> Inventory {
    let mut functions = BTreeMap::new();
    let mut add = |owner: &TypeDeclaration, suffix: &str, name: &str| {
        functions.insert(
            format!("{}.json.{suffix}", owner.stable_id),
            format!("json_{}_{name}", owner.name),
        );
    };
    if utf8 {
        for (suffix, name) in [
            ("utf8.owned-valid", "owned_valid"),
            ("utf8.quoted-len", "utf8_quoted_len"),
            ("utf8.quoted-render", "utf8_quote"),
            ("utf8.owned-row-len", "owned_row_len"),
            ("utf8.owned-row-render", "owned_row_render"),
        ] {
            add(row, suffix, name);
        }
        add(root, "utf8.owned.encoded-len", "utf8_owned_encoded_len");
        add(root, "utf8.owned.encode", "utf8_owned_encode");
    } else {
        for (suffix, name) in [
            ("identifier-render", "identifier_render"),
            ("view.encoded-len", "view_encoded_len"),
            ("view.encode", "view_encode"),
            ("view.render", "view_render"),
            ("array.encoded-len", "view_array_encoded_len"),
            ("array.encode", "view_array_encode"),
            ("owned-valid", "owned_valid"),
            ("owned-row-len", "owned_row_len"),
            ("owned-row-render", "owned_row_render"),
        ] {
            add(row, suffix, name);
        }
        add(root, "request.encode", "request_encode");
        add(root, "owned.encoded-len", "owned_encoded_len");
        add(root, "owned.encode", "owned_encode");
    }
    let mut retained_functions = BTreeMap::new();
    for (owner, suffix, name) in if utf8 {
        vec![
            (row, "utf8.valid", "identifier_valid"),
            (row, "utf8.materialize-text", "utf8_text"),
            (row, "view.decode", "view_decode"),
            (row, "utf8.owned-materialize", "utf8_materialize"),
            (root, "request.decode", "request_decode"),
            (root, "utf8.owned.decode", "utf8_owned_decode"),
        ]
    } else {
        vec![
            (row, "identifier-valid", "identifier_valid"),
            (row, "view.decode", "view_decode"),
            (row, "array.decode", "view_array_decode"),
            (row, "identifier-fingerprint", "identifier_fingerprint"),
            (row, "owned-identifier", "owned_identifier"),
            (row, "owned-materialize", "owned_materialize"),
            (root, "request.decode", "request_decode"),
            (root, "owned.decode", "owned_decode"),
        ]
    } {
        retained_functions.insert(
            format!("{}.json.{suffix}", owner.stable_id),
            format!("json_{}_{name}", owner.name),
        );
    }
    let mut retained_types = BTreeMap::new();
    for (owner, suffix, name) in [
        (row, "view", "JsonView"),
        (row, "view-decode", "JsonViewDecode"),
        (root, "identifier-span", "JsonIdentifierSpan"),
        (root, "request-result", "JsonRequestDecode"),
        (
            root,
            if utf8 {
                "utf8.owned-result"
            } else {
                "owned-result"
            },
            if utf8 {
                "JsonUtf8OwnedDecode"
            } else {
                "JsonOwnedDecode"
            },
        ),
    ] {
        retained_types.insert(
            format!("{}.json.{suffix}", owner.stable_id),
            format!("{}{name}", owner.name),
        );
    }
    if !utf8 {
        retained_types.insert(
            format!("{}.json.array-result", row.stable_id),
            format!("{}JsonArrayDecode", row.name),
        );
    }
    if stream {
        retained_functions.insert(
            format!("{}.json.stream.normalize", root.stable_id),
            format!("json_{}_stream_normalize", root.name),
        );
        retained_types.insert(
            format!("{}.json.stream-input", root.stable_id),
            format!("{}JsonStreamInput", root.name),
        );
    }
    let mut all_imports = BTreeMap::new();
    for (module, name) in [
        ("scan", "strict_end"),
        ("scan", "root"),
        ("scan", "kind"),
        ("scan", "first_member"),
        ("scan", "member_value"),
        ("scan", "next_member"),
        ("scan", "key_eq"),
        ("scan", "decimal_end"),
        ("scan", "value_end"),
        ("scan", "first_element"),
        ("scan", "next_element"),
        ("token", "integer_end"),
        ("token", "i64_or"),
        ("digits", "i64_len"),
        ("write", "usize_len"),
        ("query", "scan_string"),
        ("query", "decoded_len"),
        ("query", "emit_at"),
        ("query", "token_end"),
        ("query", "decoded_token_eq"),
    ] {
        all_imports.insert(
            format!("std.data.json.{module}.{name}"),
            format!("jv_{name}"),
        );
    }
    if utf8 {
        for (module, name, alias) in [
            ("query", "scalar_at", "ju_escape_scalar"),
            ("utf8", "scalar_at", "ju_raw_scalar"),
            ("utf8", "sequence_end", "ju_raw_end"),
            ("scan", "utf8_text_valid", "ju_text_valid"),
            ("utf8", "bounded_valid", "ju_bounded_valid"),
            ("utf8", "quoted_len", "ju_quoted_len"),
        ] {
            all_imports.insert(format!("std.data.json.{module}.{name}"), alias.into());
        }
    }
    let mut imports = BTreeSet::from([
        "std.data.json.digits.i64_len",
        "std.data.json.write.usize_len",
    ]);
    if utf8 {
        imports.extend([
            "std.data.json.utf8.bounded_valid",
            "std.data.json.utf8.quoted_len",
        ]);
    }
    Inventory {
        functions,
        retained_functions,
        retained_types,
        variant_id: format!("{}.json.view-encode", row.stable_id),
        variant_name: format!("{}JsonViewEncode", row.name),
        imports,
        all_imports,
    }
}
fn mismatch() -> Vec<Diagnostic> {
    refusal("decode-only codec generation inventory or retained dependency closure differs from its closed profile")
}
pub(super) fn full_profile(profile: JsonCodecProfile) -> Option<JsonCodecProfile> {
    match profile {
        JsonCodecProfile::OwnedRequestDecodeOnlyV1 => Some(JsonCodecProfile::OwnedRequest),
        JsonCodecProfile::StreamOwnedRequestDecodeOnlyV1 => {
            Some(JsonCodecProfile::StreamOwnedRequest)
        }
        JsonCodecProfile::Utf8OwnedRequestDecodeOnlyV1 { max_string_bytes } => {
            Some(JsonCodecProfile::Utf8OwnedRequest { max_string_bytes })
        }
        JsonCodecProfile::StreamUtf8OwnedRequestDecodeOnlyV1 { max_string_bytes } => {
            Some(JsonCodecProfile::StreamUtf8OwnedRequest { max_string_bytes })
        }
        _ => None,
    }
}
pub(super) fn source(
    program: &crate::ast::Program,
    root: &TypeDeclaration,
    full: JsonCodecProfile,
    authored: &str,
) -> Result<String, Vec<Diagnostic>> {
    let utf8 = match full {
        JsonCodecProfile::OwnedRequest | JsonCodecProfile::StreamOwnedRequest => false,
        JsonCodecProfile::Utf8OwnedRequest { .. }
        | JsonCodecProfile::StreamUtf8OwnedRequest { .. } => true,
        _ => return Err(mismatch()),
    };
    // Existing full generator predicates run first, including stream permit and
    // bound/shape order. Its bytes and old public profiles remain untouched.
    let raw = super::profile_fragment(program, root, full)?;
    if raw.len() > MAX_GENERATED_BYTES {
        return Err(refusal(
            "JSON codec generated source exceeds its fixed 131072-byte bound",
        ));
    }
    // Fresh discard names are assigned over the COMPLETE old fragment before
    // projection, with exact authored identifiers. Removing earlier encoders
    // must not renumber any later retained decoder binding.
    let raw =
        super::template::discard_bindings(&raw, authored, &program.path).map_err(|e| vec![e])?;
    let TypeDeclarationKind::Record { fields } = &root.kind else {
        return Err(mismatch());
    };
    let Type::Named { arguments, .. } = &fields[1].ty else {
        return Err(mismatch());
    };
    let Type::Named { name, .. } = &arguments[0] else {
        return Err(mismatch());
    };
    let row = program
        .types
        .iter()
        .find(|t| &t.name == name)
        .ok_or_else(mismatch)?;
    let stream = matches!(
        full,
        JsonCodecProfile::StreamOwnedRequest | JsonCodecProfile::StreamUtf8OwnedRequest { .. }
    );
    project(&raw, &inventory(root, row, utf8, stream))
}
fn project(raw: &str, expected: &Inventory) -> Result<String, Vec<Diagnostic>> {
    if raw.len() > MAX_GENERATED_BYTES {
        return Err(refusal(
            "JSON codec generated source exceeds its fixed 131072-byte bound",
        ));
    }
    let mut ast = crate::parse(raw, "decode-only-fragment.spx").map_err(|e| vec![e])?;
    if !ast.interfaces.is_empty()
        || !ast.protocols.is_empty()
        || !ast.implementations.is_empty()
        || !ast.session_protocols.is_empty()
        || !ast.agents.is_empty()
    {
        return Err(mismatch());
    }
    if ast.module_uses.len() != expected.all_imports.len()
        || ast.module_uses.iter().any(|u| {
            expected.all_imports.get(&u.persistent_id) != Some(&u.alias)
                || u.target_module != u.persistent_id.rsplit_once('.').unwrap().0
                || u.kind != crate::ast::ModuleUseKind::Function
        })
        || ast.functions.len() != expected.functions.len() + expected.retained_functions.len()
        || ast.types.len() != 1 + expected.retained_types.len()
        || expected.retained_functions.iter().any(|(id, name)| {
            ast.functions
                .iter()
                .filter(|f| &f.stable_id == id && &f.name == name && f.explicit_id)
                .count()
                != 1
        })
        || expected.retained_types.iter().any(|(id, name)| {
            ast.types
                .iter()
                .filter(|t| &t.stable_id == id && &t.name == name && t.explicit_id)
                .count()
                != 1
        })
    {
        return Err(mismatch());
    }
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for (id, name) in ast
        .functions
        .iter()
        .map(|f| (&f.stable_id, &f.name))
        .chain(ast.types.iter().map(|t| (&t.stable_id, &t.name)))
    {
        if !ids.insert(id) || !names.insert(name) {
            return Err(mismatch());
        }
    }
    if expected.functions.iter().any(|(id, name)| {
        ast.functions
            .iter()
            .filter(|f| &f.stable_id == id && &f.name == name && f.explicit_id)
            .count()
            != 1
    }) || ast
        .types
        .iter()
        .filter(|t| {
            t.stable_id == expected.variant_id && t.name == expected.variant_name && t.explicit_id
        })
        .count()
        != 1
        || expected.imports.iter().any(|id| {
            ast.module_uses
                .iter()
                .filter(|u| {
                    u.persistent_id == *id
                        && u.alias == import_alias(id)
                        && u.target_module == id.rsplit_once('.').unwrap().0
                        && u.kind == crate::ast::ModuleUseKind::Function
                })
                .count()
                != 1
        })
    {
        return Err(mismatch());
    }
    let variant = ast
        .types
        .iter()
        .find(|t| t.stable_id == expected.variant_id)
        .unwrap();
    let prefix = expected
        .variant_id
        .strip_suffix("view-encode")
        .ok_or_else(mismatch)?;
    if !matches!(&variant.kind, TypeDeclarationKind::Variant { cases } if cases.len() == 2
        && cases[0].name == "Encoded" && cases[0].explicit_id && cases[0].stable_id == format!("{prefix}view-encoded")
        && cases[0].fields.len() == 1 && cases[0].fields[0].name == "text" && cases[0].fields[0].explicit_id
        && cases[0].fields[0].stable_id == format!("{prefix}view-text") && cases[0].fields[0].ty == Type::String
        && cases[1].name == "Refused" && cases[1].explicit_id && cases[1].stable_id == format!("{prefix}view-refused")
        && cases[1].fields.len() == 1 && cases[1].fields[0].name == "required" && cases[1].fields[0].explicit_id
        && cases[1].fields[0].stable_id == format!("{prefix}view-required") && cases[1].fields[0].ty == Type::Usize)
    {
        return Err(mismatch());
    }
    let mut import_ids = BTreeSet::new();
    let mut import_names = BTreeSet::new();
    if ast
        .module_uses
        .iter()
        .any(|u| !import_ids.insert(&u.persistent_id) || !import_names.insert(&u.alias))
    {
        return Err(mismatch());
    }
    let forbidden = expected
        .functions
        .values()
        .map(String::as_str)
        .chain(
            ast.module_uses
                .iter()
                .filter(|u| expected.imports.contains(u.persistent_id.as_str()))
                .map(|u| u.alias.as_str()),
        )
        .collect::<BTreeSet<_>>();
    // Examine every retained node BEFORE removing anything. No reference may
    // acquire a missing encoder, encode-only import or result type by omission.
    let mut closed = true;
    for function in ast
        .functions
        .iter()
        .filter(|f| !expected.functions.contains_key(&f.stable_id))
    {
        closed &= function.type_parameters.is_empty()
            && function.yields.is_none()
            && function.follows.is_none()
            && !uses_type(&function.return_type, &expected.variant_name)
            && function
                .params
                .iter()
                .all(|p| !uses_type(&p.ty, &expected.variant_name));
        for expr in function
            .requires
            .iter()
            .chain(&function.ensures)
            .chain(std::iter::once(&function.body))
        {
            closed &= expression_closed(expr, &forbidden, &expected.variant_name);
        }
    }
    for ty in ast
        .types
        .iter()
        .filter(|t| t.stable_id != expected.variant_id)
    {
        closed &= ty.extends.is_none() && ty.type_parameters.is_empty();
        closed &= match &ty.kind {
            TypeDeclarationKind::Record { fields } => fields
                .iter()
                .all(|f| !uses_type(&f.ty, &expected.variant_name)),
            TypeDeclarationKind::Variant { cases } => cases.iter().all(|c| {
                c.fields
                    .iter()
                    .all(|f| !uses_type(&f.ty, &expected.variant_name))
            }),
            _ => false,
        };
        closed &= ty
            .invariants()
            .iter()
            .all(|e| expression_closed(e, &forbidden, &expected.variant_name));
    }
    if !closed {
        return Err(mismatch());
    }
    drop(forbidden);
    drop(import_ids);
    drop(import_names);
    drop(ids);
    drop(names);
    ast.functions
        .retain(|f| !expected.functions.contains_key(&f.stable_id));
    ast.types.retain(|t| t.stable_id != expected.variant_id);
    ast.module_uses
        .retain(|u| !expected.imports.contains(u.persistent_id.as_str()));
    Ok(crate::format::canonical(&ast))
}
fn uses_type(ty: &Type, forbidden: &str) -> bool {
    match ty {
        Type::Named { name, arguments } => {
            name == forbidden || arguments.iter().any(|t| uses_type(t, forbidden))
        }
        Type::Function { parameters, result } => {
            parameters.iter().any(|t| uses_type(t, forbidden)) || uses_type(result, forbidden)
        }
        _ => false,
    }
}
fn import_alias(id: &str) -> &'static str {
    match id {
        "std.data.json.digits.i64_len" => "jv_i64_len",
        "std.data.json.write.usize_len" => "jv_usize_len",
        "std.data.json.utf8.bounded_valid" => "ju_bounded_valid",
        "std.data.json.utf8.quoted_len" => "ju_quoted_len",
        _ => unreachable!("closed inventory"),
    }
}
fn pattern_closed(pattern: &MatchPattern, variant: &str) -> bool {
    match pattern {
        MatchPattern::Variant { type_name, .. } => type_name != variant,
        MatchPattern::Record {
            type_name, fields, ..
        } => {
            type_name != variant
                && fields
                    .iter()
                    .all(|f| record_pattern_closed(&f.pattern, variant))
        }
        MatchPattern::Or { alternatives, .. } => {
            alternatives.iter().all(|p| pattern_closed(p, variant))
        }
        MatchPattern::Wildcard { .. }
        | MatchPattern::Literal { .. }
        | MatchPattern::Binding { .. } => true,
    }
}
fn record_pattern_closed(pattern: &RecordMatchFieldPattern, variant: &str) -> bool {
    match pattern {
        RecordMatchFieldPattern::Record {
            type_name, fields, ..
        } => {
            type_name != variant
                && fields
                    .iter()
                    .all(|f| record_pattern_closed(&f.pattern, variant))
        }
        RecordMatchFieldPattern::Binding { .. } | RecordMatchFieldPattern::Wildcard { .. } => true,
    }
}
fn expression_closed(expr: &Expr, forbidden: &BTreeSet<&str>, variant: &str) -> bool {
    let mut closed = true;
    expr.visit_all_nodes(&mut |node| match &node.kind {
        ExprKind::Call {
            name,
            type_arguments,
            ..
        } => {
            closed &= !forbidden.contains(name.as_str())
                && type_arguments.iter().all(|t| !uses_type(t, variant))
        }
        ExprKind::Var(name) => closed &= !forbidden.contains(name.as_str()),
        ExprKind::ConstructRecord {
            type_name,
            type_arguments,
            ..
        }
        | ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            ..
        } => {
            closed &= type_name != variant && type_arguments.iter().all(|t| !uses_type(t, variant))
        }
        ExprKind::Match { arms, .. } => {
            closed &= arms.iter().all(|a| pattern_closed(&a.pattern, variant))
        }
        ExprKind::MethodCall { type_arguments, .. } => {
            closed &= type_arguments.iter().all(|t| !uses_type(t, variant))
        }
        // SuperMethod has no type arguments. Its actual args are visited by
        // visit_all_nodes, including every nested call/type/dependency check.
        ExprKind::SuperMethod { .. } => {}
        ExprKind::Closure {
            params,
            return_type,
            ..
        } => {
            closed &= !uses_type(return_type, variant)
                && params.iter().all(|p| !uses_type(&p.ty, variant))
        }
        ExprKind::VecConstructor { element, .. } => closed &= !uses_type(element, variant),
        ExprKind::Block { statements, .. } => {
            closed &= statements.iter().all(|s| match s {
                Statement::Let { declared, .. } => {
                    declared.as_ref().is_none_or(|t| !uses_type(t, variant))
                }
                Statement::Assign { .. }
                | Statement::Unsafe { .. }
                | Statement::While { .. }
                | Statement::For { .. }
                | Statement::ForBorrow { .. }
                | Statement::ForOwn { .. } => true,
            })
        }
        ExprKind::LexicalExit { .. }
        | ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. }
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::BorrowedUtf8Literal(_)
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. }
        | ExprKind::If { .. }
        | ExprKind::Try { .. }
        | ExprKind::UpdateRecord { .. }
        | ExprKind::Project { .. }
        | ExprKind::Yield { .. } => {}
    });
    closed
}
#[cfg(test)]
mod tests;
