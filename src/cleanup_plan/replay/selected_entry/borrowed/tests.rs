//! Real provider receivers share construction, current membership and Drop.
use super::*;
struct Provider<'a> {
    program: &'a crate::hir::ResolvedProgram,
    declarations: crate::hir::DeclarationIndex,
}
impl ReplayContext for Provider<'_> {
    fn declarations(&self) -> &crate::hir::DeclarationIndex {
        &self.declarations
    }
    fn types(&self) -> &[crate::hir::ResolvedTypeDeclaration] {
        &self.program.types
    }
    fn interfaces(&self) -> &[crate::hir::ResolvedInterface] {
        &self.program.interfaces
    }
    fn functions(&self) -> &[ResolvedFunction] {
        &self.program.functions
    }
    fn function_templates(&self) -> &[crate::hir::ResolvedFunctionTemplate] {
        &self.program.function_templates
    }
    fn function_instances(&self) -> &[crate::hir::ResolvedFunctionInstance] {
        &self.program.function_instances
    }
}
fn program() -> crate::hir::ResolvedProgram {
    let source = include_str!("../../../../../std/data-json-utf8/src/utf8.spx");
    let start = source.find("@id(\"std.data.json.utf8.at_is\")").unwrap();
    let end = source
        .find("@id(\"std.data.json.utf8.offset_at\")")
        .unwrap();
    let selected = format!(
        "module std.data.json.utf8;\n{}\n@id(\"main\") fn main()->i64 {{0}}",
        &source[start..end]
    );
    crate::hir::resolve(&crate::check(&selected, "actual-option-provider-receivers.spx").unwrap())
        .unwrap()
}
fn declarations(
    program: &crate::hir::ResolvedProgram,
    preparation: &mut Preparation,
) -> Result<crate::hir::DeclarationIndex, Diagnostic> {
    let function = &program.functions[0];
    let mut index = preparation.construct(function, |physical, debit| {
        crate::hir::DeclarationIndex::primitive_replay_metered(
            program
                .functions
                .iter()
                .map(|f| (f, crate::hir::IdentityOrigin::Explicit)),
            physical,
            debit,
        )
    })?;
    preparation.append_option_authority(function, &mut index)?;
    preparation.reserve_array::<Provider<'_>>(
        function,
        1,
        "guarded borrowed declaration provider",
    )?;
    Ok(index)
}
#[test]
fn actual_two_provider_receivers_carry_one_preparation_through_session_and_drop() {
    let program = program();
    fn run(
        program: &crate::hir::ResolvedProgram,
        work: usize,
        bytes: usize,
    ) -> (
        Result<(usize, usize), ()>,
        crate::test_allocation_observer::Metrics,
    ) {
        crate::test_allocation_observer::measure(|| {
            let result = (|| {
                let anchor = &program.functions[0];
                let mut preparation = Preparation::with_limits(anchor, work, bytes)?;
                let index = declarations(program, &mut preparation)?;
                let provider = Provider {
                    program,
                    declarations: index,
                };
                let mut session = Session::from_preparation(&provider, anchor, preparation)?;
                for function in &program.functions[..2] {
                    session.borrowed_receivers(function)?;
                }
                let (remaining, charged) = session.metrics();
                let receipt = (work - remaining, charged);
                drop(session);
                drop(provider);
                Ok::<_, Diagnostic>(receipt)
            })();
            // Diagnostic backing is not proof-session storage. Drop it before
            // taking the allocator receipt, including exact one-short refusal.
            result.map_err(|diagnostic| {
                assert_eq!(diagnostic.code, "SPX-H006");
            })
        })
    }
    let (wide, allocation) = run(&program, MAX_REPLAY_WORK_UNITS, 1 << 24);
    let (work, bytes) = wide.unwrap();
    assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated <= bytes);
    let (exact, allocation) = run(&program, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated <= bytes);
    for (work, bytes) in [(work - 1, bytes), (work, bytes - 1)] {
        let (answer, allocation) = run(&program, work, bytes);
        assert!(answer.is_err());
        assert!(!allocation.invalid && allocation.live == 0);
    }
}
#[test]
fn later_member_forgery_cannot_reuse_an_earlier_inventory_or_renewal_lease() {
    for mutation in 0..4 {
        let mut program = program();
        match mutation {
            0 => {
                program.functions[1].cleanup_plan.schema =
                    crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V6
            }
            1 => program.functions[1].cleanup.schema = crate::cleanup::CLEANUP_INVENTORY_SCHEMA_V2,
            2 => program.functions[1].loan_plan.schema = "forged.current.loan",
            _ => program.functions[1].params[0].ownership = OwnershipMode::Own,
        }
        let anchor = &program.functions[0];
        let mut preparation =
            Preparation::with_limits(anchor, MAX_REPLAY_WORK_UNITS, 1 << 24).unwrap();
        let index = declarations(&program, &mut preparation).unwrap();
        let provider = Provider {
            program: &program,
            declarations: index,
        };
        let mut session = Session::from_preparation(&provider, anchor, preparation).unwrap();
        session.borrowed_receivers(anchor).unwrap();
        assert_eq!(
            session
                .borrowed_receivers(&program.functions[1])
                .unwrap_err()
                .code,
            "SPX-H006",
            "later mutant {mutation}"
        );
        let remaining = session.metrics().0;
        assert_eq!(
            session.borrowed_receivers(anchor).unwrap_err().code,
            "SPX-H006"
        );
        assert_eq!(session.metrics().0, remaining);
        assert_eq!(session.finish().err().unwrap().code, "SPX-H006");
    }
    let program = program();
    let detached = program.functions[0].clone();
    let anchor = &program.functions[0];
    let mut preparation = Preparation::with_limits(anchor, MAX_REPLAY_WORK_UNITS, 1 << 24).unwrap();
    let index = declarations(&program, &mut preparation).unwrap();
    let provider = Provider {
        program: &program,
        declarations: index,
    };
    let mut session = Session::from_preparation(&provider, anchor, preparation).unwrap();
    assert_eq!(
        session.borrowed_receivers(&detached).unwrap_err().code,
        "SPX-H006"
    );
}
#[test]
fn canonical_option_metadata_is_current_and_failed_factory_cannot_restart_preparation() {
    let program = program();
    let anchor = &program.functions[0];
    let mut preparation = Preparation::with_limits(anchor, MAX_REPLAY_WORK_UNITS, 1 << 24).unwrap();
    let mut index = declarations(&program, &mut preparation).unwrap();
    crate::hir::borrowed_scalar::option_u8::forge_parameter_index_for_test(&mut index, 1);
    let provider = Provider {
        program: &program,
        declarations: index,
    };
    let mut session = Session::from_preparation(&provider, anchor, preparation).unwrap();
    assert_eq!(
        session.borrowed_receivers(anchor).unwrap_err().code,
        "SPX-H006"
    );
    assert_eq!(session.finish().err().unwrap().code, "SPX-H006");
    let mut preparation = Preparation::with_limits(anchor, MAX_REPLAY_WORK_UNITS, 1 << 24).unwrap();
    let mut index = declarations(&program, &mut preparation).unwrap();
    assert_eq!(
        preparation
            .append_option_authority(anchor, &mut index)
            .unwrap_err()
            .code,
        "SPX-H006"
    );
    let remaining = preparation.remaining();
    assert_eq!(
        preparation
            .charge(anchor, 0, "no restart")
            .unwrap_err()
            .code,
        "SPX-H006"
    );
    assert_eq!(preparation.remaining(), remaining);
    let provider = Provider {
        program: &program,
        declarations: index,
    };
    assert_eq!(
        Session::from_preparation(&provider, anchor, preparation)
            .err()
            .unwrap()
            .code,
        "SPX-H006"
    );
}

mod full;

mod transitive;
