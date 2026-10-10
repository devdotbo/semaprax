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
            program.functions.iter().map(move |function| {
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
