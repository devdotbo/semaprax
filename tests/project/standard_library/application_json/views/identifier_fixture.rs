// Shared exact source fixture for ordinary application and persisted-cache owners.
pub(crate) const SCHEMA: &str = r#"module consumer.schema;
@id("application.patient") record Patient {
 @id("application.patient.id") id:string,
 @id("application.patient.arrival") arrival:i64,
 @id("application.patient.service") service:i64,
 @id("application.patient.priority") priority:i64,
 @id("application.patient.deadline") deadline:i64,
}
@id("application.request") record Request {
 @id("application.request.servers") servers:Vec<string>,
 @id("application.request.patients") patients:Vec<Patient>,
}
@id("consumer.schema.anchor") fn schema_anchor()->i64 {0}
"#;

fn array(bytes: &[u8]) -> String {
    format!(
        "[{}]",
        bytes
            .iter()
            .map(|byte| format!("{byte}u8"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
fn offset(input: &[u8], token: &[u8]) -> usize {
    input
        .windows(token.len())
        .rposition(|window| window == token)
        .unwrap()
}
pub(crate) fn app() -> String {
    let mut out = r#"module consumer.app;
use type @id("application.request.json.request-result") from consumer.schema as Outcome;
use type @id("application.patient.json.view") from consumer.schema as PatientView;
use type @id("application.request.json.identifier-span") from consumer.schema as ServerSpan;
use function @id("application.request.json.request.decode") from consumer.schema as decode;
use function @id("application.patient.json.identifier-fingerprint") from consumer.schema as fingerprint;
@id("consumer.error") fn error(input:borrow Slice<u8>,expected:i64,at:usize,target:i64)->i64 {
let outcome=decode(input);match own outcome{Outcome::Decoded{servers,patients}=>1,Outcome::Error{code,offset,field}=>if code==expected && offset==at && field==target{0}else{1},}
}
@id("consumer.main") fn main()->i64 {
let first=[34u8,66u8,65u8,65u8,34u8];let second=[34u8,65u8,67u8,79u8,34u8];
let collision=fingerprint(array_as_slice(first),0usize,5usize)==fingerprint(array_as_slice(second),0usize,5usize);
let mut failures=if collision{0}else{1};
"#.to_owned();
    let valid = br#"{"servers":["BAA","ACO"],"patients":[{"id":"BAA","arrival":0,"service":1,"priority":0,"deadline":2},{"id":"ACO","arrival":0,"service":1,"priority":0,"deadline":2}]}"#;
    out.push_str(&format!("let valid={};let outcome=decode(array_as_slice(valid));failures=failures+match own outcome{{Outcome::Error{{code,offset,field}}=>1,Outcome::Decoded{{servers,patients}}=>if vec_len<ServerSpan>(servers)==2usize && vec_len<PatientView>(patients)==2usize{{0}}else{{1}},}};\n", array(valid)));
    let rows = br#"{"servers":["S"],"patients":[{"id":"BAA","arrival":0,"service":1,"priority":0,"deadline":2},{"id":"ACO","arrival":0,"service":1,"priority":0,"deadline":2},{"id":"\u0042AA","arrival":0,"service":1,"priority":0,"deadline":2}]}"#;
    let cases: Vec<(&[u8], i64, usize, i64)> = vec![
        (
            br#"{"servers":["BAA","\u0042AA"],"patients":[]}"#,
            10,
            offset(
                br#"{"servers":["BAA","\u0042AA"],"patients":[]}"#,
                br#""\u0042AA""#,
            ),
            1,
        ),
        (rows, 10, offset(rows, br#""\u0042AA""#), 2),
        (br#"{"servers":["\u0000"],"patients":[]}"#, 6, 12, 1),
        (br#"{"servers":["\ud83d\ude00"],"patients":[]}"#, 6, 12, 1),
        ("{\"servers\":[\"é\"],\"patients\":[]}".as_bytes(), 6, 12, 1),
        (
            "{\"servers\":[\"😀\"],\"patients\":[]}".as_bytes(),
            6,
            12,
            1,
        ),
    ];
    for (index, (input, code, at, field)) in cases.into_iter().enumerate() {
        out.push_str(&format!("let bad_{index}={};failures=failures+error(array_as_slice(bad_{index}),{code},{at}usize,{field});\n",array(input)));
    }
    // Full grammar validation still outranks the earlier duplicate/schema fault.
    let malformed = br#"{"servers":["BAA","\u0042AA"],"patients":[]}?"#;
    out.push_str(&format!("let late={};failures=failures+error(array_as_slice(late),1,{}usize,0);if failures==0{{728}}else{{0-failures}}\n}}",array(malformed),malformed.len()-1));
    out
}
