// Shared source generator for the native and semantic-work fixture owners.
// Scalar padding changes identities only; it adds no allocation/call/loop.
pub(super) fn source(template: &str) -> String {
    let older = "    let older = Choice::Data";
    assert_eq!(template.matches(older).count(), 1);
    let mut source = template.to_owned();
    for index in 3..=9 {
        let old = format!("    let pad{index} = {index};\n");
        assert_eq!(source.matches(&old).count(), 1);
        source = source.replace(&old, "");
    }
    let mut padding = String::new();
    for index in 0..997 {
        use std::fmt::Write;
        writeln!(&mut padding, "    let order_padding{index} = 0;").unwrap();
    }
    let generated = source.replacen(older, &(padding + older), 1);
    assert_eq!(
        generated.matches("bytes_copy(").count(),
        template.matches("bytes_copy(").count(),
        "padding preserves all original allocation sites"
    );
    generated
}
pub(super) fn binding<'a>(
    function: &'a crate::hir::ResolvedFunction,
    name: &str,
) -> (
    &'a crate::hir::ResolvedBinding,
    &'a crate::hir::ResolvedExpr,
) {
    let crate::hir::ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("run body is a block");
    };
    let mut matches = statements.iter().filter_map(|statement| match statement {
        crate::hir::ResolvedStatement::Let { binding, value, .. } if binding.name == name => {
            Some((binding, value))
        }
        _ => None,
    });
    let found = matches.next().expect("current named binding exists");
    assert!(
        matches.next().is_none(),
        "binding name selects exactly one current let"
    );
    found
}
pub(super) fn assert_opposing_ids(function: &crate::hir::ResolvedFunction) {
    use crate::hir::{FunctionExecutionId, ValueId};
    let owner = FunctionExecutionId::Monomorphic(function.id.clone());
    for (name, path) in [
        ("older", "body.s999"),
        ("newer", "body.s1000"),
        ("before", "body.s1001"),
        ("joined", "body.s1002"),
        ("after", "body.s1003"),
    ] {
        assert_eq!(
            binding(function, name).0.id,
            ValueId::local(&owner, path),
            "{name}: exact current source identity"
        );
    }
    assert!(
        binding(function, "newer").0.id < binding(function, "older").0.id,
        "decimal path-length identity 10 precedes 9 at this actual join"
    );
    use crate::cleanup_plan::{ExitContinuation, StatusLane, StorageId};
    let older = StorageId::Value(binding(function, "older").0.id.clone());
    let newer = StorageId::Value(binding(function, "newer").0.id.clone());
    let order = |name| {
        let site = &binding(function, name).1.id;
        let mut exits = function.cleanup_plan.exits.iter().filter(|exit| {
            matches!(&exit.continuation,
            ExitContinuation::ReturnFailure { source }
                if source.expression == *site && source.lane == StatusLane::OperationFailure)
        });
        let exit = exits
            .next()
            .expect("named call has its current exact failure exit");
        assert!(
            exits.next().is_none(),
            "named call selects exactly one failure exit"
        );
        exit.finalize_in_order
            .iter()
            .filter(|action| action.source.storage == older || action.source.storage == newer)
            .map(|action| action.source.storage.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(order("before"), [newer.clone(), older.clone()]);
    assert_eq!(order("after"), [older, newer]);
}
pub(super) fn assert_projection(checked: &crate::ast::Program) {
    let projection = crate::format::canonical(checked);
    let parsed =
        crate::parse(&projection, "terminal-order-canonical.spx").expect("canonical source parses");
    assert_eq!(crate::format::canonical(&parsed), projection);
    crate::check(&projection, "terminal-order-canonical.spx").expect("canonical source checks");
}
