//! Provisional owner reads never publish or transfer a result before ensures.
use super::*;
use crate::cleanup_plan::{ContractPhase, ExitContinuation, StatusProducer, StorageId};

const SOURCE: &str = r#"module test.provisional_views;
@id("choose") fn choose(input:borrow Slice<u8>)->Bytes
 requires byte_len(input)>=0usize
 ensures byte_len(bytes_as_slice(result))==2usize {
 let outer=bytes_copy(input);
 if byte_len(input)==2usize {return outer;};
 return bytes_zeroed(3usize);
}
@id("text") fn text()->string
 ensures byte_len(str_as_bytes(string_as_str(result)))==4usize {return "aé\u{0}";}
@id("main") fn main()->i64 {
 let input=[1u8,2u8];let value=choose(array_as_slice(input));
 let word=text();
 if byte_len(bytes_as_slice(value))==2usize && string_len(word)==4 {29}else{0}
}
"#;
fn fixture() -> ResolvedProgram {
    let ast = crate::check(SOURCE, std::path::Path::new("provisional-views.spx")).unwrap();
    let program = resolve(&ast).unwrap();
    validate(&program).unwrap();
    program
}
fn selected(program: &ResolvedProgram) -> &ResolvedFunction {
    program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "choose")
        .unwrap()
}
fn selected_mut(program: &mut ResolvedProgram) -> &mut ResolvedFunction {
    program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "choose")
        .unwrap()
}
fn length_operand(expression: &mut ResolvedExpr) -> &mut ResolvedExpr {
    let ResolvedExprKind::Binary { left, .. } = &mut expression.kind else {
        panic!("comparison")
    };
    let ResolvedExprKind::Call { args, .. } = &mut left.kind else {
        panic!("byte_len")
    };
    &mut args[0]
}
fn refusal(source: &str, code: &str) {
    let errors = crate::check(source, std::path::Path::new("provisional-refusal.spx")).unwrap_err();
    assert!(errors.iter().any(|d| d.code == code), "{errors:?}");
}
#[test]
fn exact_bytes_and_string_postcondition_loans_replay_with_cached_cleanup() {
    let mut program = fixture();
    for function in program
        .functions
        .iter_mut()
        .filter(|f| matches!(f.id.as_str(), "choose" | "text"))
    {
        let loans: Vec<_> = function
            .loan_plan
            .loans
            .iter()
            .filter(|l| l.origin.root == function.result_id)
            .collect();
        assert!(!loans.is_empty());
        assert!(loans.iter().all(|l| l.origin.projections.is_empty()
            && !l.ends.is_empty()
            && !l.end_edges.is_empty()));
        let cleanup = crate::cache_codec::encode(&function.cleanup_plan).unwrap();
        function.cleanup_plan = crate::cache_codec::decode(&cleanup).unwrap();
        assert_eq!(
            crate::cache_codec::encode(&function.cleanup_plan).unwrap(),
            cleanup
        );
        let loan = crate::cache_codec::encode(&function.loan_plan).unwrap();
        function.loan_plan = crate::cache_codec::decode(&loan).unwrap();
        assert_eq!(
            crate::cache_codec::encode(&function.loan_plan).unwrap(),
            loan
        );
    }
    validate(&program).unwrap();
    let generated = crate::codegen::emit_hir_c(&program).unwrap();
    assert_eq!(generated, crate::codegen::emit_hir_c(&program).unwrap());
    crate::wasm::emit_resolved_module(&program).unwrap();
}
#[test]
fn result_is_absent_before_body_completion_and_cannot_transfer_in_contracts() {
    refusal(
        &SOURCE.replace(
            "requires byte_len(input)>=0usize",
            "requires byte_len(bytes_as_slice(result))==2usize",
        ),
        "SPX-T201",
    );
    refusal(
        &SOURCE.replace(
            "let outer=bytes_copy(input);",
            "let outer=bytes_copy(bytes_as_slice(result));",
        ),
        "SPX-T201",
    );
    let transfer=SOURCE.replace("ensures byte_len(bytes_as_slice(result))==2usize","ensures consume(result)")+
        "\n@id(\"consume\") fn consume(value:own Bytes)->bool {byte_len(bytes_as_slice(value))==2usize}\n";
    refusal(&transfer, "SPX-O105");
    refusal(
        &SOURCE.replace("bytes_as_slice(result)", "string_as_str(result)"),
        "SPX-T263",
    );
    let projected = r#"module t;
@id("packet") record Packet{@id("packet.bytes") bytes:Bytes,}
@id("make") fn make()->Packet ensures byte_len(bytes_as_slice(result.bytes))==1usize {
 Packet{bytes:bytes_zeroed(1usize)}
}
@id("main") fn main()->i64 {0}
"#;
    refusal(projected, "SPX-T266");
    let escape = r#"module escape;
@id("escape") fn escape(value:own Bytes)->Slice<u8>{bytes_as_slice(value)}
@id("main") fn main()->i64{0}
"#;
    refusal(escape, "SPX-T264");
}
#[test]
fn forged_result_root_phase_type_and_projection_have_no_borrow_authority() {
    let baseline = fixture();
    let view_kind = length_operand(&mut selected_mut(&mut baseline.clone()).ensures[0])
        .kind
        .clone();
    let mut body = baseline.clone();
    let ResolvedExprKind::Block { statements, .. } = &mut selected_mut(&mut body).body.kind else {
        panic!("body block")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("owned copy")
    };
    let ResolvedExprKind::Call { args, .. } = &mut value.kind else {
        panic!("bytes_copy")
    };
    args[0].kind = view_kind.clone();
    assert_eq!(validate(&body).unwrap_err().code, "SPX-H006");
    let mut requires = baseline.clone();
    length_operand(&mut selected_mut(&mut requires).requires[0]).kind = view_kind;
    assert_eq!(validate(&requires).unwrap_err().code, "SPX-H006");
    let foreign = baseline
        .functions
        .iter()
        .find(|f| f.id.as_str() == "text")
        .unwrap()
        .result_id
        .clone();
    for change in 0..4 {
        let mut changed = baseline.clone();
        let operand = length_operand(&mut selected_mut(&mut changed).ensures[0]);
        let ResolvedExprKind::BorrowPlace { operation, place } = &mut operand.kind else {
            panic!("view")
        };
        match change {
            0 => place.root = foreign.clone(),
            1 => place
                .projections
                .push(PlaceProjection::Field(DeclarationId::new("foreign.field"))),
            2 => *operation = DeclarationId::new(crate::byte_ops::STRING_AS_STR_ID),
            _ => operand.ownership = OwnershipMode::Own,
        }
        assert_eq!(validate(&changed).unwrap_err().code, "SPX-H006");
    }
}
#[test]
fn provisional_storage_loan_and_failure_cleanup_cannot_be_substituted() {
    let baseline = fixture();
    let function = selected(&baseline);
    let failures: Vec<_> = function
        .cleanup_plan
        .exits
        .iter()
        .enumerate()
        .filter_map(|(index, exit)| {
            let ExitContinuation::ReturnFailure { source } = &exit.continuation else {
                return None;
            };
            function
                .cleanup_plan
                .status_sources
                .iter()
                .any(|s| {
                    s.id == *source
                        && matches!(
                            s.producer,
                            StatusProducer::ContractFalse {
                                phase: ContractPhase::Ensures,
                                ..
                            }
                        )
                })
                .then_some(index)
        })
        .collect();
    assert!(!failures.is_empty());
    for index in failures {
        assert!(function.cleanup_plan.exits[index]
            .finalize_in_order
            .iter()
            .any(|a| a.source.storage == StorageId::ProvisionalResult));
        let mut changed = baseline.clone();
        selected_mut(&mut changed).cleanup_plan.exits[index]
            .finalize_in_order
            .retain(|a| a.source.storage != StorageId::ProvisionalResult);
        assert_eq!(validate(&changed).unwrap_err().code, "SPX-H006");
    }
    let mut destination = baseline.clone();
    let plan = &mut selected_mut(&mut destination).cleanup_plan;
    let provisional = plan
        .slots
        .iter_mut()
        .find(|slot| slot.storage == StorageId::ProvisionalResult)
        .unwrap();
    provisional.storage = StorageId::Value(ValueId::new("foreign.destination"));
    assert_eq!(validate(&destination).unwrap_err().code, "SPX-H006");
    let mut absent = baseline.clone();
    let result = selected(&absent).result_id.clone();
    selected_mut(&mut absent)
        .loan_plan
        .loans
        .retain(|l| l.origin.root != result);
    assert_eq!(validate(&absent).unwrap_err().code, "SPX-H006");
    let mut foreign = baseline.clone();
    selected_mut(&mut foreign).result_id = ValueId::new("foreign.result");
    assert_eq!(validate(&foreign).unwrap_err().code, "SPX-H006");
}

#[test]
fn immutable_contract_view_alias_derives_exact_provisional_byte_provenance() {
    let source = SOURCE.replace(
        "ensures byte_len(bytes_as_slice(result))==2usize",
        "ensures {let view=bytes_as_slice(result);byte_len(view)==2usize}",
    );
    let ast = crate::check(&source, std::path::Path::new("provisional-alias.spx")).unwrap();
    let canonical = crate::format::canonical(&ast);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "canonical-alias.spx").unwrap())
    );
    let baseline = resolve(&ast).unwrap();
    validate(&baseline).unwrap();
    let result = selected(&baseline).result_id.clone();
    let (binding, facts) = baseline
        .declarations
        .byte_slice_roots
        .iter()
        .find(|(_, facts)| facts.root == result)
        .unwrap();
    assert_eq!(facts.root_kind, ByteSliceRootKind::OwnedBytes);
    assert_eq!(facts.projected_type, ResolvedType::Bytes);
    assert_eq!(facts.root_length, ByteSliceExtent::ValueLength);
    assert_eq!(facts.offset, ByteSliceExtent::Constant(0));
    assert_eq!(facts.length, ByteSliceExtent::ValueLength);
    assert!(facts.producer.is_some() && facts.projections.is_empty());
    let graph = crate::graph::to_json(&ast).unwrap();
    assert_eq!(graph, crate::graph::to_json(&ast).unwrap());
    assert!(graph.contains("\"kind\":\"loan_plan\""));
    for change in 0..4 {
        let mut forged = baseline.clone();
        let facts = forged
            .declarations
            .byte_slice_roots
            .get_mut(binding)
            .unwrap();
        match change {
            0 => facts.root = ValueId::new("foreign.provisional"),
            1 => facts.root_kind = ByteSliceRootKind::FunctionParameter,
            2 => facts.producer = None,
            _ => facts.projected_type = ResolvedType::String,
        }
        assert_eq!(validate(&forged).unwrap_err().code, "SPX-H006");
    }
}
