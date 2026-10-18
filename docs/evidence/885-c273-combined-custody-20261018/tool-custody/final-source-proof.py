from pathlib import Path
import subprocess,json,hashlib
R=Path('/Volumes/Semaprax/worktrees/next-large-combined-20261017');H=lambda b:hashlib.sha256(b).hexdigest();git=lambda *a:subprocess.check_output(['git',*a],cwd=R)
head=git('rev-parse','HEAD').decode().strip();assert head=='bf500d1f8d7bfe01548e517a8c9482e74c90a9dc'
g='docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py';raw=(R/g).read_bytes();ns={'__file__':str(R/g),'_AUTHENTICATED_PROGRAM':raw};exec(compile(raw,g,'exec'),ns)
p=ns['packet']();assert len(p['rows'])==444;c=ns['CONFIG'];assert len(c['modules'])==94
paths=[r['source'] for r in p['rows']];proc=subprocess.Popen(['git','cat-file','--batch'],cwd=R,stdin=subprocess.PIPE,stdout=subprocess.PIPE)
checks=[];archives={}
for r in p['rows']:
 path=r['source'];b=(R/path).read_bytes();pin=r['current'];assert(len(b),H(b))==(pin['length'],pin['sha256']),path
 proc.stdin.write(('HEAD:'+path+'\n').encode());proc.stdin.flush();header=proc.stdout.readline().split();assert len(header)==3 and header[1]==b'blob';n=int(header[2]);gb=proc.stdout.read(n);assert proc.stdout.read(1)==b'\n';assert gb==b,path
 checks.append(dict(path=path,length=len(b),sha256=H(b)))
 for v in [pin,*r['views']]:
  a=v['archive'];ab=(R/a).read_bytes();assert(len(ab),H(ab))==(v['length'],v['sha256']),a;archives[a]=H(ab)
proc.stdin.close();assert proc.wait()==0
for path,pin in c['modules'].items():
 b=(R/path).read_bytes();assert(len(b),H(b))==(pin['length'],pin['sha256']),path
for path in paths:assert not git('diff','--name-only','HEAD','--',path).strip(),path
assert not git('diff','--name-only','HEAD','--','src','tests','crates').strip(),'role generation changed raw source'
t='docs/evidence/main-e177-final-source-binding-20261017/current-projection-inputs.tsv';tb=(R/t).read_bytes();assert len(tb.splitlines())==975
result=dict(status='PASS_SOURCE_ONLY_ALL444_ACTUAL_GIT_AND_ARCHIVES',source_head=head,current_rows=444,modules=94,ordered_rows=975,all_current_rows_equal_physical_and_HEAD=True,all_archive_views_authenticated=True,role_generation_changed_raw_source=False,current_sources=checks,archives=archives,Cargo=False)
O=Path('/Volumes/Semaprax/provenance/current444-postbind-20261018/final-source-proof.json');O.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k not in ['current_sources','archives']}));print('receipt_sha256',H(O.read_bytes()))
