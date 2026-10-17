from pathlib import Path
import json,subprocess,hashlib,os,stat
R=Path('/Volumes/Semaprax/worktrees/wasm-custody-compile-repair-20261017');HEAD='566bf724d583f2d00040e9336404d6d6f902bd38';REL='docs/evidence/next-core-final-coupling-20261017';H=lambda b:hashlib.sha256(b).hexdigest();G=lambda p:subprocess.check_output(['git','show',HEAD+':'+p],cwd=R)
def held(p):
 fd=os.open(R/p,os.O_RDONLY|os.O_NOFOLLOW|os.O_CLOEXEC|os.O_NONBLOCK);ident=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  st=os.fstat(fd);assert stat.S_ISREG(st.st_mode)and st.st_size<=9753094
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(st.st_size);assert len(b)==st.st_size and not f.read(1)
  assert ident(st)==ident(os.fstat(fd))==ident(os.lstat(R/p));return b
 finally:os.close(fd)
p='docs/evidence/main-e177-full-controller-acquisition-20261017/inputs.json';old=held(p);assert old==G(p);v=json.loads(old);assert len(v['rows'])==471;changes=[]
for row in v['rows']:
 raw=held(row['path']);pair=(len(raw),H(raw));prior=(row['current_length'],row['current_sha256'])
 if pair!=prior:changes.append(dict(source=row['path'],before=prior,after=pair));row['current_length'],row['current_sha256']=pair
 # Full original incoming channels remain independently authenticated.
 incoming=held(row['main_archive']);assert(len(incoming),H(incoming))==(row['main_length'],row['main_sha256'])
new=(json.dumps(v,indent=2)+'\n').encode();assert len(new)<=262144
undo=json.loads(new)
for change in changes:
 row=next(x for x in undo['rows']if x['path']==change['source']);row['current_length'],row['current_sha256']=change['before']
assert undo==json.loads(old);(R/p).write_bytes(new)
h='src/source_reflows_merge/main_e177_inputs.rs';b=G(h);assert held(h)==b;s=b.decode();ops=[]
for a,z in [(f'assert_eq!(data.len(), {len(old)}); // MAIN_E177_PACKET_LENGTH',f'assert_eq!(data.len(), {len(new)}); // MAIN_E177_PACKET_LENGTH'),(H(old),H(new))]:
 if a==z:continue
 assert s.count(a)==1;s=s.replace(a,z);ops.append(dict(before=a,after=z,count=1))
(R/h).write_text(s);(R/REL/'471-reseal.json').write_text(json.dumps(dict(head=HEAD,changes=changes,helper_edits=ops,all_incoming_fields_order_unchanged=True,packet_length=len(new),packet_sha256=H(new)),indent=2)+'\n');print(len(changes),len(new),H(new))
