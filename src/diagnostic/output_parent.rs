//! Shared `SPX-I301` wording for rejected or missing output parents.
//!
//! Native single-file outputs, Web packages and explicit Project outputs
//! enforce the same parent policy: the inspected directory must already be a
//! real non-reparse directory. These helpers only describe a rejection; they
//! never relax the policy, follow a link for publication, or create a path.

use std::fs;
use std::io;
use std::path::Path;

use super::Diagnostic;

/// Describe an inspected output parent that is not a real non-reparse
/// directory.
///
/// `subject` names the checked directory role, for example
/// `"single-file native output parent"`. `output` is the requested output
/// path; when `directory` is a link that resolves to a directory containing
/// it, the help suggests the equivalent output path below the resolved
/// directory. Resolution is read-only and happens only on this rejection
/// path.
pub fn rejected_directory(
    subject: &str,
    directory: &Path,
    output: &Path,
    metadata: &fs::Metadata,
) -> Diagnostic {
    let is_link = metadata.file_type().is_symlink();
    let kind = if is_link {
        link_kind()
    } else if !metadata.is_dir() {
        "is not a directory"
    } else {
        "is a reparse point"
    };
    let help = if is_link {
        link_help(directory, output)
    } else {
        "choose an output whose parent is an existing real directory".to_owned()
    };
    Diagnostic::io(
        "SPX-I301",
        format!(
            "{subject} {} {kind}; it must be a real non-reparse directory",
            directory.display()
        ),
    )
    .with_help(help)
}

/// Recovery hint for an output parent that could not be inspected.
///
/// Only a missing directory has a hint: create it first. Other inspection
/// failures keep their message without help.
pub fn missing_directory_help(directory: &Path, error: &io::Error) -> Option<String> {
    (error.kind() == io::ErrorKind::NotFound).then(|| {
        format!(
            "create the directory {} first, then rerun the build",
            directory.display()
        )
    })
}

fn link_help(directory: &Path, output: &Path) -> String {
    const FALLBACK: &str = "choose an output whose parent is a real directory";
    match fs::canonicalize(directory) {
        Ok(resolved) if resolved.is_dir() => match output.strip_prefix(directory) {
            Ok(rest) => format!(
                "{} resolves to {}; use the resolved output path {}, or {FALLBACK}",
                directory.display(),
                resolved.display(),
                resolved.join(rest).display()
            ),
            Err(_) => format!(
                "{} resolves to {}; {FALLBACK}",
                directory.display(),
                resolved.display()
            ),
        },
        Ok(resolved) => format!(
            "{} resolves to {}, which is not a directory; {FALLBACK}",
            directory.display(),
            resolved.display()
        ),
        Err(error) => format!(
            "cannot resolve the link {}: {error}; {FALLBACK}",
            directory.display()
        ),
    }
}

#[cfg(windows)]
fn link_kind() -> &'static str {
    "is a symbolic link or junction"
}

#[cfg(not(windows))]
fn link_kind() -> &'static str {
    "is a symbolic link"
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "semaprax-output-parent-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("create scratch root");
            Self(root)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn reject(directory: &Path, output: &Path) -> Diagnostic {
        let metadata = fs::symlink_metadata(directory).expect("inspect fixture");
        rejected_directory("test output parent", directory, output, &metadata)
    }

    #[test]
    fn link_to_directory_suggests_resolved_output() {
        let scratch = Scratch::new("dir-link");
        let real = scratch.0.join("real");
        let link = scratch.0.join("link");
        fs::create_dir(&real).expect("create real directory");
        symlink(&real, &link).expect("create link");
        let resolved = fs::canonicalize(&link).expect("resolve link");

        let diagnostic = reject(&link, &link.join("artifact"));
        assert_eq!(
            diagnostic.message,
            format!(
                "test output parent {} is a symbolic link; it must be a real non-reparse directory",
                link.display()
            )
        );
        assert_eq!(
            diagnostic.help.as_deref(),
            Some(
                format!(
                    "{} resolves to {}; use the resolved output path {}, or choose an output whose parent is a real directory",
                    link.display(),
                    resolved.display(),
                    resolved.join("artifact").display()
                )
                .as_str()
            )
        );
        assert!(diagnostic.path.is_none());
    }

    #[test]
    fn link_to_directory_with_unrelated_output_names_only_resolution() {
        let scratch = Scratch::new("unrelated");
        let real = scratch.0.join("real");
        let link = scratch.0.join("link");
        fs::create_dir(&real).expect("create real directory");
        symlink(&real, &link).expect("create link");
        let resolved = fs::canonicalize(&link).expect("resolve link");

        let diagnostic = reject(&link, Path::new("relative/artifact"));
        assert_eq!(
            diagnostic.help.as_deref(),
            Some(
                format!(
                    "{} resolves to {}; choose an output whose parent is a real directory",
                    link.display(),
                    resolved.display()
                )
                .as_str()
            )
        );
    }

    #[test]
    fn link_to_file_reports_non_directory_target() {
        let scratch = Scratch::new("file-link");
        let file = scratch.0.join("file");
        let link = scratch.0.join("link");
        fs::write(&file, b"x").expect("create file");
        symlink(&file, &link).expect("create link");

        let diagnostic = reject(&link, &link.join("artifact"));
        assert!(diagnostic.message.contains("is a symbolic link"));
        let help = diagnostic.help.expect("link help");
        assert!(help.contains("which is not a directory"), "{help}");
    }

    #[test]
    fn dangling_link_reports_unresolvable_link() {
        let scratch = Scratch::new("dangling");
        let link = scratch.0.join("link");
        symlink(scratch.0.join("absent"), &link).expect("create link");

        let diagnostic = reject(&link, &link.join("artifact"));
        let help = diagnostic.help.expect("link help");
        assert!(
            help.starts_with(&format!("cannot resolve the link {}: ", link.display())),
            "{help}"
        );
        assert!(help.ends_with("; choose an output whose parent is a real directory"));
    }

    #[test]
    fn regular_file_parent_is_not_a_directory() {
        let scratch = Scratch::new("file-parent");
        let file = scratch.0.join("file");
        fs::write(&file, b"x").expect("create file");

        let diagnostic = reject(&file, &file.join("artifact"));
        assert_eq!(
            diagnostic.message,
            format!(
                "test output parent {} is not a directory; it must be a real non-reparse directory",
                file.display()
            )
        );
        assert_eq!(
            diagnostic.help.as_deref(),
            Some("choose an output whose parent is an existing real directory")
        );
    }

    #[test]
    fn only_missing_directory_has_create_help() {
        let directory = Path::new("/absent/parent");
        assert_eq!(
            missing_directory_help(directory, &io::Error::from(io::ErrorKind::NotFound)).as_deref(),
            Some("create the directory /absent/parent first, then rerun the build")
        );
        assert_eq!(
            missing_directory_help(directory, &io::Error::from(io::ErrorKind::PermissionDenied)),
            None
        );
    }
}
