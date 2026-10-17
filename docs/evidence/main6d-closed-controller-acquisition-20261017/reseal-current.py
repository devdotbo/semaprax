"""Reseal only seven current Main6d subjects after the final reviewed source/raw edits."""
from pathlib import Path
import argparse,json,hashlib,os,stat,subprocess,re
P='docs/evidence/main6d-closed-controller-acquisition-20261017/inputs.json'
S='src/source_reflows/semantic_receivers/status_inline.rs'
MAIN='6d101efd8c255330a3fe5fb0c06e058c6f8c085f'
SUBJECTS={'src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs','src/cleanup_plan/replay/expression_facts/receiving/retained.rs','tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs','src/source_reflows/semantic_receivers.rs','docs/evidence/projection-audit-scope-v1/legacy-inputs.tsv'}
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);p.add_argument('--before-head',required=True);p.add_argument('--expected-head',required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();R=a.repo.resolve();assert R.is_relative_to('/Volumes/Semaprax/worktrees');assert all(re.fullmatch('[0-9a-f]{40}',x)for x in [a.before_head,a.expected_head]);git=lambda *v:subprocess.check_output(['git',*v],cwd=R);assert git('rev-parse','HEAD').decode().strip()==a.expected_head;assert not a.out.exists();a.out.mkdir(parents=True);H=lambda b:hashlib.sha256(b).hexdigest()
def old(path):
 assert int(git('cat-file','-s',a.before_head+':'+path))<=262144
 return git('show',a.before_head+':'+path)
def held(path):
 fd=os.open(R/path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC);ident=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  st=os.fstat(fd);assert stat.S_ISREG(st.st_mode)and st.st_size<=262144
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(st.st_size);assert len(b)==st.st_size and not f.read(1)
  assert ident(st)==ident(os.fstat(fd))==ident(os.lstat(R/path));return b
 finally:os.close(fd)
before=old(P);helper=old(S);assert held(P)==before and held(S)==helper
v=json.loads(before);assert v['main_head']==MAIN and len(v['rows'])==7 and {x['source']for x in v['rows']}==SUBJECTS
snapshots={};changes=[]
for row in v['rows']:
 for field in ['current','incoming','before']:
  q=row[field];raw=held(q['path']);assert(len(raw),H(raw))==(q['length'],q['sha256'])
 source=row['source'];actual=held(source);snapshots[source]=actual
 if (len(actual),H(actual))!=(row['current']['length'],row['current']['sha256']):
  previous=old(source);changes.append(dict(source=source,before_length=len(previous),before_sha256=H(previous),after_length=len(actual),after_sha256=H(actual)));(a.out/(source.replace('/','__')+'.before.txt')).write_bytes(previous)
 archive='docs/evidence/main6d-closed-controller-acquisition-20261017/current/'+H(actual)+'.txt';row['current']=dict(path=archive,length=len(actual),sha256=H(actual))
v['current_source_stage']=dict(kind='reviewed-working-stage',git_head=a.expected_head,actual_tuples=[dict(source=row['source'],length=row['current']['length'],sha256=row['current']['sha256'])for row in v['rows']],working_delta_paths=git('status','--porcelain','--untracked-files=all').decode().splitlines())
after=(json.dumps(v,indent=2)+'\n').encode();assert len(after)<=262144
s=helper.decode();ops=[]
for x,z,count in [('assert_eq!(data.len(), '+str(len(before))+');','assert_eq!(data.len(), '+str(len(after))+');',1),(', '+str(len(before))+',"'+H(before)+'"',', '+str(len(after))+',"'+H(after)+'"',0)]:
 if x==z:continue
 if count==0:continue # The held call has no spaces; handle the exact anchored call below.
 assert s.count(x)==count;s=s.replace(x,z);ops.append(dict(before=x,after=z,count=count))
x='inputs.json"),'+str(len(before))+',"';z='inputs.json"),'+str(len(after))+',"'
if x!=z:assert s.count(x)==1;s=s.replace(x,z);ops.append(dict(before=x,after=z,count=1))
assert s.count(H(before))==2;s=s.replace(H(before),H(after));ops.append(dict(before=H(before),after=H(after),count=2))
undo=s
for op in reversed(ops):assert undo.count(op['after'])==op['count'];undo=undo.replace(op['after'],op['before'])
assert undo.encode()==helper
(a.out/'packet.whole-before.json.txt').write_bytes(before);(a.out/'helper.whole-before.rs.txt').write_bytes(helper)
assert git('rev-parse','HEAD').decode().strip()==a.expected_head and held(P)==before and held(S)==helper
for path,raw in snapshots.items():assert held(path)==raw
assert git('rev-parse','HEAD').decode().strip()==a.expected_head
for row in v['rows']:
 path=R/row['current']['path'];path.parent.mkdir(parents=True,exist_ok=True)
 if path.exists():assert held(row['current']['path'])==snapshots[row['source']]
 else:path.write_bytes(snapshots[row['source']])
(R/P).write_bytes(after);(R/S).write_text(s)
report=dict(status='SOURCE_ONLY_SEVEN_CURRENT_TUPLES_RESEALED',head=a.expected_head,before_head=a.before_head,changed_subjects=changes,all_incoming_before_control_fields_unchanged=True,whole_packet_before_sha256=H(before),packet_after_sha256=H(after),helper_edits=ops,whole_helper_inverse=True,Cargo=False,Rust_executed=False)
(a.out/'receipt.json').write_text(json.dumps(report,indent=2)+'\n');print(a.out/'receipt.json')
