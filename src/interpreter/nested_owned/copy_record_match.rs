//! Exact by-value scalar observation of the existing flat Copy-row profile.
use crate::hir::{self, DeclarationIndex, OwnershipMode, ResolvedExpr, ResolvedExprKind};

pub(in crate::interpreter) fn admitted(
    index: &DeclarationIndex,
    expression: &ResolvedExpr,
) -> bool {
    let ResolvedExprKind::Match {
        mode,
        scrutinee,
        arms,
    } = &expression.kind
    else {
        return false;
    };
    let [arm] = arms.as_slice() else { return false };
    *mode == hir::ResolvedMatchMode::Value
        && scrutinee.ownership == OwnershipMode::Value
        && expression.ownership == OwnershipMode::Value
        && super::super::is_admitted_resolved_scalar(&expression.ty)
        && hir::copy_record_collection::admitted(index, &scrutinee.ty)
        && arm.guard.is_none()
        && arm.value.ty == expression.ty
        && arm.value.ownership == OwnershipMode::Value
        && super::pattern_is_exact(
            index,
            *mode,
            &scrutinee.ty,
            &arm.pattern,
            &mut std::collections::BTreeSet::new(),
        )
}

pub(in crate::interpreter) fn bind(
    index: &DeclarationIndex,
    expression: &ResolvedExpr,
    record: &std::sync::Arc<super::super::OwnedRecordValue>,
    fields: &[hir::ResolvedRecordMatchPatternField],
    bindings: &mut Vec<(hir::ValueId, super::super::Value)>,
) -> Result<(), super::super::Flow> {
    if !admitted(index, expression) {
        return Err(super::super::Flow::Guard(
            "plain value record match lacks exact Copy-row authority",
        ));
    }
    let ResolvedExprKind::Match { scrutinee, .. } = &expression.kind else {
        unreachable!()
    };
    super::bind_borrowed_pattern(index, &scrutinee.ty, record, fields, bindings)
}

#[cfg(test)]
mod tests {
    use super::admitted;
    use crate::hir::{self, OwnershipMode, ResolvedExprKind, ResolvedMatchPattern, ResolvedType};
    #[test]
    fn scalar_copy_row_observation_requires_exact_pattern_type_fields_and_value_modes() {
        let source = r#"module rows;
@id("row") record Row {@id("row.n") n:i64,@id("row.good") good:bool,}
@id("app.main") fn main()->i64 {let value=Row{n:7,good:true};match value {Row{n,good}=>if n==7 && good {42}else{0},}}
"#;
        let path = std::env::temp_dir().join(format!("copy-row-match-{}.spx", std::process::id()));
        std::fs::write(&path, source).unwrap();
        let checked = crate::check(source, &path).unwrap();
        let execution =
            crate::interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
        crate::interpreter::verify_envelope(&execution.envelope).unwrap();
        let wire: serde_json::Value = serde_json::from_str(&execution.envelope).unwrap();
        assert!(execution.returned, "{}", execution.envelope);
        assert_eq!(wire["payload"]["outcome"]["value"], "42");
        std::fs::remove_file(path).unwrap();
        let program = hir::resolve(&checked).unwrap();
        let main = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.main")
            .unwrap();
        let ResolvedExprKind::Block { tail, .. } = &main.body.kind else {
            panic!("body block")
        };
        assert!(admitted(&program.declarations, tail));
        for mutation in 0..7 {
            let mut forged = tail.as_ref().clone();
            let ResolvedExprKind::Match { mode, arms, .. } = &mut forged.kind else {
                unreachable!()
            };
            let ResolvedMatchPattern::Record {
                record,
                instance,
                fields,
            } = &mut arms[0].pattern
            else {
                unreachable!()
            };
            match mutation {
                0 => *mode = hir::ResolvedMatchMode::Own,
                1 => *record = hir::DeclarationId::new("foreign.row"),
                2 => *instance = ResolvedType::Bool,
                3 => fields[0].field = hir::DeclarationId::new("foreign.field"),
                4 => fields.push(fields[0].clone()),
                5 | 6 => {
                    let hir::ResolvedRecordMatchFieldPattern::Binding(binding) =
                        &mut fields[0].pattern
                    else {
                        unreachable!()
                    };
                    if mutation == 5 {
                        binding.ty = ResolvedType::String;
                    } else {
                        binding.ownership = OwnershipMode::Own;
                    }
                }
                _ => unreachable!(),
            }
            assert!(
                !admitted(&program.declarations, &forged),
                "mutation {mutation}"
            );
        }
    }
}
