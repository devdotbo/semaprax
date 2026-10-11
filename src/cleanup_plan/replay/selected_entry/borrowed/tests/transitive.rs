//! Transitive standard readers keep their real checked contracts and scalar
//! decisions; profile acceptance is followed by the complete replay driver.
use super::*;
fn utf8_program() -> crate::hir::ResolvedProgram {
    let source = include_str!("../../../../../../std/data-json-utf8/src/utf8.spx");
    let end = source.find("// Bounded JSON text kernels").unwrap();
    let selected = format!("{}\n@id(\"main\") fn main()->i64 {{0}}", &source[..end]);
    crate::hir::resolve(&crate::check(&selected, "actual-transitive-utf8.spx").unwrap()).unwrap()
}
#[test]
fn actual_transitive_utf8_readers_replay_scalar_and_option_decisions_in_one_session() {
    let program = utf8_program();
    let anchor = &program.functions[0];
    let mut preparation = Preparation::with_limits(anchor, MAX_REPLAY_WORK_UNITS, 1 << 24).unwrap();
    let index = declarations(&program, &mut preparation).unwrap();
    let provider = Provider {
        program: &program,
        declarations: index,
    };
    let mut session = Session::from_preparation(&provider, anchor, preparation).unwrap();
    for function in program.functions.iter().filter(|f| f.name != "main") {
        validate_structure(&program, function).unwrap();
        session.borrowed_replay(function).unwrap();
    }
    session.finish().unwrap();
}
#[test]
fn scalar_match_wrong_literal_type_or_missing_catch_all_cannot_publish_a_profile() {
    for mutant in 0..2 {
        let mut program = utf8_program();
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.name == "is_shortest")
            .unwrap();
        let crate::hir::ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("actual shortest-form body block");
        };
        let crate::hir::ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
            panic!("actual shortest-form scalar decision");
        };
        if mutant == 0 {
            arms[0].pattern = ResolvedMatchPattern::Literal(crate::hir::PatternValue::Uint8(1));
        } else {
            arms.last_mut().unwrap().pattern =
                ResolvedMatchPattern::Literal(crate::hir::PatternValue::Int(9));
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
        let function = program
            .functions
            .iter()
            .find(|f| f.name == "is_shortest")
            .unwrap();
        assert_eq!(
            session.borrowed_replay(function).unwrap_err().code,
            "SPX-H006"
        );
        let remaining = session.metrics().0;
        assert_eq!(
            session.borrowed_replay(anchor).unwrap_err().code,
            "SPX-H006"
        );
        assert_eq!(session.metrics().0, remaining);
    }
}
