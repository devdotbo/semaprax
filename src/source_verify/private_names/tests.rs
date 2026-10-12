use super::{PrivateNames, Providers};
use crate::ast::TypeDeclaration;
use crate::diagnostic::Diagnostic;

/// A held closure whose single copy is the last declaration of the program.
struct Held<'a>(&'a TypeDeclaration);
impl Providers for Held<'_> {
    fn provider(&self, index: usize) -> Option<(&str, &TypeDeclaration)> {
        (index == 0).then_some(("held.provider", self.0))
    }
}

const PROVIDER: &str = "module held.provider;

@id(\"copy\")
record Decoded {
    @id(\"copy.n\")
    n: i64,
}

@id(\"zero\")
fn zero() -> i64
{
    0
}
";

// `_spx_layout_0` is an authored declaration below the bound copies; it keeps
// its authored spelling even though it shares the private prefix.
const SYNTHETIC: &str = "module held.caller;

@id(\"authored\")
record _spx_layout_0 {
    @id(\"authored.n\")
    n: i64,
}

@id(\"copy\")
record _spx_layout_1 {
    @id(\"copy.n\")
    n: i64,
}

@id(\"pick\")
fn pick(value: _spx_layout_1, other: _spx_layout_0) -> i64
{
    if value.n == 0 { value } else { other }
}
";

const PRESENTED: &str =
    "`if` branches return different types: Decoded [unimported @id(\"copy\") from held.provider] \
     and _spx_layout_0";
const ORDINARY: &str = "`if` branches return different types: _spx_layout_1 and _spx_layout_0";

fn join(diagnostics: &[Diagnostic]) -> &str {
    diagnostics
        .iter()
        .find(|d| d.code == "SPX-T211")
        .map(|d| d.message.as_ref())
        .unwrap_or_else(|| panic!("branch join mismatch: {diagnostics:?}"))
}

#[test]
fn presentation_binds_only_the_exact_program_and_leaves_no_ambient_scope() {
    let provider = crate::parse(PROVIDER, "provider.spx").unwrap();
    let program = crate::parse(SYNTHETIC, "caller.spx").unwrap();
    let held = Held(&provider.types[0]);
    let base = program.types.len() - 1;
    let view = PrivateNames::new(&program, &program, base, &held);

    let presented = crate::source_verify::verify_presenting(&program, Some(view));
    assert_eq!(join(&presented), PRESENTED);
    assert!(
        presented
            .iter()
            .all(|d| !d.message.contains("_spx_layout_1")
                && d.help
                    .as_deref()
                    .is_none_or(|h| !h.contains("_spx_layout_1"))),
        "{presented:?}"
    );

    // Control: the unbound verifier, and the same program verified again after
    // the presenting verification returned, render ordinary source text.
    assert_eq!(join(&crate::source_verify::verify(&program)), ORDINARY);
    assert_eq!(
        join(&crate::source_verify::verify_presenting(&program, None)),
        ORDINARY
    );

    // Exact program drift: an equal clone at another address is not the
    // program the copies were appended to, so the view presents nothing.
    let drifted = program.clone();
    assert_eq!(drifted, program);
    assert_eq!(
        join(&crate::source_verify::verify_presenting(
            &drifted,
            Some(view)
        )),
        ORDINARY
    );

    // Hostile base: a view bound past the copy presents nothing. Presentation
    // follows bound addresses only, never the shared `_spx_layout_` prefix.
    let past = PrivateNames::new(&program, &program, program.types.len(), &held);
    assert_eq!(
        join(&crate::source_verify::verify_presenting(
            &program,
            Some(past)
        )),
        ORDINARY
    );
}
