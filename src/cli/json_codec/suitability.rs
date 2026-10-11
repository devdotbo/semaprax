//! Read-only authenticated shape explanation; no output destination or fallback.
use super::*;
use semaprax::project::JsonCodecSuitabilityOptions;
const USAGE: &str = "json-codec <project> --explain --type <stable-id> [--source <path>] [--target native64|wasm32] [--max-string-bytes N] [--max-array-items N] [--max-bytes N] [--revision <digest>] [--offset N]";
fn options(args: &[String]) -> Option<(PathBuf, JsonCodecSuitabilityOptions)> {
    if args.len() < 4 || args[0].is_empty() || args[0].starts_with('-') {
        return None;
    }
    let mut options = JsonCodecSuitabilityOptions::default();
    let mut seen = std::collections::BTreeSet::new();
    let mut index = 1;
    while index < args.len() {
        let flag = args[index].as_str();
        if !seen.insert(flag) {
            return None;
        }
        if flag == "--explain" {
            index += 1;
            continue;
        }
        let value = args.get(index + 1)?.as_str();
        if value.is_empty() || value.starts_with('-') {
            return None;
        }
        match flag {
            "--type" => options.record = value.into(),
            "--source" => options.source = Some(value.into()),
            "--target" if matches!(value, "native64" | "wasm32") => {
                options.target = Some(value.into())
            }
            "--revision" => options.revision = Some(value.into()),
            "--max-string-bytes" => options.max_string_bytes = Some(profile::bound(value, 64)?),
            "--max-array-items" => options.max_array_items = Some(profile::bound(value, 256)?),
            "--max-bytes" => {
                options.max_bytes = profile::bound(value, 16384)?;
                if options.max_bytes < 512 {
                    return None;
                }
            }
            "--offset" => {
                options.offset = value.parse().ok()?;
                if options.offset >= 12 || options.offset.to_string() != value {
                    return None;
                }
            }
            _ => return None,
        }
        index += 2;
    }
    if !seen.contains("--explain")
        || options.record.is_empty()
        || (options.offset != 0 && options.revision.is_none())
    {
        return None;
    }
    Some((PathBuf::from(&args[0]), options))
}
pub(super) fn run(args: &[String], report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let (path, options) = options(args).ok_or_else(|| {
        eprintln!("{USAGE}");
        2
    })?;
    let project = super::super::project::resolve_positional(path);
    let output = semaprax::project::with_authenticated_project(&project, |snapshot| {
        semaprax::project::explain_json_codec_suitability(&snapshot.retain_revision(), &options)
    })
    .map_err(|errors| report(&errors))?;
    print!("{output}");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn args(tail: &[&str]) -> Vec<String> {
        std::iter::once(".")
            .chain(tail.iter().copied())
            .map(str::to_owned)
            .collect()
    }
    #[test]
    fn compact_readonly_defaults_and_continuations_have_exact_grammar() {
        let (_, o) = options(&args(&["--explain", "--type", "app.Row"])).unwrap();
        assert_eq!(o.max_bytes, 4096);
        assert_eq!(o.offset, 0);
        assert!(o.source.is_none());
        assert!(o.target.is_none());
        assert!(options(&args(&[
            "--explain",
            "--type",
            "app.Row",
            "--offset",
            "1",
            "--revision",
            "current"
        ]))
        .is_some());
        for tail in [
            vec!["--explain", "--type", "app.Row", "--output", "a"],
            vec!["--explain", "--type", "app.Row", "--offset", "1"],
            vec!["--explain", "--type", "app.Row", "--max-bytes", "0512"],
            vec!["--explain", "--type", "app.Row", "--type", "other"],
            vec!["--explain", "--type", "app.Row", "--target", "hosted"],
        ] {
            assert!(options(&args(&tail)).is_none());
        }
    }
}
