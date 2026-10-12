//! Full trace replay is independent of receiving-only profile acceptance.
use super::*;

// Keep refusal evidence on the stack so the diagnostic is dropped inside the
// allocation observation and cannot falsify the through-Drop live-byte gate.
#[derive(Debug)]
struct Refusal {
    stage: &'static str,
    function: usize,
    message: [u8; 512],
    length: usize,
}
impl std::fmt::Display for Refusal {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            output,
            "{} function {}: {}",
            self.stage,
            self.function,
            std::str::from_utf8(&self.message[..self.length]).expect("UTF-8 prefix")
        )
    }
}

fn run(
    program: &crate::hir::ResolvedProgram,
    work: usize,
    bytes: usize,
) -> (
    Result<(usize, usize), Refusal>,
    crate::test_allocation_observer::Metrics,
) {
    crate::test_allocation_observer::measure(|| {
        let mut stage = "preparation";
        let mut ordinal = 0;
        (|| {
            let anchor = &program.functions[0];
            let mut preparation = Preparation::with_limits(anchor, work, bytes)?;
            stage = "declarations";
            let index = declarations(program, &mut preparation)?;
            let provider = Provider {
                program,
                declarations: index,
            };
            stage = "session";
            let mut session = Session::from_preparation(&provider, anchor, preparation)?;
            stage = "replay";
            for (index, function) in program.functions[..2].iter().enumerate() {
                ordinal = index;
                session.borrowed_replay(function)?;
            }
            let (remaining, charged) = session.metrics();
            let receipt = (work - remaining, charged);
            drop(session);
            drop(provider);
            Ok::<_, Diagnostic>(receipt)
        })()
        .map_err(|diagnostic| {
            assert_eq!(diagnostic.code, "SPX-H006");
            let mut length = diagnostic.message.len().min(512);
            while !diagnostic.message.is_char_boundary(length) {
                length -= 1;
            }
            let mut message = [0; 512];
            message[..length].copy_from_slice(&diagnostic.message.as_bytes()[..length]);
            Refusal {
                stage,
                function: ordinal,
                message,
                length,
            }
        })
    })
}

#[test]
fn actual_option_helpers_full_replay_matches_ordinary_and_metering_through_drop() {
    fn source_pair(
        module: &str,
        source: &str,
        first: &str,
        next: &str,
    ) -> crate::hir::ResolvedProgram {
        let start = source.find(first).unwrap();
        let end = source.find(next).unwrap();
        let selected = format!(
            "module {module};\n{}\n@id(\"main\") fn main()->i64 {{0}}",
            &source[start..end]
        );
        crate::hir::resolve(&crate::check(&selected, "actual-borrowed-full-replay.spx").unwrap())
            .unwrap()
    }
    let lines = source_pair(
        "std.io.lines",
        include_str!("../../../../../../std/io-lines/src/lines.spx"),
        "@id(\"std.io.lines.line-end\")",
        "@id(\"std.io.lines.line-content-len\")",
    );
    let conversion = source_pair(
        "std.data.json.utf8",
        include_str!("../../../../../../std/data-json-utf8/src/utf8.spx"),
        "@id(\"std.data.json.utf8.offset_at\")",
        "@id(\"std.data.json.utf8.sequence_kind\")",
    );
    for program in [program(), lines, conversion] {
        for function in &program.functions[..2] {
            validate_structure(&program, function).unwrap();
        }
        let (wide, allocation) = run(&program, MAX_REPLAY_WORK_UNITS, 1 << 24);
        let (work, bytes) = wide.unwrap_or_else(|refusal| panic!("wide replay refused: {refusal}"));
        assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated <= bytes);
        let (exact, allocation) = run(&program, work, bytes);
        assert_eq!(
            exact.unwrap_or_else(|refusal| panic!("exact replay refused: {refusal}")),
            (work, bytes)
        );
        assert!(!allocation.invalid && allocation.live == 0 && allocation.allocated <= bytes);
        for (work, bytes) in [(work - 1, bytes), (work, bytes - 1)] {
            let (answer, allocation) = run(&program, work, bytes);
            assert!(answer.is_err());
            assert!(!allocation.invalid && allocation.live == 0);
        }
    }
}

#[test]
fn option_profile_cannot_bless_forged_case_or_result_trace_and_failure_is_sticky() {
    // A receiving lease is bound to a private nonzero brand, not duplicated
    // dyn-vtable metadata. Forwarding a genuine lease through another provider
    // does not mint that provider a receiving identity.
    {
        struct Forward<'a>(&'a dyn ReplayContext);
        impl ReplayContext for Forward<'_> {
            fn declarations(&self) -> &crate::hir::DeclarationIndex {
                self.0.declarations()
            }
            fn types(&self) -> crate::hir::items::View<'_, crate::hir::ResolvedTypeDeclaration> {
                self.0.types()
            }
            fn interfaces(&self) -> &[crate::hir::ResolvedInterface] {
                self.0.interfaces()
            }
            fn functions(&self) -> crate::hir::items::View<'_, ResolvedFunction> {
                self.0.functions()
            }
            fn function_templates(&self) -> &[crate::hir::ResolvedFunctionTemplate] {
                self.0.function_templates()
            }
            fn function_instances(&self) -> &[crate::hir::ResolvedFunctionInstance] {
                self.0.function_instances()
            }
            fn borrowed_helper_lease<'a>(
                &'a self,
                function: &ResolvedFunction,
            ) -> Option<crate::cleanup_plan::ReplayBorrowedLease<'a>> {
                self.0.borrowed_helper_lease(function)
            }
        }
        let program = program();
        let anchor = &program.functions[0];
        let mut preparation =
            Preparation::with_limits(anchor, MAX_REPLAY_WORK_UNITS, 1 << 24).unwrap();
        let index = declarations(&program, &mut preparation).unwrap();
        let provider = Provider {
            program: &program,
            declarations: index,
        };
        let mut budget = preparation.budget;
        let context =
            selected_context::borrowed::Context::new(&provider, anchor, &mut budget).unwrap();
        let lease = context.borrowed_helper_lease(anchor).unwrap();
        lease.validate(&context, anchor, &mut budget).unwrap();
        let other =
            selected_context::borrowed::Context::new(&provider, anchor, &mut budget).unwrap();
        assert_eq!(
            lease
                .validate(&other, anchor, &mut budget)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
        let forwarded = Forward(&context);
        assert!(forwarded.borrowed_helper_identity().is_none());
        let forwarded_lease = forwarded.borrowed_helper_lease(anchor).unwrap();
        assert_eq!(
            forwarded_lease
                .validate(&forwarded, anchor, &mut budget)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
        // The unchanged receiving context remains the sole exact brand. This
        // is a low-level identity check; the Session's sticky refusal is still
        // exercised by every original hostile trace below.
        lease.validate(&context, anchor, &mut budget).unwrap();
    }
    for mutation in 0..7 {
        let mut program = program();
        let other_result = program.functions[0].body.id.clone();
        let function = &mut program.functions[1];
        match mutation {
            0 => {
                let case = function
                    .cleanup_plan
                    .edges
                    .iter_mut()
                    .find_map(|edge| {
                        if let EdgeCondition::VariantCase { case, .. } = &mut edge.condition {
                            Some(case)
                        } else {
                            None
                        }
                    })
                    .expect("actual Option decision edge");
                *case = DeclarationId::new("forged.option.case");
            }
            1 => {
                let matches = function
                    .cleanup_plan
                    .edges
                    .iter_mut()
                    .find_map(|edge| {
                        if let EdgeCondition::VariantCase { matches, .. } = &mut edge.condition {
                            Some(matches)
                        } else {
                            None
                        }
                    })
                    .expect("actual Option decision edge");
                *matches = !*matches;
            }
            2 => {
                let source = function
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .find_map(|exit| {
                        if let ExitContinuation::CommitResult { source } = &mut exit.continuation {
                            Some(source)
                        } else {
                            None
                        }
                    })
                    .expect("actual scalar result commit");
                *source = CleanupResultSource::Scalar {
                    expression: other_result,
                };
            }
            3 => {
                let exit = function
                    .cleanup_plan
                    .exits
                    .iter_mut()
                    .find(|exit| {
                        matches!(&exit.continuation, ExitContinuation::CommitResult { .. })
                    })
                    .expect("actual scalar result commit");
                exit.continuation = ExitContinuation::ReturnUnit;
            }
            4 => {
                let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
                    panic!("actual option body");
                };
                let ResolvedExprKind::Match { scrutinee, .. } = &mut tail.kind else {
                    panic!("actual option match");
                };
                let ResolvedExprKind::Call { args, .. } = &mut scrutinee.kind else {
                    panic!("actual byte read");
                };
                let ResolvedExprKind::Place(place) = &mut args[0].kind else {
                    panic!("actual borrowed parameter root");
                };
                place.root.invalidate_fingerprint_for_test();
            }
            5 => {
                let forged = function.body.id.clone();
                let call = function
                    .cleanup_plan
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.transitions)
                    .find_map(|transition| {
                        if let CleanupTransition::CallCommit { call, .. } = transition {
                            Some(call)
                        } else {
                            None
                        }
                    })
                    .expect("actual call commit");
                *call = forged;
            }
            _ => {
                let block = function
                    .cleanup_plan
                    .blocks
                    .iter_mut()
                    .find(|block| {
                        block.transitions.iter().any(|transition| {
                            matches!(transition, CleanupTransition::CallCommit { .. })
                        })
                    })
                    .expect("actual call commit block");
                let duplicate = block
                    .transitions
                    .iter()
                    .find(|transition| matches!(transition, CleanupTransition::CallCommit { .. }))
                    .unwrap()
                    .clone();
                block.transitions.push(duplicate);
            }
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
        session.borrowed_replay(anchor).unwrap();
        assert_eq!(
            session
                .borrowed_replay(&program.functions[1])
                .unwrap_err()
                .code,
            "SPX-H006",
            "trace mutant {mutation}"
        );
        let remaining = session.metrics().0;
        assert_eq!(
            session.borrowed_replay(anchor).unwrap_err().code,
            "SPX-H006"
        );
        assert_eq!(session.metrics().0, remaining);
        assert_eq!(session.finish().err().unwrap().code, "SPX-H006");
    }
}
