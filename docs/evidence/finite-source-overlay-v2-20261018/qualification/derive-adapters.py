from pathlib import Path
import hashlib,json,ast
D=Path(__file__).parent/'adapters';D.mkdir(exist_ok=True);R=Path('/Volumes/Semaprax/worktrees/focused146-structural-repairs-20261018');H=lambda b:hashlib.sha256(b).hexdigest()
def derive(name,old,edits):
 raw=Path(old).read_bytes();s=raw.decode()
 for a,b in edits:assert s.count(a)==1,(name,a,s.count(a));s=s.replace(a,b,1)
 back=s
 for a,b in reversed(edits):assert back.count(b)==1;back=back.replace(b,a,1)
 assert back.encode()==raw;ast.parse(s);(D/(name+'.whole-before.txt')).write_bytes(raw);(D/name).write_text(s);(D/(name+'.inverse.json')).write_text(json.dumps(dict(before_sha256=H(raw),after_sha256=H(s.encode()),edits=[dict(before=a,after=b)for a,b in edits]),indent=2)+'\n')
oldroot="/Volumes/Semaprax/worktrees/next-large-combined-20261017";rel='docs/evidence/finite-overlay542-current174-20261018'
extra="""# The ONE fixed overlay page and its literal dependencies are part of the actual readset.
page_root='docs/evidence/finite-source-overlay-v2-20261018/'
needed.update(page_root+name for name in ('policy.json','page.json','overlay_page.py'))
page=json.loads((R/(page_root+'page.json')).read_bytes())
for row in [*page['replacements'],*page['additions']]:
 if row['current']['length']<=262144:needed.add(row['source'])
 else:excluded.append(dict(path=row['source'],reason='page-current-exceeds-original-Python-perread-bound'))
 for pin in [row['current'],*row['views']]:
  if pin['length']<=262144:needed.add(pin['archive'])
  else:excluded.append(dict(path=pin['archive'],reason='page-view-exceeds-original-Python-perread-bound'))
"""
derive('prepare174.py','/Volumes/Semaprax/provenance/mergef94-519-174-final6-adapters-20261018/prepare174-current441.py',[(oldroot,str(R)),("REL='docs/evidence/mergef94-current519-174-20261018'",'REL='+repr(rel)),("assert cfg['rows']==433 and cfg['logical_rows']==len(packet['rows'])==519","assert cfg['rows']==433 and cfg['logical_rows']==519 and len(packet['rows'])==542"),("strict=values['STRICT']",extra+"\nstrict=values['STRICT']"),("logical_rows=519","logical_rows=542")])
nsraw=(R/'src/source_reflows/incoming090_semantic_receivers.rs').read_bytes();sha=H(nsraw)
derive('fix174-mask.py','/Volumes/Semaprax/provenance/mergef94-519-174-final6-adapters-20261018/fix174-mask.py',[(oldroot,str(R)),('docs/evidence/mergef94-current519-174-20261018/successor174/bind-next.py',rel+'/successor174/bind-next.py'),("assert h(current)=='5bd87c75b3742408c1f2643ec99278a10c70353d99c3072b85329241b1fe86d7'","assert h(current)=='"+sha+"'"),("sha(source)=='5bd87c75b3742408c1f2643ec99278a10c70353d99c3072b85329241b1fe86d7'","sha(source)=='"+sha+"'")])
