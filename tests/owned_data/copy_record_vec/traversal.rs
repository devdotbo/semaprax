use super::runtime;

#[test]
fn copy_rows_optional_index_empty_and_singleton_execute_across_engines() {
    runtime::run(
        "borrowed-row-traversal",
        r#"module app.copy_traversal;
@id("row") record Row { @id("row.value") value:i64, @id("row.other") other:i64, }
@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(2usize);
 let mut observed=0;
 for borrow unused at zero in empty { observed=observed+unused.value+i64_from_usize(zero); 0 }
 let rows=vec_push<Row>(empty,Row{value:17,other:5});
 for borrow row at index in rows { observed=observed+row.value+row.other+i64_from_usize(index); 0 }
 for borrow again in rows { observed=observed+again.value+again.other; 0 }
 if observed==44 && vec_len<Row>(rows)==1usize {42}else{1}
}"#,
        0,
        42,
        false,
    );
}

fn shiftsim_scan_receivers() -> String {
    let live = include_str!("../../../examples/shiftsim-typed-record-successor/src/schedule.spx");
    let old = include_str!(
        "../../../docs/evidence/application-traversal-plan-adoption/schedule_before_traversal.txt"
    );
    fn declaration(source: &str, name: &str) -> String {
        let signature = format!("fn {name}(");
        let start = source.find(&signature).unwrap();
        let id = source[..start].rfind("@id(").unwrap();
        let end = start + source[start..].find("\n}\n").unwrap() + 3;
        source[id..end].to_owned()
    }
    let mut source = String::from(
        r#"module app.scans;
@id("shiftsim.patient_state") record PatientState { @id("shiftsim.patient_state.arrival") arrival:i64, @id("shiftsim.patient_state.assigned") assigned:bool, }
@id("shiftsim.worker_state") record WorkerState { @id("shiftsim.worker_state.free_at") free_at:i64, }
"#,
    );
    for name in ["next_patient", "next_server", "next_time_from_arrivals"] {
        source.push_str(&declaration(live, name));
        let original = declaration(old, name);
        let renamed = original
            .replace(
                &format!("fn {name}("),
                &format!("fn {name}_indexed_control("),
            )
            .replace(
                &format!("shiftsim.schedule.{name}\""),
                &format!("shiftsim.schedule.{name}.indexed_control\""),
            );
        assert_eq!(
            declaration(live, &format!("{name}_indexed_control")),
            renamed,
            "the complete pre-adoption receiver must remain exact"
        );
        source.push_str(&renamed);
    }
    source
}

#[test]
fn shiftsim_live_scans_match_complete_indexed_prefix_empty_full_and_early_overlength() {
    let source = shiftsim_scan_receivers()
        + r#"
@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<PatientState>(3usize);
 let empty_workers=vec_with_capacity<WorkerState>(3usize);
 let arrivals=vec_push<i64>(vec_with_capacity<i64>(1usize),9);
 let states=vec_push<PatientState>(vec_push<PatientState>(vec_push<PatientState>(vec_with_capacity<PatientState>(3usize),PatientState{arrival:8,assigned:false}),PatientState{arrival:2,assigned:false}),PatientState{arrival:1,assigned:false});
 let workers=vec_push<WorkerState>(vec_push<WorkerState>(vec_push<WorkerState>(vec_with_capacity<WorkerState>(3usize),WorkerState{free_at:8}),WorkerState{free_at:2}),WorkerState{free_at:1});
 let mut good=next_patient(empty,0usize,3)==next_patient_indexed_control(empty,0usize,3) && next_server(empty_workers,0usize,3)==next_server_indexed_control(empty_workers,0usize,3);
 let mut count=0usize;
 while count<=3usize {
  good=good && next_patient(states,count,3)==next_patient_indexed_control(states,count,3) && next_server(workers,count,3)==next_server_indexed_control(workers,count,3) && next_time_from_arrivals(arrivals,0usize,1usize,workers,count,0)==next_time_from_arrivals_indexed_control(arrivals,0usize,1usize,workers,count,0);
  good=good && next_patient(states,count,0)==next_patient_indexed_control(states,count,0) && next_patient(states,count,1)==next_patient_indexed_control(states,count,1) && next_patient(states,count,10)==next_patient_indexed_control(states,count,10);
  good=good && next_server(workers,count,0)==next_server_indexed_control(workers,count,0) && next_server(workers,count,1)==next_server_indexed_control(workers,count,1) && next_server(workers,count,10)==next_server_indexed_control(workers,count,10);
  count=count+1usize; 0
 }
 good=good && next_patient(states,4usize,3)==next_patient_indexed_control(states,4usize,3) && next_server(workers,4usize,3)==next_server_indexed_control(workers,4usize,3);
 if good {42}else{1}
}
"#;
    runtime::run("shiftsim-prefix-scans", &source, 0, 42, false);
}

#[test]
fn shiftsim_live_and_complete_indexed_scans_keep_late_overlength_vec_refusal() {
    for name in [
        "next_patient",
        "next_patient_indexed_control",
        "next_server",
        "next_server_indexed_control",
        "next_time_from_arrivals",
        "next_time_from_arrivals_indexed_control",
    ] {
        let call = if name.starts_with("next_patient") {
            format!("i64_from_usize({name}(patients,2usize,0))")
        } else if name.starts_with("next_server") {
            format!("i64_from_usize({name}(workers,2usize,0))")
        } else {
            format!("{name}(arrivals,1usize,1usize,workers,2usize,0)")
        };
        let source = shiftsim_scan_receivers()
            + &format!(
                r#"
@id("app.main") fn main()->i64 {{
 let patients=vec_push<PatientState>(vec_with_capacity<PatientState>(1usize),PatientState{{arrival:8,assigned:false}});
 let workers=vec_push<WorkerState>(vec_with_capacity<WorkerState>(1usize),WorkerState{{free_at:8}});
 let arrivals=vec_push<i64>(vec_with_capacity<i64>(1usize),9);
 {call}
}}
"#
            );
        runtime::run(name, &source, 2, 0, false);
    }
}
