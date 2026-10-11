//! Independent ordinary resolution and selected receiving facts; no saved plan
//! or source schema grants a type, field, signature or renewal permission.
use super::*;
use crate::workspace_graph::SourceRecordReplayFixture;
const STRING: &str = r#"module receiving;
@id("app.probe") fn probe()->i64 {
 let key=str"x";let mut text="a";let mut at=0;
 while at<3 {at=at+1;text=string_concat(text,"b");if at==1 {continue;} else {0} if at==3 {return string_len(text)+str_len_bytes(key);} else {0} 0}
 string_len(text)
}
@id("app.main") fn main()->i64 {probe()}
"#;
const VECTOR: &str = r#"module receiving;
@id("app.probe") fn probe()->i64 {
 let key=str"x";let mut rows=vec_with_capacity<i64>(3usize);let mut at=0;
 while at<3 {at=at+1;rows=vec_push<i64>(rows,7);if at==1 {continue;} else {0} if at==3 {return i64_from_usize(vec_len<i64>(rows))+str_len_bytes(key);} else {0} 0}
 0
}
@id("app.main") fn main()->i64 {probe()}
"#;
const RECORD: &str = r#"module receiving;
@id("app.Row") record Row {@id("app.Row.n") n:i64,@id("app.Row.text") text:string,}
@id("app.probe") fn probe(rows:borrow Vec<Row>)->i64 {
 let key=str"x";let mut seen=0;
 for borrow row at index in rows {seen=seen+row.n;if index==0usize {continue;} else {0} let text=row.text;if str_len_bytes(text)==str_len_bytes(key) {return seen;} else {0} 0}
 seen
}
@id("app.main") fn main()->i64 {let first=vec_push<Row>(vec_with_capacity<Row>(2usize),Row{n:3,text:"x"});let rows=vec_push<Row>(first,Row{n:7,text:"x"});probe(rows)}
"#;
fn direct(source: &str) -> ResolvedProgram {
    let ast = crate::check(source, "flat.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let ast = crate::check(&canonical, "flat.spx").unwrap();
    assert_eq!(crate::format::canonical(&ast), canonical);
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&ast, &graph).unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    program
}
fn budget() -> ReplayBudget {
    let mut result = ReplayBudget::new();
    result.physical = Some(Box::new(PhysicalReservation::new(usize::MAX)));
    result
}
fn function(program: &ResolvedProgram) -> &ResolvedFunction {
    program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.probe")
        .unwrap()
}
#[test]
fn string_vec_and_borrowed_record_receiving_replay_match_complete_ordinary_plans() {
    for source in [STRING, VECTOR, RECORD] {
        let ordinary = direct(source);
        let expected = function(&ordinary);
        assert!(expected.cleanup_plan.lexical_control.is_some());
        assert!(nodes(&expected.body).iter().any(|node| node.ty == ResolvedType::Never));
        let fixture = SourceRecordReplayFixture::new(source);
        let retained = fixture
            .functions()
            .iter()
            .find(|f| f.id == expected.id)
            .unwrap();
        assert_eq!(retained.cleanup, expected.cleanup);
        assert_eq!(retained.cleanup_plan, expected.cleanup_plan);
        fixture.with_context(|context| {
            let measured =
                super::super::selected_entry::validate_retained_lexical_with_reservation(
                    context,
                    retained,
                    PhysicalReservation::new(usize::MAX),
                )
                .unwrap_or_else(|error| panic!("selected replay of {source}: {error:?}"));
            let bytes = measured.charged();
            drop(measured);
            let exact = super::super::selected_entry::validate_retained_lexical_with_reservation(
                context,
                retained,
                PhysicalReservation::new(bytes),
            )
            .unwrap();
            assert_eq!(exact.charged(), bytes);
            drop(exact);
            assert_eq!(
                super::super::selected_entry::validate_retained_lexical_with_reservation(
                    context,
                    retained,
                    PhysicalReservation::new(bytes - 1)
                )
                .err()
                .unwrap()
                .code,
                "SPX-H006"
            );
        });
    }
}
fn nodes(root: &ResolvedExpr) -> Vec<&ResolvedExpr> {
    let mut pending = vec![root];
    let mut result = Vec::new();
    while let Some(node) = pending.pop() {
        result.push(node);
        pending.extend(crate::hir::lexical_exit::ChildCursor::new(node));
    }
    result
}
#[test]
fn exact_vec_call_and_field_modes_types_ids_and_elements_remain_closed() {
    let helper = direct(
        r#"module receiving;
@id("app.Row") record Row {@id("app.Row.n") n:i64,@id("app.Row.text") text:string,}
@id("app.weight") fn weight(row:borrow Row)->i64 {row.n}
@id("app.main") fn main()->i64 {let row=Row{n:42,text:"x"};weight(row)}
"#,
    );
    let current_helper = helper
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.weight")
        .unwrap();
    super::super::scalar_replay::validate(&helper, current_helper, &mut budget()).unwrap();
    let projection = nodes(&current_helper.body)
        .into_iter()
        .find(|node| matches!(&node.kind, ResolvedExprKind::Place(place) if !place.projections.is_empty()))
        .expect("actual resolved row.n Field place");
    assert_eq!(super::super::scalar_replay::records::expression_for_test(
        &helper, current_helper, projection, &mut budget()).unwrap(), Some(true));
    for mutation in 0..6 {
        let mut forged = projection.clone();
        let ResolvedExprKind::Place(place) = &mut forged.kind else { unreachable!() };
        match mutation {
            0 => place.projections[0] = crate::hir::PlaceProjection::Field(DeclarationId::new("foreign.field")),
            1 => forged.ownership = OwnershipMode::Own,
            2 => place.root = crate::hir::ValueId::intrinsic_parameter("foreign.parameter", 0),
            3 => forged.ty = ResolvedType::Bool,
            4 => forged.ownership = OwnershipMode::Borrow,
            _ => place.projections.push(place.projections[0].clone()),
        }
        assert_eq!(super::super::scalar_replay::records::expression_for_test(
            &helper, current_helper, &forged, &mut budget()).unwrap(), Some(false),
            "record projection mutation {mutation}");
    }
    let mut wrong_parameter = current_helper.clone();
    wrong_parameter.params[0].ownership = OwnershipMode::Own;
    assert_eq!(super::super::scalar_replay::records::expression_for_test(
        &helper, &wrong_parameter, projection, &mut budget()).unwrap(), Some(false));
    for source in [VECTOR, RECORD] {
        let ordinary = direct(source);
        let current = function(&ordinary);
        let fixture = SourceRecordReplayFixture::new(source);
        fixture.with_context(|context|{
   let call=nodes(&current.body).into_iter().find(|e|matches!(&e.kind,ResolvedExprKind::Call{callee,..} if crate::vec_ops::by_id(callee.as_str()).is_some()));
   if let Some(call)=call{assert!(collections::call(context,current,call,&mut budget()).unwrap());for mutation in 0..4{let mut forged=call.clone();let ResolvedExprKind::Call{type_arguments,args,..}=&mut forged.kind else{unreachable!()};match mutation{0=>type_arguments.clear(),1=>args[0].ownership=OwnershipMode::Shared,2=>args[0].ty=ResolvedType::Bool,_=>forged.ownership=OwnershipMode::Borrow}assert_eq!(collections::call(context,current,&forged,&mut budget()).unwrap_err().code,"SPX-H006","call mutation {mutation}");}}
   if let Some(field)=nodes(&current.body).into_iter().find(|e|matches!(e.kind,ResolvedExprKind::VecFieldRead{..})){collections::field(context,current,field,&mut budget()).unwrap();for mutation in 0..5{let mut forged=field.clone();let ResolvedExprKind::VecFieldRead{field,args,element,..}=&mut forged.kind else{unreachable!()};match mutation{0=>*field=DeclarationId::new("foreign.field"),1=>args[0].ownership=OwnershipMode::Shared,2=>args[1].ty=ResolvedType::Bool,3=>*element=ResolvedType::Bytes,_=>forged.ownership=OwnershipMode::Own}assert_eq!(collections::field(context,current,&forged,&mut budget()).unwrap_err().code,"SPX-H006","field mutation {mutation}");}}
  });
    }
}
#[test]
fn string_read_move_and_renewal_censuses_keep_exact_work_backing_and_sticky_refusal() {
    // Independent key-length witness: prepayment is cumulative, requires no
    // backing, and a one-short receiver cannot retry using fresh work.
    let vector = direct(VECTOR);
    let probe_function = function(&vector);
    let mut probe = budget();
    let backing = probe.physical.as_deref().unwrap().charged();
    collections::prepay(&vector, probe_function, 4, &mut probe).unwrap();
    let short_key_work = MAX_REPLAY_WORK_UNITS - probe.remaining;
    let mut key_bytes = 5usize;
    let mut visits = 0usize;
    for ty in &vector.types {
        visits += 1;
        key_bytes += ty.id.as_str().len() + ty.name.len() + 1;
        for parameter in &ty.type_parameters {
            visits += 1;
            key_bytes += parameter.name.len() + 1;
        }
        if let ResolvedTypeDeclarationKind::Record { fields } = &ty.kind {
            for field in fields {
                visits += 1;
                key_bytes += field.id.as_str().len() + field.name.len() + 1;
            }
        }
    }
    assert!(vector.types.iter().any(|ty| ty.type_parameters.len() > 1));
    assert_eq!(short_key_work, visits + key_bytes * ((vector.declarations.declarations().len() + 1) * 64 + 128));

    let mut long_probe = budget();
    collections::prepay(&vector, probe_function, 256, &mut long_probe).unwrap();
    assert!(MAX_REPLAY_WORK_UNITS - long_probe.remaining > short_key_work);
    assert_eq!(probe.physical.as_deref().unwrap().charged(), backing);
    for (limit, success) in [(short_key_work, true), (short_key_work - 1, false)] {
        let mut current = budget();
        current.remaining = limit;
        assert_eq!(
            collections::prepay(&vector, probe_function, 4, &mut current).is_ok(),
            success
        );
        if !success {
            assert_eq!(current.remaining, 0);
            assert!(collections::prepay(&vector, probe_function, 4, &mut current).is_err());
        }
    }
    let ordinary = direct(STRING);
    let function = function(&ordinary);
    let mut measured = budget();
    let reads = conditions::reads(function, &mut measured).unwrap();
    assert_eq!(
        reads,
        crate::string_ops::conditions::function_reads(function)
    );
    let moves = conditions::moves(function, &mut measured).unwrap();
    assert_eq!(
        moves,
        crate::literal_format::moving_string_operands(function)
    );
    assert_eq!(moves.len(), 1);
    let work = MAX_REPLAY_WORK_UNITS - measured.remaining;
    let bytes = measured.physical.as_deref().unwrap().charged();
    drop(measured);
    for (work_limit, byte_limit, success) in [
        (work, bytes, true),
        (work - 1, bytes, false),
        (work, bytes - 1, false),
    ] {
        let mut current = ReplayBudget::new();
        current.remaining = work_limit;
        current.physical = Some(Box::new(PhysicalReservation::new(byte_limit)));
        let outcome = conditions::reads(function, &mut current)
            .and_then(|_| conditions::moves(function, &mut current));
        assert_eq!(outcome.is_ok(), success);
        if byte_limit < bytes {
            assert!(current.physical.as_deref_mut().unwrap().reserve(0).is_err());
        }
    }
    let mut physical = PhysicalReservation::new(usize::MAX);
    let mut remaining = MAX_REPLAY_WORK_UNITS;
    let sites = crate::cleanup_plan::renewal_profile::bindings_with_reservation_and_work(
        &ordinary,
        function,
        &mut physical,
        &mut |units| {
            if let Some(next) = remaining.checked_sub(units) {
                remaining = next;
                Ok(())
            } else {
                remaining = 0;
                Err(replay_error(function, "test work exhausted"))
            }
        },
    )
    .unwrap();
    let old = crate::cleanup_plan::renewal_profile::bindings(&ordinary, function);
    for node in nodes(&function.body) {
        assert_eq!(
            sites.get(&node.id).map(|b| &b.id),
            old.get(&node.id).map(|b| &b.id)
        );
    }
    assert_eq!(
        sites.appends(),
        &crate::stdin_stream_ops::owner_reopens(function)
    );
    let mut remaining = 0;
    let before = physical.charged();
    assert!(
        crate::cleanup_plan::renewal_profile::bindings_with_reservation_and_work(
            &ordinary,
            function,
            &mut physical,
            &mut |_| {
                remaining = 0;
                Err(replay_error(function, "test sticky work refusal"))
            }
        )
        .is_err()
    );
    assert_eq!(remaining, 0);
    assert!(physical.charged() >= before);
}
