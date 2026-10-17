from pathlib import Path
import json,hashlib,subprocess
R=Path('/Volumes/Semaprax/worktrees/wasm-custody-compile-repair-20261017');HEAD='72936084939be5d80de518aa6c261ab0c3cc8f36';MAIN='6d101efd8c255330a3fe5fb0c06e058c6f8c085f';REL='docs/evidence/main6d-closed-controller-acquisition-20261017';D=R/REL;D.mkdir(parents=True,exist_ok=True);H=lambda b:hashlib.sha256(b).hexdigest();G=lambda rev,p:subprocess.check_output(['git','show',rev+':'+p],cwd=R)
def archive(b,kind):
 p=REL+'/'+kind+'/'+H(b)+'.txt';(R/p).parent.mkdir(parents=True,exist_ok=True);(R/p).write_bytes(b);return dict(path=p,length=len(b),sha256=H(b))
control='docs/evidence/status-inline-receipts-v1/sources.json';controlraw=G(MAIN,control);assert(R/control).read_bytes()==controlraw;proofs=json.loads(controlraw);assert len(proofs)==7;rows=[]
for proof in proofs:
 source=proof['path'];current=G(HEAD,source);assert(R/source).read_bytes()==current
 incoming=G(MAIN,source);before=G(MAIN,proof['archive']);assert H(incoming)==proof['after_sha256']and H(before)==proof['before_sha256'];assert(R/proof['archive']).read_bytes()==before
 rows.append(dict(source=source,current=archive(current,'current'),incoming=archive(incoming,'incoming'),before=dict(path=proof['archive'],length=len(before),sha256=H(before))))
packet=dict(schema='semaprax.main6d-closed-controller-acquisition.v1',main_head=MAIN,current_source_stage=dict(kind='immutable-git',head=HEAD),control=dict(path=control,length=len(controlraw),sha256=H(controlraw)),rows=rows);raw=(json.dumps(packet,indent=2)+'\n').encode();assert len(raw)<=262144;(D/'inputs.json').write_bytes(raw)
helper='src/source_reflows/semantic_receivers/status_inline.rs';old=G(HEAD,helper);assert(R/helper).read_bytes()==old;(D/'status-inline.whole-before.rs.txt').write_bytes(old)
compiled='\n'.join('        '+json.dumps(row['source'])+' => Some(::std::include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/'+row['source']+'"))),'for row in rows)
append='''
// Distinct closed Main6d acquisition; the complete incoming controller above is unchanged.
const MAIN6D_CAP: usize = 262_144;
fn main6d_error(message: &'static str) -> ::std::io::Error {
    ::std::io::Error::new(::std::io::ErrorKind::InvalidData, message)
}
fn main6d_held(path: &::std::path::Path, length: usize, expected: &str) -> ::std::io::Result<String> {
    if length > MAIN6D_CAP { return Err(main6d_error("Main6d exact source bound")); }
    crate::source_reflows_merge::main_e177_inputs::main_e177_file_raw(path, length, expected).map_err(main6d_error)
}
fn main6d_compiled(source: &str) -> Option<&'static str> {
    match source {
__COMPILED__
        _ => None,
    }
}
fn main6d_packet() -> ::std::io::Result<serde_json::Value> {
    let data=::std::include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/__REL__/inputs.json"));
    assert_eq!(data.len(), __LENGTH__);
    assert_eq!(format!("{:x}",crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(data.as_bytes()))), "__SHA__");
    let root=::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(main6d_held(&root.join("__REL__/inputs.json"),__LENGTH__,"__SHA__")?,data,"whole physical and compiled Main6d packet");
    let packet:serde_json::Value=serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"],"semaprax.main6d-closed-controller-acquisition.v1");
    assert_eq!(packet["main_head"],"__MAIN__");
    assert_eq!(packet["rows"].as_array().unwrap().len(),7);
    Ok(packet)
}
fn main6d_read(path: &::std::path::Path) -> ::std::io::Result<String> {
    let root=::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative=path.strip_prefix(root).map_err(|_|main6d_error("foreign Main6d root"))?;
    if !relative.components().all(|part|matches!(part,::std::path::Component::Normal(_))) {return Err(main6d_error("foreign Main6d spelling"));}
    let source=relative.to_str().ok_or_else(||main6d_error("nonUTF8 Main6d path"))?;
    if source=="__CONTROL__" {
        let text=main6d_held(path,__CONTROL_LENGTH__,"__CONTROL_SHA__")?;
        assert_eq!(text,::std::include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/__CONTROL__")),"whole immutable Main6d control table");
        return Ok(text);
    }
    let packet=main6d_packet()?;let rows=packet["rows"].as_array().unwrap();
    let before:Vec<_>=rows.iter().filter(|row|row["before"]["path"]==source).collect();
    if before.len()==1 {
        let before=&before[0]["before"];
        return main6d_held(path,before["length"].as_u64().unwrap()as usize,before["sha256"].as_str().unwrap());
    }
    let found:Vec<_>=rows.iter().filter(|row|row["source"]==source).collect();
    if found.len()!=1 {return Err(main6d_error("unknown or duplicate Main6d source"));}
    let row=found[0];let current=&row["current"];
    let actual=main6d_held(path,current["length"].as_u64().unwrap()as usize,current["sha256"].as_str().unwrap())?;
    assert_eq!(Some(actual.as_str()),main6d_compiled(source),"whole actual and compiled Main6d source");
    assert_eq!(actual,main6d_held(&root.join(current["path"].as_str().unwrap()),current["length"].as_u64().unwrap()as usize,current["sha256"].as_str().unwrap())?,"whole current source archive");
    let incoming=&row["incoming"];
    main6d_held(&root.join(incoming["path"].as_str().unwrap()),incoming["length"].as_u64().unwrap()as usize,incoming["sha256"].as_str().unwrap())
}
mod std {
    pub use ::std::{collections,panic};
    pub mod fs {
        pub fn read_to_string(path: impl AsRef<::std::path::Path>) -> ::std::io::Result<String> {
            super::super::main6d_read(path.as_ref())
        }
    }
}
'''
for a,b in {'__COMPILED__':compiled,'__REL__':REL,'__LENGTH__':str(len(raw)),'__SHA__':H(raw),'__MAIN__':MAIN,'__CONTROL__':control,'__CONTROL_LENGTH__':str(len(controlraw)),'__CONTROL_SHA__':H(controlraw)}.items():append=append.replace(a,b)
new=old+append.encode();assert len(new.splitlines())<=1500;(R/helper).write_bytes(new)
(D/'whole-helper-inverse.json').write_text(json.dumps(dict(base=HEAD,source=helper,before=dict(length=len(old),sha256=H(old),archive=REL+'/status-inline.whole-before.rs.txt'),after=dict(length=len(new),sha256=H(new)),edits=[dict(before='',after=append,offset=len(old),count=1)],whole_controller_prefix_unchanged=True),indent=2)+'\n')
print(json.dumps(dict(packet_length=len(raw),packet_sha256=H(raw),helper_lines=len(new.splitlines()),helper_sha256=H(new),rows=7,max_subject=max(x[k]['length']for x in rows for k in ['current','incoming','before']))))
