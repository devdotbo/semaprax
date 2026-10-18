//! Same-call borrowing only; no stored packet or acquisition cache.
use super::{actual, clippy_stage, digest, packet, read_view, value_4a6eb};

#[rustfmt::skip] // Exact extracted historical lookup body.
pub(super) fn from_packet(packet: &serde_json::Value, source: &str, expected: &str, supplied: &str) -> String {
    let Some(row) = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == source)
    else {
        return crate::source_reflows_merge::next_big_batch_inputs::value_local(
            source, expected, supplied,
        );
    };
    let Some(prior) = row["views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["sha256"] == expected)
    else {
        return supplied.to_owned();
    };
    let current = actual(row);
    let previous = read_view(prior);
    if supplied == current || supplied == previous {
        previous
    } else {
        supplied.to_owned()
    }
}

#[rustfmt::skip] // Exact historical continuation; only first lookup is borrowed.
pub(super) fn with_packet(authenticated: &serde_json::Value, source: &str, expected: &str, supplied: &str) -> String {
    // Current and prior logical-table views retain their complete scope before
    // the separately authenticated historical Clippy-stage acquisition.
    let direct = from_packet(authenticated, source, expected, supplied);
    if digest(direct.as_bytes()) == expected {
        return direct;
    }
    if let Some(value) = clippy_stage::requested(source, expected, supplied) {
        let _ = packet();
        return value;
    }
    let normalized = clippy_stage::normalize(source, supplied);
    let result = value_4a6eb(source, expected, normalized.as_ref());
    if normalized.as_ref() != supplied
        && result == normalized.as_ref()
        && digest(result.as_bytes()) != expected
    {
        supplied.to_owned()
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::from_packet;
    use crate::source_reflows_merge::main_e177_final_inputs::digest;
    use std::path::PathBuf;

    struct Files(PathBuf);
    impl Drop for Files {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn files() -> Files {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("semaprax-packet-borrow-{}-{n}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        Files(dir)
    }
    fn panic_text(f: impl FnOnce() + std::panic::UnwindSafe) -> String {
        let panic = std::panic::catch_unwind(f).expect_err("fresh authentication must refuse");
        if let Some(text) = panic.downcast_ref::<String>() {
            text.clone()
        } else {
            panic.downcast_ref::<&str>().unwrap().to_string()
        }
    }

    #[test]
    fn borrowed_lookup_rechecks_current_on_each_invocation_before_history() {
        let files = files();
        let current = files.0.join("current.rs");
        let old = files.0.join("prior.rs");
        std::fs::write(&current, b"new!").unwrap();
        std::fs::write(&old, b"old!").unwrap();
        let source = current.to_str().unwrap();
        let expected = digest(b"old!");
        let packet = serde_json::json!({"rows":[{"source":source,"compiled":false,
            "current":{"archive":source,"length":4,"sha256":digest(b"new!")},
            "views":[{"archive":old.to_str().unwrap(),"length":4,"sha256":expected}]}]});
        assert_eq!(from_packet(&packet, source, &expected, "new!"), "old!");
        assert_eq!(
            from_packet(&packet, source, &expected, "foreign"),
            "foreign"
        );
        // A second invocation with the same borrowed metadata still reads the
        // physical current channel; missing history cannot precede its refusal.
        std::fs::write(&current, b"drft").unwrap();
        std::fs::remove_file(&old).unwrap();
        assert!(panic_text(|| {
            from_packet(&packet, source, &expected, "old!");
        })
        .contains("length-or-digest"));
        std::fs::write(&current, b"new!extra").unwrap();
        assert!(panic_text(|| {
            from_packet(&packet, source, &expected, "new!");
        })
        .contains("length-or-identity"));
        std::fs::write(&current, [0xff; 4]).unwrap();
        let mut invalid_utf8 = packet.clone();
        invalid_utf8["rows"][0]["current"]["sha256"] = digest(&[0xff; 4]).into();
        assert!(panic_text(|| {
            from_packet(&invalid_utf8, source, &expected, "new!");
        })
        .contains("utf8"));
        std::fs::write(&current, b"new!").unwrap();
        assert!(panic_text(|| {
            from_packet(&packet, source, &expected, "new!");
        })
        .contains("missing"));
    }

    #[test]
    fn packet_borrowing_preserves_complete_old_root_and_both_fresh_fallbacks() {
        let before = include_str!("main_e177_packet_lookup_before.rs.txt");
        assert_eq!(
            digest(before.as_bytes()),
            "b8b5ca81b4e0001c7dc8ffc354da34cf9966f7dee840b58f7e1265c2806479e6"
        );
        let current = include_str!("main_e177_final_inputs.rs");
        let child = include_str!("main_e177_packet_lookup.rs");
        let old_value = before
            .split_once("pub(crate) fn value(source:")
            .unwrap()
            .1
            .split_once("fn value_4a6eb(")
            .unwrap()
            .0;
        let old_lookup = before
            .split_once("fn value_4a6eb(")
            .unwrap()
            .1
            .split_once("#[test]\nfn latest_main")
            .unwrap()
            .0;
        let child_lookup = child.split_once("pub(super) fn from_packet(packet: &serde_json::Value, source:").unwrap().1
            .split_once("#[rustfmt::skip] // Exact historical continuation; only first lookup is borrowed.\npub(super) fn with_packet(").unwrap().0;
        assert_eq!(
            child_lookup,
            old_lookup.strip_prefix("source:").unwrap().replacen(
                "    let packet = packet();\n",
                "",
                1
            ) + "\n"
        );
        let child_value = child
            .split_once("pub(super) fn with_packet(authenticated: &serde_json::Value, source:")
            .unwrap()
            .1
            .split_once("#[cfg(test)]\nmod tests")
            .unwrap()
            .0;
        assert_eq!(
            child_value,
            old_value.replacen(
                "let direct = value_4a6eb(source, expected, supplied);",
                "let direct = from_packet(authenticated, source, expected, supplied);",
                1
            ) + "\n"
        );
        let mut restored = current.replace(
            "#[path = \"main_e177_packet_lookup.rs\"]\nmod packet_lookup;\n",
            "",
        );
        let new_value = "pub(crate) fn value(source: &str, expected: &str, supplied: &str) -> String {\n    packet_lookup::with_packet(&packet(), source, expected, supplied)\n}\n";
        let new_lookup = "fn value_4a6eb(source: &str, expected: &str, supplied: &str) -> String {\n    packet_lookup::from_packet(&packet(), source, expected, supplied)\n}\n";
        assert_eq!(restored.matches(new_value).count(), 1);
        restored = restored.replacen(
            new_value,
            &("pub(crate) fn value(source:".to_owned() + old_value),
            1,
        );
        assert_eq!(restored.matches(new_lookup).count(), 1);
        restored = restored.replacen(new_lookup, &("fn value_4a6eb(".to_owned() + old_lookup), 1);
        let reuse = "        packet_lookup::with_packet(&current, source, expected, supplied)\n";
        assert_eq!(restored.matches(reuse).count(), 1);
        restored = restored.replacen(reuse, "        value(source, expected, supplied)\n", 1);
        assert_eq!(restored, before, "complete independently pinned old root");
    }
}
