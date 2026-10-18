from pathlib import Path
import json,hashlib,subprocess,copy,ast,os,stat
R=Path('/Volumes/Semaprax/worktrees/next-large-combined-20261017');D=Path(__file__).parent;HEAD=__import__('sys').argv[1];HEAD=subprocess.check_output(['git','rev-parse',HEAD],cwd=R).decode().strip();git=lambda p:subprocess.check_output(['git','show',HEAD+':'+p],cwd=R);H=lambda b:hashlib.sha256(b).hexdigest();E=lambda x:(json.dumps(x,separators=(',',':'))+'\n').encode();assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=R).decode().strip()==HEAD and not subprocess.check_output(['git','status','--porcelain'],cwd=R).strip()
REL='docs/evidence/fmt-local-441-20261018';EXT=REL+'/extension.json';CFG='docs/evidence/remote12ad-checkpoint-20261018/guard-config.json';HELP='src/source_reflows_merge/main_e177_final_inputs.rs';GUARD='docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py';BASE='docs/evidence/main-e177-final-source-binding-20261017/current-inputs.json';out={};old={p:git(p)for p in [EXT,CFG,HELP,GUARD]};e=json.loads(old[EXT]);c=json.loads(old[CFG]);base=json.loads(git(BASE));by={r['source']:copy.deepcopy(r)for r in base['rows']};by.update({r['source']:r for r in e['rows']});before=copy.deepcopy(by);expected={'src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase.rs','src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_tests.rs'};changed=[]
for p,r in by.items():
 b=git(p);assert (R/p).read_bytes()==b
 if(len(b),H(b))==(r['current']['length'],r['current']['sha256']):continue
 assert p in expected,p
 prior=copy.deepcopy(r['current']);view=dict(name='whole-67281-C5-current',**prior)
 if view not in r['views']:r['views'].append(view)
 archive=REL+'/whole/'+H(b)+'.txt';assert not(R/archive).exists()or (R/archive).read_bytes()==b;out[archive]=b;r['current']=dict(length=len(b),sha256=H(b),archive=archive);changed.append(p)
assert set(changed)==expected
# Preserve all prior prior extension rows/order and add only the two actual moved source rows.
oldorder=[r['source']for r in e['rows']]
for r in base['rows']:
 if r['source']in expected and r['source']not in oldorder:e['rows'].append(by[r['source']])
assert [r['source']for r in e['rows']][:len(oldorder)]==oldorder
for p,r in by.items():
 if 'fragments'in r:r['fragments']=[copy.deepcopy(by[x['source']])for x in r['fragments']]
for p,r in before.items():
 now=copy.deepcopy(by[p])
 if p in expected:now['current']=r['current'];now['views']=r['views']
 if 'fragments'in r:now['fragments']=r['fragments']
 assert now==r
# Reapply only exact named predecessor pins required by the existing protocol.
local=json.loads(git(e['whole_local133']['archive']))
for tag,whole in [('whole-fmt433-current',base),('whole-local133-current',local)]:
 for oldrow in whole['rows']:
  row=by[oldrow['source']]
  if row['current']!=oldrow['current']:
   pin=dict(name=tag,**oldrow['current'])
   if pin not in row['views']:row['views'].append(pin)
ext=E(e);assert len(ext)<=262144;out[EXT]=ext
helper=old[HELP].decode();x=f'({len(old[EXT])},"{H(old[EXT])}".to_owned())';y=f'({len(ext)},"{H(ext)}".to_owned())';assert helper.count(x)==1;helper_pin_edit=dict(before=x,after=y);helper=helper.replace(x,y);out[HELP]=helper.encode();assert len(helper.splitlines())<=1500
stage=c['source_successor']['path'];old[stage]=git(stage);sv=json.loads(old[stage])
for row in sv['rows']:
 if row['source']not in expected:continue
 raw=git(row['source']);prior=dict(row['current']);view=dict(name='whole-67281-C5-current',**prior)
 if view not in row.setdefault('views',[]):row['views'].append(view)
 archive=REL+'/whole/'+H(raw)+'.txt';out[archive]=raw;row['current']=dict(length=len(raw),sha256=H(raw),archive=archive)
stagebytes=E(sv);assert len(stagebytes)<=262144;out[stage]=stagebytes
before=H(old[stage]);after=H(stagebytes);assert helper.count(before)==2;helper=helper.replace(before,after).replace(str(len(old[stage])),str(len(stagebytes)));out[HELP]=helper.encode();c['source_successor'].update(length=len(stagebytes),sha256=H(stagebytes))
for p in sorted(expected&set(c['modules'])):
 b=git(p);c['modules'][p].update(length=len(b),sha256=H(b))
c['extension'].update(length=len(ext),sha256=H(ext));assert HELP in c['modules'];c['modules'][HELP].update(length=len(out[HELP]),sha256=H(out[HELP]));cfg=E(c);assert len(cfg)<=262144;out[CFG]=cfg
s=old[GUARD].decode();edits=[]
for name,value in [('_CONFIG_LENGTH',len(cfg)),('_CONFIG_SHA256',H(cfg))]:
 tree=ast.parse(s);n=next(n.value for n in tree.body if isinstance(n,ast.Assign)and isinstance(n.targets[0],ast.Name)and n.targets[0].id==name);lines=s.splitlines(True);a=sum(map(len,lines[:n.lineno-1]))+n.col_offset;b=sum(map(len,lines[:n.end_lineno-1]))+n.end_col_offset;edits.append(dict(before=s[a:b],after=repr(value)));s=s[:a]+repr(value)+s[b:]
ast.parse(s);assert len(s.encode())<=65536;out[GUARD]=s.encode()
for p in ['docs/evidence/accepted-strict-guard-generation-20261017/check-source.py','docs/evidence/accepted-final6-source-binding-20261017/check-source.py','docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py']:
 b=git(p);old[p]=b;assert b.count(H(old[GUARD]).encode())==1;b=b.replace(H(old[GUARD]).encode(),H(out[GUARD]).encode());prefix='guard_raw=held(guard_helper,'if'verify-or-refresh'in p else'helper_raw=bounded(helper,';x=(prefix+str(len(old[GUARD]))).encode();assert b.count(x)==1;b=b.replace(x,(prefix+str(len(out[GUARD]))).encode());ast.parse(b);out[p]=b
# Authenticate every full current image/view and all current module pins, without raising Python reader bounds.
for r in by.values():
 for pin in [r['current']]+r['views']:
  b=out.get(pin['archive'],None)
  if b is None:b=git(pin['archive'])
  assert(len(b),H(b))==(pin['length'],pin['sha256'])
for p,pin in c['modules'].items():
 b=out.get(p,git(p));assert(len(b),H(b))==(pin['length'],pin['sha256'])
O=D/'postbind-candidate';assert not O.exists();O.mkdir();records=[]
for p,b in sorted(out.items()):
 q=O/'candidate'/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b)
 if p in old:q=O/'whole-before'/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(old[p])
 records.append(dict(path=p,length=len(b),sha256=H(b),before_sha256=H(old[p])if p in old else None))
(O/'receipt.json').write_text(json.dumps(dict(status='SOURCE_ONLY_TWO_C5_COMPILE_FIX_ROWS_UNAPPLIED',source_head=HEAD,changed_sources=changed,logical_rows=444,base433_unchanged_sha256=H(git(BASE)),whole_extension_before_sha256=H(old[EXT]),extension_after_sha256=H(ext),old_extension_rows=len(oldorder),new_extension_rows=len(e['rows']),helper_pin_edit=helper_pin_edit,guard_pin_edits=edits,files=records,prior174_and_FINAL6_producers_not_replayed=True,Cargo=False),indent=2)+'\n');print(O/'receipt.json')
