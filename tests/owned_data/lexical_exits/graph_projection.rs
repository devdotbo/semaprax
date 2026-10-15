//! Owning source and agent projections preserve the same authenticated exits.
use super::ITERATOR;
use semaprax::{ast::LexicalExitKind, format, graph, hir};
use serde_json::{json, Value};

fn objects_of_kind<'a>(value: &'a Value, kind: &str, found: &mut Vec<&'a Value>) {
    match value {
        Value::Object(fields) => {
            if value["kind"] == kind {
                found.push(value);
            }
            for child in fields.values() {
                objects_of_kind(child, kind, found);
            }
        }
        Value::Array(items) => {
            for child in items {
                objects_of_kind(child, kind, found);
            }
        }
        _ => {}
    }
}

fn target(target: &hir::ControlTarget) -> Value {
    match target {
        hir::ControlTarget::Function(owner) => {
            json!({"kind":"function", "owner":owner.identity_key()})
        }
        hir::ControlTarget::Loop { owner, site } => {
            json!({"kind":"loop", "owner":owner.identity_key(), "site":site.as_str()})
        }
    }
}

#[test]
fn owning_iterator_lexical_graph_replays_exact_source_targets_and_resume_inventory() {
    let source = semaprax::check(ITERATOR, "owning-lexical-graph.spx").unwrap();
    let canonical = format::canonical(&source);
    assert!(canonical.contains("for own item in vec_into_iter<Bytes>(values)"));
    let roundtrip = semaprax::check(&canonical, "owning-lexical-graph.spx").unwrap();
    assert_eq!(format::canonical(&roundtrip), canonical);
    let resolved = hir::resolve(&roundtrip).unwrap();
    hir::validate(&resolved).unwrap();
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let control = function.cleanup_plan.lexical_control.as_deref().unwrap();
    let rendered = graph::to_json(&roundtrip).expect("admitted owning lexical graph projection");
    assert_eq!(rendered, graph::to_json(&source).unwrap());
    graph::verify_json(&roundtrip, &rendered).unwrap();
    let document: Value = serde_json::from_str(&rendered).unwrap();
    let node = document["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["kind"] == "function" && node["id"] == function.id.as_str())
        .unwrap();
    let projected = &node["cleanup"]["lexical_control"];
    assert_eq!(node["cleanup"]["schema"], function.cleanup_plan.schema);
    assert_eq!(projected["schema"], control.schema);
    assert_eq!(
        projected["loops"].as_array().unwrap().len(),
        control.loops.len()
    );
    let [loop_] = control.loops.as_slice() else {
        panic!("one authored owning loop");
    };
    let hir::LoopLatchKind::OwnedIteratorStep {
        next,
        assignment,
        step,
        remainder,
    } = &loop_.latch
    else {
        panic!("owning iterator advancement must retain its typed remainder");
    };
    let latch = json!({"kind":"owned_iterator_step", "next":next.as_str(), "assignment":assignment.as_str(), "step":step.as_str(), "remainder":remainder.as_str()});
    assert_eq!(projected["loops"][0]["target"], target(&loop_.target));
    assert_eq!(projected["loops"][0]["condition"], loop_.condition.as_str());
    assert_eq!(projected["loops"][0]["body"], loop_.body.as_str());
    assert_eq!(projected["loops"][0]["latch"], latch);
    let mut loops = Vec::new();
    objects_of_kind(&node["body"], "lexical_while", &mut loops);
    assert_eq!(loops.len(), 1);
    assert_eq!(loops[0]["latch"], latch);
    assert_eq!(loops[0]["condition"]["id"], loop_.condition.as_str());
    let mut matches = Vec::new();
    objects_of_kind(&node["body"], "match", &mut matches);
    let owning = matches
        .iter()
        .find(|node| node["id"] == assignment.as_str())
        .unwrap();
    assert_eq!(owning["ownership_mode"], "own");
    let fields = owning["arms"][1]["pattern"]["fields"].as_array().unwrap();
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[1]["binding"]["id"], remainder.as_str());
    assert!(fields
        .iter()
        .all(|field| field["binding"]["ownership_mode"] == "own"));
    let mut expressions = Vec::new();
    objects_of_kind(&node["body"], "lexical_exit", &mut expressions);
    assert_eq!(expressions.len(), control.exits.len());
    for (index, exit) in control.exits.iter().enumerate() {
        let kind = match exit.kind {
            LexicalExitKind::Break => "break",
            LexicalExitKind::Continue => "continue",
            LexicalExitKind::Return => "return",
        };
        let expression = expressions
            .iter()
            .find(|node| node["id"] == exit.site.as_str())
            .unwrap();
        assert_eq!(expression["exit_kind"], kind);
        assert_eq!(expression["target"], target(&exit.target));
        assert_eq!(
            expression["type_id"],
            hir::ResolvedType::Never.identity_key()
        );
        assert_eq!(
            projected["exits"][index],
            json!({"site":exit.site.as_str(), "target":target(&exit.target), "kind":kind, "exit":exit.exit.0, "resume":exit.resume.map(|id|id.0)})
        );
    }
    let expected = control.resumes.iter().map(|resume| json!({"entry":resume.entry.0, "blocks":resume.blocks.iter().map(|id|id.0).collect::<Vec<_>>()})).collect::<Vec<_>>();
    assert_eq!(projected["resumes"], json!(expected));
    // Reuse another real source exit identity; a stale agent projection never
    // gains authority merely because both identities exist in this module.
    let continued = control
        .exits
        .iter()
        .find(|exit| exit.kind == LexicalExitKind::Continue)
        .unwrap();
    let returned = control
        .exits
        .iter()
        .find(|exit| exit.kind == LexicalExitKind::Return)
        .unwrap();
    let needle = format!(
        "\"site\":{}",
        serde_json::to_string(continued.site.as_str()).unwrap()
    );
    assert_eq!(rendered.matches(&needle).count(), 1);
    let hostile = rendered.replace(
        &needle,
        &format!(
            "\"site\":{}",
            serde_json::to_string(returned.site.as_str()).unwrap()
        ),
    );
    assert!(graph::verify_json(&roundtrip, &hostile).is_err());
}
