use super::{provider, ProviderProofs};
use crate::ast::Program;
use crate::workspace_graph::{AuthoredDeclaration, AuthoredKind};
use std::collections::BTreeMap;
fn programs() -> Vec<Program> {
    [
        (
            "leaf.spx",
            r#"module leaf;
@id("leaf.fill") fn fill(output:own Bytes,index:usize,value:u8)->Bytes {
 let mut next=output; next=bytes_set(next,index,value); next
}"#,
        ),
        (
            "relay.spx",
            r#"module relay;
use function @id("leaf.fill") from leaf as fill;
@id("relay.fill") fn relay(output:own Bytes,index:usize,value:u8)->Bytes {
 fill(output,index,value)
}"#,
        ),
    ]
    .into_iter()
    .map(|(path, source)| crate::parse(source, path).unwrap())
    .collect()
}
fn facts(programs: &[Program]) -> BTreeMap<&str, AuthoredDeclaration<'_>> {
    programs
        .iter()
        .flat_map(|program| {
            program
                .functions
                .iter()
                .map(move |function| {
                    (
                        function.stable_id.as_str(),
                        AuthoredDeclaration {
                            path: &program.path,
                            module: &program.module,
                            explicit: function.explicit_id,
                            kind: AuthoredKind::Function,
                            function: Some(function),
                            ty: None,
                        },
                    )
                })
                .chain(program.types.iter().map(move |declaration| {
                    (
                        declaration.stable_id.as_str(),
                        AuthoredDeclaration {
                            path: &program.path,
                            module: &program.module,
                            explicit: declaration.explicit_id,
                            kind: AuthoredKind::Type,
                            function: None,
                            ty: Some(declaration),
                        },
                    )
                }))
        })
        .collect()
}
#[test]
fn provider_phase_reuses_actual_transitive_proof_and_rechecks_origin_before_lookup() {
    let programs = programs();
    let authored = facts(&programs);
    let target = &programs[1].functions[0];
    let mut proofs = ProviderProofs::new(&authored, &programs);
    assert!(proofs.authorized(target, "relay").unwrap());
    let (repeated, overflowed, used) =
        crate::bounded_output::with_limit_usage(0, || proofs.authorized(target, "relay"));
    assert!(repeated.unwrap());
    assert!(!overflowed);
    assert_eq!(used, 0);
    assert!(
        !proofs.authorized(target, "leaf").unwrap(),
        "a cached body cannot authenticate a different provider"
    );
    let copied = target.clone();
    assert!(provider(&copied, "relay", &authored, &programs).is_none());
    drop(proofs);
    let mut stale = facts(&programs);
    stale.get_mut("leaf.fill").unwrap().module = "forged";
    assert!(!ProviderProofs::new(&stale, &programs)
        .authorized(target, "relay")
        .unwrap());
    let mut changed = programs.clone();
    changed[0].functions[0].body = crate::parse(
        "module changed; @id(\"body\") fn body()->Bytes { bytes_zeroed(1usize) }",
        "changed.spx",
    )
    .unwrap()
    .functions
    .remove(0)
    .body;
    let changed_facts = facts(&changed);
    assert!(
        !ProviderProofs::new(&changed_facts, &changed)
            .authorized(&changed[1].functions[0], "relay")
            .unwrap(),
        "new source must replay the actual allocating provider"
    );
}
#[test]
fn provider_phase_exact_and_one_short_reservations_refuse_without_fallback() {
    let programs = programs();
    let authored = facts(&programs);
    let target = &programs[1].functions[0];
    let (first, overflowed, required) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        ProviderProofs::new(&authored, &programs).authorized(target, "relay")
    });
    assert!(first.unwrap());
    assert!(!overflowed);
    assert!(required > 0);
    let (exact, overflowed, used) = crate::bounded_output::with_limit_usage(required, || {
        ProviderProofs::new(&authored, &programs).authorized(target, "relay")
    });
    assert!(exact.unwrap());
    assert!(!overflowed);
    assert_eq!(used, required);
    let mut short = ProviderProofs::new(&authored, &programs);
    let (failure, overflowed, _) =
        crate::bounded_output::with_limit_usage(required - 1, || short.authorized(target, "relay"));
    assert_eq!(failure.unwrap_err().code, "SPX-G171");
    assert!(overflowed);
    assert_eq!(
        short.authorized(target, "relay").unwrap_err().code,
        "SPX-G171"
    );
}

fn vector_programs() -> Vec<Program> {
    [
        (
            "leaf.spx",
            r#"module leaf;
@id("row") record Row {@id("row.label") label:string,@id("row.index") index:usize,}
@id("leaf.fill") fn fill(output:own Bytes,rows:borrow Vec<Row>)->Bytes {
 let mut next=output;next=bytes_set(next,vec_len<Row>(rows),65u8);next
}"#,
        ),
        (
            "relay.spx",
            r#"module relay;
use type @id("row") from leaf as Renamed;
use function @id("leaf.fill") from leaf as fill;
@id("relay.fill") fn relay(output:own Bytes,rows:borrow Vec<Renamed>)->Bytes {
 fill(output,rows)
}"#,
        ),
    ]
    .into_iter()
    .map(|(path, text)| crate::parse(text, path).unwrap())
    .collect()
}
#[test]
fn imported_alias_vector_uses_current_declared_layout_and_actual_provider_body() {
    let programs = vector_programs();
    let authored = facts(&programs);
    let target = &programs[1].functions[0];
    assert!(!crate::loop_calls::ast_param_admitted(
        &programs[1],
        target.params[1].mode,
        &target.params[1].ty
    ));
    assert!(ProviderProofs::new(&authored, &programs)
        .authorized(target, "relay")
        .unwrap());
    let mut allocating = programs.clone();
    allocating[0].functions[0].body = crate::parse(
        r#"module allocating;@id("allocating") fn allocate()->Bytes {bytes_zeroed(1usize)}"#,
        "allocating.spx",
    )
    .unwrap()
    .functions
    .remove(0)
    .body;
    let changed = facts(&allocating);
    assert!(!ProviderProofs::new(&changed, &allocating)
        .authorized(&allocating[1].functions[0], "relay")
        .unwrap());
}
#[test]
fn imported_vector_refuses_forged_identity_layout_mode_and_builtin_shadow() {
    let original = vector_programs();
    for mutation in 0..7 {
        let mut programs = original.clone();
        match mutation {
            0 => programs[1].module_uses[0].target_module = "forged".to_owned(),
            1 => programs[1].module_uses[0].persistent_id = "different.row".to_owned(),
            2 => programs[1].functions[0].params[1].mode = crate::ast::ParamMode::Own,
            3 => {
                let crate::ast::TypeDeclarationKind::Record { fields } =
                    &mut programs[0].types[0].kind
                else {
                    panic!("record")
                };
                fields[0].explicit_id = false;
            }
            4 => {
                let crate::ast::TypeDeclarationKind::Record { fields } =
                    &mut programs[0].types[0].kind
                else {
                    panic!("record")
                };
                fields[0].ty = crate::ast::Type::Named {
                    name: "Vec".to_owned(),
                    arguments: vec![crate::ast::Type::U8],
                };
            }
            5 => {
                let mut shadow = programs[1].module_uses[0].clone();
                shadow.alias = "Vec".to_owned();
                programs[1].module_uses.push(shadow);
            }
            6 => {
                let crate::ast::Type::Named { arguments, .. } =
                    &mut programs[1].functions[0].params[1].ty
                else {
                    panic!("Vec")
                };
                arguments.push(crate::ast::Type::Usize);
            }
            _ => unreachable!(),
        }
        let authored = facts(&programs);
        assert!(
            !ProviderProofs::new(&authored, &programs)
                .authorized(&programs[1].functions[0], "relay")
                .unwrap(),
            "mutation {mutation}"
        );
    }
    let programs = vector_programs();
    let mut authored = facts(&programs);
    // An authored lookup key cannot relabel another current declaration.
    authored.get_mut("row").unwrap().ty = None;
    assert!(!ProviderProofs::new(&authored, &programs)
        .authorized(&programs[1].functions[0], "relay")
        .unwrap());
}
#[test]
fn imported_vector_positive_proof_retains_exact_and_one_short_reservation() {
    let programs = vector_programs();
    let authored = facts(&programs);
    let target = &programs[1].functions[0];
    let (result, overflowed, required) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            ProviderProofs::new(&authored, &programs).authorized(target, "relay")
        });
    assert!(result.unwrap());
    assert!(!overflowed && required > 0);
    let (result, overflowed, used) = crate::bounded_output::with_limit_usage(required, || {
        ProviderProofs::new(&authored, &programs).authorized(target, "relay")
    });
    assert!(result.unwrap());
    assert!(!overflowed);
    assert_eq!(used, required);
    let (result, overflowed, _) = crate::bounded_output::with_limit_usage(required - 1, || {
        ProviderProofs::new(&authored, &programs).authorized(target, "relay")
    });
    assert_eq!(result.unwrap_err().code, "SPX-G171");
    assert!(overflowed);
}

#[test]
fn imported_record_lookup_key_cannot_relabel_a_different_current_nominal() {
    let mut programs = vector_programs();
    let other=crate::parse(r#"module leaf;
@id("other.row") record Other {@id("other.row.label") label:string,@id("other.row.index") index:usize,}
@id("other.anchor") fn anchor()->i64 {0}
"#,"leaf-other.spx").unwrap().types.remove(0);
    programs[0].types.push(other);
    let mut authored = facts(&programs);
    authored.get_mut("row").unwrap().ty = Some(&programs[0].types[1]);
    assert!(!ProviderProofs::new(&authored, &programs)
        .authorized(&programs[1].functions[0], "relay")
        .unwrap());
}
