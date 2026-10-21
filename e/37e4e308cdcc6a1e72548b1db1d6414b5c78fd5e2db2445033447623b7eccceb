//! Pure canonical-row references for the same one bounded metadata page.
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
pub(super) const SCHEMA: &str = "semaprax.finite-source-overlay-page.v3";
pub(super) const SCHEMA_V4: &str = "semaprax.finite-source-overlay-page.v4";
pub(super) const SCHEMA_V5: &str = "semaprax.finite-source-overlay-page.v5";
const EXTRA519: &[(&str, &str)] = &[
    (
        "src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs",
        "e9f1208c4daeeb93e1200a127c9dee49e57c3ee370c10c5c0305c8c4266d1443",
    ),
    (
        "src/cleanup_plan/replay.rs",
        "85efc15e0205f0f2c4070c9d80b6f30b50f73bc6e4070854484bdd75e8fb9a8a",
    ),
];
const OLD519: &[(&str, &str)] = &[
    (
        "src/source_reflows_merge.rs",
        "3ef2bb440f23c573007567c98b4bfc608b555bd2cc8f2591d365476b850bd46a",
    ),
    (
        "src/source_reflows/incoming67107_semantic_receivers.rs",
        "383eff4edb39826861991d6dcd84dc0c489471e90e66ed214b4d5ad46a907ac1",
    ),
];
const REFERENCE: &str = "canonical_source";
const ROOTS: &[&str] = &[
    "tests/hir_module_boundaries.rs",
    "tests/workspace/semantic_graph/source_inventory.rs",
    "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
];

// No path is opened, normalized, followed or granted here. All targets already
// are complete canonical rows in the predecessor or this same literal page.
pub(super) fn expand(before: &Value, policy: &Value, page: &Value) -> Result<Value, &'static str> {
    if page["schema"] != SCHEMA {
        return Err("v3 schema");
    }
    let old = before["rows"].as_array().ok_or("v3 old rows")?;
    let mut canonical = BTreeMap::new();
    for row in old {
        let source = row["source"].as_str().ok_or("v3 old source")?;
        if canonical.insert(source, row).is_some() {
            return Err("v3 duplicate old source");
        }
    }
    let mut overlay = BTreeSet::new();
    for group in ["replacements", "additions"] {
        for row in page[group].as_array().ok_or("v3 rows")? {
            let source = row["source"].as_str().ok_or("v3 source")?;
            if !overlay.insert(source) {
                return Err("v3 duplicate canonical source");
            }
            canonical.insert(source, row);
        }
    }
    let mut allowed: BTreeSet<_> = super::edges(before)?.into_iter().collect();
    for edge in policy["new_fragment_edges"].as_array().ok_or("v3 edges")? {
        if edge.as_object().ok_or("v3 edge")?.len() != 2 {
            return Err("v3 edge fields");
        }
        allowed.insert((
            edge["parent"].as_str().ok_or("v3 parent")?.to_owned(),
            edge["child"].as_str().ok_or("v3 child")?.to_owned(),
        ));
    }
    let mut expanded = page.clone();
    expanded["schema"] = Value::String("semaprax.finite-source-overlay-page.v2".to_owned());
    for group in ["replacements", "additions"] {
        for row in expanded[group].as_array_mut().ok_or("v3 rows")? {
            let parent = row["source"].as_str().ok_or("v3 parent")?.to_owned();
            let Some(fragments) = row.get_mut("fragments") else {
                continue;
            };
            let mut seen = BTreeSet::new();
            for fragment in fragments.as_array_mut().ok_or("v3 fragments")? {
                if fragment.get(REFERENCE).is_none() {
                    continue;
                }
                if !ROOTS.contains(&parent.as_str()) {
                    return Err("v3 nonliteral root");
                }
                if fragment.as_object().ok_or("v3 reference")?.len() != 1 {
                    return Err("v3 reference fields");
                }
                let child = fragment[REFERENCE].as_str().ok_or("v3 reference source")?;
                let edge = (parent.clone(), child.to_owned());
                if parent == child || !allowed.contains(&edge) || !seen.insert(edge) {
                    return Err("v3 undeclared or duplicate reference");
                }
                let target = *canonical.get(child).ok_or("v3 missing canonical row")?;
                if target["compiled"] != true || target.get("fragments").is_some() {
                    return Err("v3 noncompiled or nested reference");
                }
                *fragment = target.clone();
            }
        }
    }
    Ok(expanded)
}

// Only these two already authenticated old519 records can be inherited. The
// tuple is [literal index, current length, current SHA256, additional views].
// Its current archive is exactly the existing e/<full SHA256> image channel.
// No path, file, permission, parsed authority or accepted result is acquired.
pub(super) fn expand_v4(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    if page["schema"] != SCHEMA_V4 {
        return Err("v4 schema");
    }
    let old = before["rows"].as_array().ok_or("v4 old rows")?;
    if old.len() != 519 {
        return Err("v4 whole old519");
    }
    let mut expanded = page.clone();
    expanded["schema"] = SCHEMA.into();
    let mut seen = BTreeSet::new();
    for row in expanded["replacements"]
        .as_array_mut()
        .ok_or("v4 replacements")?
    {
        let Some(reference) = row.get("old519") else {
            if OLD519.iter().any(|(source, _)| row["source"] == *source) {
                return Err("v4 missing literal inheritance");
            }
            continue;
        };
        if row.as_object().ok_or("v4 record")?.len() != 1 {
            return Err("v4 reference fields");
        }
        let tuple = reference.as_array().ok_or("v4 tuple")?;
        if tuple.len() != 4 {
            return Err("v4 tuple fields");
        }
        let index = usize::try_from(tuple[0].as_u64().ok_or("v4 literal index")?)
            .map_err(|_| "v4 literal index")?;
        let (source, sha) = *OLD519.get(index).ok_or("v4 foreign literal index")?;
        if !seen.insert(index) {
            return Err("v4 duplicate literal index");
        }
        let mut matches = old.iter().filter(|r| r["source"] == source);
        let prior = matches.next().ok_or("v4 missing old row")?;
        if matches.next().is_some() || super::digest(&super::canonical(prior)) != sha {
            return Err("v4 complete old row drift");
        }
        let hash = tuple[2].as_str().ok_or("v4 current hash")?;
        let current = serde_json::json!({"length":tuple[1].clone(), "sha256":hash,
            "archive":format!("e/{hash}")});
        super::pin(&current)?;
        if current == prior["current"] {
            return Err("v4 unchanged replacement");
        }
        let extra = tuple[3].as_array().ok_or("v4 appended views")?;
        for view in extra {
            super::pin(view)?;
        }
        let mut full = prior.clone();
        full["current"] = current;
        let mut retained = prior["current"].clone();
        retained["name"] = "whole-pre-overlay-v2-current".into();
        let views = full["views"].as_array_mut().ok_or("v4 old views")?;
        views.push(retained);
        views.extend(extra.iter().cloned());
        *row = full;
    }
    if seen.len() != OLD519.len() {
        return Err("v4 missing literal reference");
    }
    if expanded["additions"]
        .as_array()
        .ok_or("v4 additions")?
        .iter()
        .any(|row| row.get("old519").is_some())
    {
        return Err("v4 reference in additions");
    }
    expand(before, policy, &expanded)
}

// Exactly two additional immutable rows; the complete V4 reader below remains
// the authority for its original two references and every downstream check.
pub(super) fn expand_v5(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    if page["schema"] != SCHEMA_V5 {
        return Err("v5 schema");
    }
    let old = before["rows"].as_array().ok_or("v5 old rows")?;
    if old.len() != 519 {
        return Err("v5 whole old519");
    }
    let mut expanded = page.clone();
    expanded["schema"] = SCHEMA_V4.into();
    let mut seen = BTreeSet::new();
    for row in expanded["replacements"]
        .as_array_mut()
        .ok_or("v5 replacements")?
    {
        let Some(reference) = row.get("old519") else {
            if EXTRA519.iter().any(|(source, _)| row["source"] == *source) {
                return Err("v5 missing literal inheritance");
            }
            continue;
        };
        let tuple = reference.as_array().ok_or("v5 tuple")?;
        if tuple.len() != 4 {
            return Err("v5 tuple fields");
        }
        let index = usize::try_from(tuple[0].as_u64().ok_or("v5 literal index")?)
            .map_err(|_| "v5 literal index")?;
        if index < 2 {
            continue;
        } // V4 validates these complete records, unchanged.
        if row.as_object().ok_or("v5 record")?.len() != 1 {
            return Err("v5 reference fields");
        }
        let (source, sha) = *EXTRA519.get(index - 2).ok_or("v5 foreign literal index")?;
        if !seen.insert(index) {
            return Err("v5 duplicate literal index");
        }
        let mut matches = old.iter().filter(|r| r["source"] == source);
        let prior = matches.next().ok_or("v5 missing old row")?;
        if matches.next().is_some() || super::digest(&super::canonical(prior)) != sha {
            return Err("v5 complete old row drift");
        }
        let hash = tuple[2].as_str().ok_or("v5 current hash")?;
        let current = serde_json::json!({"length":tuple[1].clone(),"sha256":hash,"archive":format!("e/{hash}")});
        super::pin(&current)?;
        if current == prior["current"] {
            return Err("v5 unchanged replacement");
        }
        let extra = tuple[3].as_array().ok_or("v5 appended views")?;
        for view in extra {
            super::pin(view)?;
        }
        let mut full = prior.clone();
        full["current"] = current;
        let mut retained = prior["current"].clone();
        retained["name"] = "whole-pre-overlay-v2-current".into();
        let views = full["views"].as_array_mut().ok_or("v5 old views")?;
        views.push(retained);
        views.extend(extra.iter().cloned());
        *row = full;
    }
    if seen.len() != EXTRA519.len() {
        return Err("v5 missing literal reference");
    }
    expand_v4(before, policy, &expanded)
}

#[cfg(test)]
mod tests {
    use super::super::{canonical, compose_values, compose_values_v2, digest, held_4a6eb};
    use super::*;
    fn record(source: &str) -> Value {
        serde_json::json!({"source":source,"compiled":true,"current":{"length":1,
            "sha256":"a".repeat(64),"archive":"e/fixture"},"views":[],"unknown":{"kept":[1,2]}})
    }
    fn fixture() -> (Value, Value, Value, Value) {
        let child = record("src/v3_fixture.rs");
        let mut root = record(ROOTS[0]);
        root["fragments"] = serde_json::json!([child]);
        let before = serde_json::json!({"rows":[root,child],"source_cap":9753094,"unknown":[3]});
        let pred = serde_json::json!({"rows":2,"logical_sha256":digest(&canonical(&before))});
        let page = serde_json::json!({"schema":"semaprax.finite-source-overlay-page.v2",
            "index":1,"total_pages":1,"predecessor":pred,"replacements":[root],"additions":[]});
        let policy = serde_json::json!({"predecessor":pred,"replacements":[ROOTS[0]],
            "additions":[],"new_fragment_edges":[]});
        let mut compact = page.clone();
        compact["schema"] = SCHEMA.into();
        compact["replacements"][0]["fragments"][0] =
            serde_json::json!({"canonical_source":"src/v3_fixture.rs"});
        (before, policy, page, compact)
    }
    #[test]
    fn v3_exact_canonical_expansion_keeps_every_v2_field_order_and_inverse() {
        let (before, policy, page, compact) = fixture();
        assert_eq!(expand(&before, &policy, &compact).unwrap(), page);
        assert_eq!(
            compose_values(before.clone(), &policy, &compact).unwrap(),
            compose_values_v2(before.clone(), &policy, &page).unwrap()
        );
        assert_eq!(
            compose_values(before.clone(), &policy, &compact).unwrap(),
            before
        );
    }
    #[test]
    fn v3_refuses_foreign_missing_duplicate_nested_and_noncompiled_references() {
        let (before, policy, _, compact) = fixture();
        for mode in 0..8 {
            let b = before.clone();
            let mut p = compact.clone();
            match mode {
                0 => p["replacements"][0]["source"] = "src/foreign_root.rs".into(),
                1 => p["replacements"][0]["fragments"][0][REFERENCE] = "../foreign.rs".into(),
                2 => {
                    let item = p["replacements"][0]["fragments"][0].clone();
                    p["replacements"][0]["fragments"]
                        .as_array_mut()
                        .unwrap()
                        .push(item);
                }
                3 => p["replacements"][0]["fragments"][0]["extra"] = true.into(),
                4 => {
                    let mut nested = b["rows"][1].clone();
                    nested["fragments"] = serde_json::json!([]);
                    p["replacements"].as_array_mut().unwrap().push(nested);
                }
                5 => {
                    let mut foreign = b["rows"][1].clone();
                    foreign["compiled"] = false.into();
                    p["replacements"].as_array_mut().unwrap().push(foreign);
                }
                6 => {
                    let item = p["replacements"][0].clone();
                    p["replacements"].as_array_mut().unwrap().push(item);
                }
                _ => p["replacements"][0]["fragments"][0][REFERENCE] = ROOTS[0].into(),
            }
            assert!(expand(&b, &policy, &p).is_err(), "mode={mode}");
        }
        let mut p = compact.clone();
        p["index"] = 2.into();
        assert!(compose_values(before.clone(), &policy, &p).is_err());
        let mut p = compact;
        p["replacements"][0]["unknown"] = false.into();
        assert!(compose_values(before, &policy, &p).is_err());
    }
    #[test]
    fn v3_replays_the_complete_frozen_v2_page_through_unchanged_composer() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |name: &str, n, sha| {
            assert!(n <= 262144);
            held_4a6eb(&root.join(name), n, sha).unwrap()
        };
        let raw = read(
            "e/381c6d9fb1b93e2c92c2863ecdeeee84ea8cf4e5eaa08e7e4381fe85d5ca66fc",
            247814,
            "381c6d9fb1b93e2c92c2863ecdeeee84ea8cf4e5eaa08e7e4381fe85d5ca66fc",
        );
        let policy = read(
            "e/1d3bcd7ebbb484f1909e120b8c2f1e9af399d5b4306e0f9789371df8154f7965",
            28483,
            "1d3bcd7ebbb484f1909e120b8c2f1e9af399d5b4306e0f9789371df8154f7965",
        );
        let page: Value = serde_json::from_str(&raw).unwrap();
        let policy: Value = serde_json::from_str(&policy).unwrap();
        let before = super::super::super::packet_before_overlay();
        let mut compact = page.clone();
        compact["schema"] = SCHEMA.into();
        for group in ["replacements", "additions"] {
            for row in compact[group].as_array_mut().unwrap() {
                if ROOTS.contains(&row["source"].as_str().unwrap()) {
                    for child in row["fragments"].as_array_mut().unwrap() {
                        *child = serde_json::json!({"canonical_source":child["source"].clone()});
                    }
                }
            }
        }
        assert_eq!(expand(&before, &policy, &compact).unwrap(), page);
        assert_eq!(
            compose_values(before.clone(), &policy, &compact).unwrap(),
            compose_values_v2(before, &policy, &page).unwrap()
        );
        let source = include_str!("main_e177_overlay_v2.rs");
        let body = source
            .split_once("fn compose_values_v2(")
            .unwrap()
            .1
            .split_once("\npub(super) fn compose")
            .unwrap()
            .0;
        let restored = format!("fn compose_values({body}");
        assert_eq!(restored.len(), 5763);
        assert_eq!(
            digest(restored.as_bytes()),
            "db2c2411d57188a8397ca9e5e87f192e99be0f1d6cd7105f5c90ac4fe7419402"
        );
    }

    fn v4_fixture() -> (Value, Value, Value, Value) {
        let before = super::super::super::packet_before_overlay();
        let predecessor =
            serde_json::json!({"rows":519,"logical_sha256":digest(&canonical(&before))});
        let mut rows = Vec::new();
        let mut refs = Vec::new();
        let mut paths = Vec::new();
        for prior in before["rows"].as_array().unwrap() {
            let Some(index) = OLD519
                .iter()
                .position(|(source, _)| prior["source"] == *source)
            else {
                continue;
            };
            let hash = digest(format!("fixed-v4-current-{index}").as_bytes());
            let current =
                serde_json::json!({"length":1,"sha256":hash,"archive":format!("e/{hash}")});
            let extra = serde_json::json!([{"name":"complete-extra","length":1,
                "sha256":hash,"archive":format!("e/{hash}"),"opaque":{"kept":[3,1,2]}}]);
            let mut row = prior.clone();
            row["current"] = current;
            let mut retained = prior["current"].clone();
            retained["name"] = "whole-pre-overlay-v2-current".into();
            row["views"].as_array_mut().unwrap().push(retained);
            row["views"]
                .as_array_mut()
                .unwrap()
                .extend(extra.as_array().unwrap().iter().cloned());
            paths.push(prior["source"].clone());
            rows.push(row);
            refs.push(serde_json::json!({"old519":[index,1,hash,extra]}));
        }
        assert_eq!(rows.len(), 2);
        let full = serde_json::json!({"schema":"semaprax.finite-source-overlay-page.v2",
            "index":1,"total_pages":1,"predecessor":predecessor,"replacements":rows,"additions":[]});
        let policy = serde_json::json!({"predecessor":predecessor,"replacements":paths,
            "additions":[],"new_fragment_edges":[]});
        let mut compact = full.clone();
        compact["schema"] = SCHEMA_V4.into();
        compact["replacements"] = refs.into();
        (before, policy, full, compact)
    }

    #[test]
    fn v4_fixed_old_rows_expand_to_every_original_field_and_whole_inverse() {
        let (before, policy, full, compact) = v4_fixture();
        assert_eq!(expand_v4(&before, &policy, &compact).unwrap(), full);
        assert_eq!(
            compose_values(before.clone(), &policy, &compact).unwrap(),
            compose_values_v2(before.clone(), &policy, &full).unwrap()
        );
        let current = compose_values(before.clone(), &policy, &compact).unwrap();
        let mut restored = current.clone();
        restored["rows"] = before["rows"].clone();
        assert_eq!(
            restored, before,
            "entire predecessor object and ordered inventories"
        );
        for prior in before["rows"].as_array().unwrap() {
            let actual = current["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["source"] == prior["source"])
                .unwrap();
            if OLD519.iter().any(|(source, _)| prior["source"] == *source) {
                let mut inverse = actual.clone();
                inverse["current"] = prior["current"].clone();
                inverse["views"] = prior["views"].clone();
                assert_eq!(&inverse, prior, "every inherited field and unknown field");
            } else {
                assert_eq!(actual, prior);
            }
        }
    }

    #[test]
    fn v4_refuses_missing_foreign_duplicate_tampered_drift_and_unknown_version() {
        let (before, policy, full, compact) = v4_fixture();
        for mode in 0..15 {
            let mut b = before.clone();
            let mut p = compact.clone();
            match mode {
                0 => {
                    p["replacements"].as_array_mut().unwrap().pop();
                }
                1 => p["replacements"][0]["old519"][0] = 2.into(),
                2 => p["replacements"][1] = p["replacements"][0].clone(),
                3 => p["replacements"][0]["extra"] = true.into(),
                4 => p["replacements"][0]["old519"][2] = "A".repeat(64).into(),
                5 => p["replacements"][0]["old519"][1] = 9753095.into(),
                6 => p["replacements"][0]["old519"][3] = Value::Null,
                7 => p["replacements"][0]["old519"]
                    .as_array_mut()
                    .unwrap()
                    .push(true.into()),
                8 => p["replacements"][0] = full["replacements"][0].clone(),
                9 => p["schema"] = "semaprax.finite-source-overlay-page.v5".into(),
                10 => {
                    let index = b["rows"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .position(|r| r["source"] == OLD519[0].0)
                        .unwrap();
                    b["rows"][index]["unknown"] = true.into();
                }
                11 => {
                    let index = b["rows"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .position(|r| r["source"] == OLD519[0].0)
                        .unwrap();
                    b["rows"][index]["current"]["length"] = 0.into();
                }
                12 => {
                    let item = p["replacements"][0].clone();
                    p["additions"].as_array_mut().unwrap().push(item);
                }
                13 => p["replacements"][0]["old519"][0] = true.into(),
                _ => p["replacements"][0]["old519"][3][0]["archive"] = "../foreign".into(),
            }
            assert!(compose_values(b, &policy, &p).is_err(), "mode={mode}");
        }
        let mut p = full;
        p["replacements"][0]["unknown"] = true.into();
        assert!(compose_values_v2(before, &policy, &p).is_err());
    }
    fn v5_fixture() -> (Value, Value, Value, Value) {
        let (before, mut policy, mut full, mut compact) = v4_fixture();
        compact["schema"] = SCHEMA_V5.into();
        for (offset, (source, _)) in EXTRA519.iter().enumerate() {
            let prior = before["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["source"] == *source)
                .unwrap();
            let hash = digest(format!("fixed-v5-current-{offset}").as_bytes());
            let mut row = prior.clone();
            row["current"] =
                serde_json::json!({"length":1,"sha256":hash,"archive":format!("e/{hash}")});
            let mut retained = prior["current"].clone();
            retained["name"] = "whole-pre-overlay-v2-current".into();
            row["views"].as_array_mut().unwrap().push(retained);
            let extra = serde_json::json!([{"name":"v5-extra","length":1,"sha256":hash,"archive":format!("e/{hash}"),"opaque":{"kept":[8,3,1]}}]);
            row["views"]
                .as_array_mut()
                .unwrap()
                .extend(extra.as_array().unwrap().iter().cloned());
            policy["replacements"]
                .as_array_mut()
                .unwrap()
                .push((*source).into());
            full["replacements"].as_array_mut().unwrap().push(row);
            compact["replacements"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"old519":[offset + 2,1,hash,extra]}));
        }
        (before, policy, full, compact)
    }
    #[test]
    fn v5_fixed_extra_rows_expand_exactly_and_keep_complete_v4_and_old_inverse() {
        let (before, policy, full, compact) = v5_fixture();
        assert_eq!(expand_v5(&before, &policy, &compact).unwrap(), full);
        let result = compose_values(before.clone(), &policy, &compact).unwrap();
        assert_eq!(
            result,
            compose_values_v2(before.clone(), &policy, &full).unwrap()
        );
        for (row, prior) in result["rows"]
            .as_array()
            .unwrap()
            .iter()
            .zip(before["rows"].as_array().unwrap())
        {
            assert_eq!(row["source"], prior["source"]);
            let mut restored = row.clone();
            restored["current"] = prior["current"].clone();
            restored["views"] = prior["views"].clone();
            assert_eq!(restored, *prior, "complete unknown fields and row order");
            let count = prior["views"].as_array().unwrap().len();
            assert_eq!(
                &row["views"].as_array().unwrap()[..count],
                prior["views"].as_array().unwrap()
            );
        }
        // V4 is still independently accepted with its original exact two rows.
        let (old, policy, whole, v4) = v4_fixture();
        assert_eq!(expand_v4(&old, &policy, &v4).unwrap(), whole);
    }
    #[test]
    fn v5_refuses_missing_foreign_duplicate_tampered_drift_and_unknown_version() {
        let (before, policy, full, compact) = v5_fixture();
        let at = compact["replacements"]
            .as_array()
            .unwrap()
            .iter()
            .position(|r| r["old519"][0] == 2)
            .unwrap();
        for mode in 0..15 {
            let mut b = before.clone();
            let mut p = compact.clone();
            match mode {
                0 => {
                    p["replacements"].as_array_mut().unwrap().remove(at);
                }
                1 => p["replacements"][at]["old519"][0] = 4.into(),
                2 => {
                    let item = p["replacements"][at].clone();
                    p["replacements"].as_array_mut().unwrap().push(item);
                }
                3 => p["replacements"][at]["extra"] = true.into(),
                4 => p["replacements"][at]["old519"][2] = "bad".into(),
                5 => p["replacements"][at]["old519"][1] = 9753095.into(),
                6 => p["replacements"][at]["old519"][3] = Value::Null,
                7 => p["replacements"][at]["old519"]
                    .as_array_mut()
                    .unwrap()
                    .push(true.into()),
                8 => p["schema"] = "semaprax.finite-source-overlay-page.v500".into(),
                9 | 10 => {
                    let row = b["rows"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|r| r["source"] == EXTRA519[0].0)
                        .unwrap();
                    if mode == 9 {
                        row["unknown"] = true.into();
                    } else {
                        row["current"]["length"] = 1.into();
                    }
                }
                11 => {
                    let item = p["replacements"][at].clone();
                    p["additions"].as_array_mut().unwrap().push(item);
                }
                12 => p["replacements"][at] = full["replacements"][at].clone(),
                13 => p["replacements"][at]["old519"][0] = true.into(),
                _ => {
                    let item = b["rows"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|r| r["source"] == EXTRA519[0].0)
                        .unwrap()
                        .clone();
                    b["rows"].as_array_mut().unwrap().push(item);
                }
            }
            assert!(compose_values(b, &policy, &p).is_err(), "mode={mode}");
        }
    }
}
