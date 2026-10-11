//! Frozen pairwise oracle versus sorted, separately owned validation IDs.
use super::super::public_example::{compile_native, execute};
use super::{REQUEST_SCHEMA, SCHEDULE};
use semaprax::{codegen, format, graph, interpreter};

const ORIGINAL: &str = include_str!("shiftsim_id_validation/original_helper.txt");
fn section<'a>(source: &'a str, first: &str, next: &str) -> &'a str {
    let begin = source.find(first).unwrap();
    let end = source[begin..].find(next).unwrap() + begin;
    &source[begin..end]
}
fn source(count: usize, identifiers: &str, expected: bool) -> String {
    let patient = section(
        REQUEST_SCHEMA,
        "@id(\"shiftsim.patient\")",
        "@id(\"shiftsim.request\")",
    );
    let sorted = section(
        SCHEDULE,
        "@id(\"shiftsim.schedule.unique_identifiers\")",
        "@id(\"shiftsim.schedule.unique_patient_ids\")",
    );
    let adopted = section(
        SCHEDULE,
        "@id(\"shiftsim.schedule.unique_patient_ids\")",
        "@id(\"shiftsim.schedule.patient_fields_invalid\")",
    );
    let old = ORIGINAL
        .replace(
            "shiftsim.schedule.unique_patient_ids",
            "shiftsim.oracle.unique_patient_ids",
        )
        .replace("fn unique_patient_ids(", "fn original_unique_patient_ids(");
    format!(
        r#"module shiftsim.ids;
{patient}
{sorted}
{adopted}
{old}
@id("app.main") fn main()->i64 {{
 let mut people=vec_with_capacity<Patient>({count}usize);
 let mut index=0usize;
 while index<{count}usize {{
  let identifier={identifiers};
  people=vec_push<Patient>(people,Patient{{priority:1,arrival:0,id:identifier,service:1,deadline:1}});
  index=index+1usize;
  0
 }}
 let before=original_unique_patient_ids(people);
 let after=unique_patient_ids(people);
 if before=={expected} && after=={expected} {{0}}else{{1}}
}}
"#
    )
}
#[test]
fn sorted_patient_id_validation_matches_frozen_pairwise_bytes_and_256_bound() {
    assert!(ORIGINAL.contains("previous_index < index"));
    assert!(!SCHEDULE.contains("previous_index < index"));
    let root = crate::standard_library::temporary("shiftsim-sorted-id-control");
    std::fs::create_dir_all(&root).unwrap();
    for (name, count, ids, expected) in [
        ("empty", 0, "string_format(\"P{}\",index)", true),
        ("one", 1, "\"P0\"", true),
        ("duplicate", 2, "\"P0\"", false),
        (
            "unicode-nul",
            2,
            "if index==0usize {\"é\\0\"}else{\"é\"}",
            true,
        ),
        ("unicode-nul-duplicate", 2, "\"é\\0\"", false),
        ("max-distinct", 256, "string_format(\"P{}\",index)", true),
        (
            "max-distant-duplicate",
            256,
            "if index==255usize {\"P0\"}else{string_format(\"P{}\",index)}",
            false,
        ),
    ] {
        let source = source(count, ids, expected);
        let path = root.join("app.spx");
        let ast = semaprax::check(&source, &path).unwrap_or_else(|d| panic!("{name}: {d:?}"));
        let canonical = format::canonical(&ast);
        let reparsed = semaprax::check(&canonical, &path).unwrap();
        assert_eq!(format::canonical(&reparsed), canonical);
        let projection = graph::to_json(&reparsed).unwrap();
        graph::verify_json(&reparsed, &projection).unwrap();
        std::fs::write(&path, &canonical).unwrap();
        for _ in 0..2 {
            let result =
                interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
            assert!(result.returned, "{name}: {}", result.envelope);
            interpreter::verify_envelope(&result.envelope).unwrap();
            let wire: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            assert_eq!(wire["payload"]["outcome"]["value"], "0", "{name}");
        }
        let c = codegen::emit_c(&reparsed).unwrap();
        for optimization in ["-O0", "-O2"] {
            let executable = compile_native(&c, &root, optimization);
            for _ in 0..2 {
                let output = execute(&executable, &[]);
                assert!(output.status.success(), "{name} {optimization}: {output:?}");
                assert_eq!(output.stdout, b"0\n", "{name}");
                assert!(output.stderr.is_empty(), "{name}: {output:?}");
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn sorted_arrival_cursor_matches_frozen_event_scans_and_host_count_oracle() {
    let old = include_str!("shiftsim_id_validation/original_event_helpers.txt");
    assert!(old.contains("while arrival_index < patient_count"));
    let model = super::MODEL;
    let states = section(
        model,
        "@id(\"shiftsim.patient_state\")",
        "@id(\"shiftsim.schedule_result\")",
    );
    let fast = section(
        SCHEDULE,
        "@id(\"shiftsim.schedule.advance_arrivals\")",
        "@id(\"shiftsim.schedule.run\")",
    );
    let dispatch = include_str!("shiftsim_id_validation/original_dispatch_helpers.txt");
    let mut source = format!("module shiftsim.events;\n{states}\n{old}\n{dispatch}\n{fast}\n");
    for (case, (count, time)) in [
        (0usize, 0i64),
        (1, 0),
        (256, 0),
        (256, 1),
        (256, 6),
        (256, 1_000_000),
    ]
    .into_iter()
    .enumerate()
    {
        let arrived = (0..count).filter(|i| (i % 7) as i64 <= time).count();
        let dispatched = (0..count)
            .filter(|i| (i % 7) as i64 <= time && i % 3 == 0)
            .count();
        let waiting = arrived - dispatched;
        let next = (0..count)
            .map(|i| (i % 7) as i64)
            .chain([0, 3, 5])
            .filter(|at| *at > time)
            .min()
            .unwrap_or(i64::MAX);
        source.push_str(&format!(r#"
@id("events.case{case}") fn case{case}()->i64 {{
 let mut patients=vec_with_capacity<PatientState>({count}usize);
 let mut arrivals=vec_with_capacity<i64>({count}usize);
 let mut index=0usize;
 let mut dispatched=0usize;
 while index<{count}usize {{
  let arrival=i64_from_usize(index % 7usize);
  let assigned=arrival<={time} && index % 3usize==0usize;
  patients=vec_push<PatientState>(patients,PatientState{{arrival:arrival,assigned:assigned}});
  arrivals=vec_push<i64>(arrivals,arrival);
  if assigned {{dispatched=dispatched+1usize;}}
  index=index+1usize;
  0
 }}
 let ordered=vec_sort<i64>(arrivals);
 let workers0=vec_with_capacity<WorkerState>(3usize);
 let workers1=vec_push<WorkerState>(workers0,WorkerState{{free_at:0}});
 let workers2=vec_push<WorkerState>(workers1,WorkerState{{free_at:3}});
 let workers=vec_push<WorkerState>(workers2,WorkerState{{free_at:5}});
 let before_patient=next_patient(patients,{count}usize,{time});
 let before_server=next_server(workers,3usize,{time});
 let before_selected=before_patient!={count}usize && before_server!=3usize;
 let after_server=next_server(workers,3usize,{time});
 let after_selected=if after_server==3usize {{false}}else{{let after_patient=next_patient(patients,{count}usize,{time}); after_patient!={count}usize && after_patient==before_patient && after_server==before_server}};
 let blocked0=vec_with_capacity<WorkerState>(1usize);
 let blocked=vec_push<WorkerState>(blocked0,WorkerState{{free_at:{time}+1}});
 let blocked_patient=next_patient(patients,{count}usize,{time});
 let blocked_server=next_server(blocked,1usize,{time});
 let before_blocked=blocked_patient!={count}usize && blocked_server!=1usize;
 let after_blocked=if next_server(blocked,1usize,{time})==1usize {{false}}else{{next_patient(patients,{count}usize,{time})!={count}usize}};
 let arrived=advance_arrivals(ordered,{count}usize,0usize,{time});
 // Reusing the same cursor is idempotent, and advancing from time zero
 // preserves all equal-time arrivals before dispatch.
 let initial=advance_arrivals(ordered,{count}usize,0usize,0);
 let advanced=advance_arrivals(ordered,{count}usize,initial,{time});
 let again=advance_arrivals(ordered,{count}usize,arrived,{time});
 let before_wait=waiting_count(patients,{count}usize,{time});
 let after_wait=i64_from_usize(arrived-dispatched);
 let before_next=next_time(patients,{count}usize,workers,3usize,{time});
 let after_next=next_time_from_arrivals(ordered,arrived,{count}usize,workers,3usize,{time});
 if before_selected==after_selected && !before_blocked && !after_blocked && arrived=={arrived}usize && advanced==arrived && again==arrived && before_wait=={waiting} && after_wait=={waiting} && before_next=={next} && after_next=={next} {{0}}else{{1}}
}}
"#));
    }
    source.push_str("@id(\"app.main\") fn main()->i64 {if case0()==0 && case1()==0 && case2()==0 && case3()==0 && case4()==0 && case5()==0 {0}else{1}}\n");
    let root = crate::standard_library::temporary("shiftsim-arrival-cursor-control");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("app.spx");
    let ast = semaprax::check(&source, &path).unwrap();
    let canonical = format::canonical(&ast);
    let reparsed = semaprax::check(&canonical, &path).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    let projection = graph::to_json(&reparsed).unwrap();
    graph::verify_json(&reparsed, &projection).unwrap();
    std::fs::write(&path, &canonical).unwrap();
    for _ in 0..2 {
        let result = interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
        assert!(result.returned, "{}", result.envelope);
        interpreter::verify_envelope(&result.envelope).unwrap();
        let wire: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        assert_eq!(wire["payload"]["outcome"]["value"], "0");
    }
    let c = codegen::emit_c(&reparsed).unwrap();
    for optimization in ["-O0", "-O2"] {
        let executable = compile_native(&c, &root, optimization);
        for _ in 0..2 {
            let output = execute(&executable, &[]);
            assert!(output.status.success(), "{optimization}: {output:?}");
            assert_eq!(output.stdout, b"0\n");
            assert!(output.stderr.is_empty(), "{output:?}");
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}
