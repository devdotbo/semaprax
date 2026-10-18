from pathlib import Path
import subprocess,json,hashlib,re,importlib.util,copy
R=Path('/Volumes/Semaprax/worktrees/focused146-structural-repairs-20261018');D=Path(__file__).parent;HEAD='f3db66f4240ca6b3cc986d6080d428438b606141';G=lambda *a:subprocess.check_output(['git',*a],cwd=R);H=lambda b:hashlib.sha256(b).hexdigest();assert G('rev-parse','HEAD').decode().strip()==HEAD and not G('status','--porcelain').strip()
carriers=['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs'];before={p:(R/p).read_bytes()for p in carriers};after={p:b.decode()for p,b in before.items()};classes=json.loads(Path('/Volumes/Semaprax/provenance/ordinary-replay-receiving-892-final-union-20261018/ORIGINAL-PARSER-CLASSIFICATION.json').read_text());new_rows=[r for r in classes if r.get('new')and r['path'].endswith('.rs')];actual={p for p in G('diff','--name-only','--diff-filter=A','b5a7f2ba0',HEAD).decode().splitlines()if p.endswith('.rs')};assert actual=={r['path']for r in new_rows}|{'src/source_reflows_merge/main_e177_overlay_v2.rs'}
groups={};proof=[]
for row in new_rows:
 path=row['path'];assert (len((R/path).read_bytes()),H((R/path).read_bytes()))==(row['length'],row['sha256'])
 hit=next(x for x in row['discovery']if x['kind']=='lib'and x['package']=='semaprax');chain=hit['chain'];anchor=next((e['source_path']for e in reversed(chain)if all(len(re.findall(r'include_str!\("[^"\n]*'+re.escape(e['source_path'])+r'"\)',s))==1 for s in after.values())),None);assert anchor,path
 key=tuple((e['line'],e['module'])for e in chain);groups.setdefault(anchor,[]).append((key,path));proof.append(dict(source=path,anchor=anchor,chain=chain))
for anchor,children in groups.items():
 ordered=[p for _,p in sorted(children)]
 for carrier,s in after.items():
  m=list(re.finditer(r'include_str!\("([^"\n]*)'+re.escape(anchor)+r'"\)',s));assert len(m)==1;m=m[0];base=m.group(1);new='concat!('+m.group(0)+','+','.join('include_str!("'+base+p+'")'for p in ordered)+')';after[carrier]=s[:m.start()]+new+s[m.end():]
for p,s in after.items():
 assert len(s.splitlines())==len(before[p].splitlines())
 for row in new_rows:
  if row.get('new')and row['path'].endswith('.rs'):assert len(re.findall(r'include_str!\("[^"\n]*'+re.escape(row['path'])+r'"\)',s))==1
ordered=Path('/Volumes/Semaprax/provenance/next-large-side-source-composition-20261017/ordered-inputs.py');raw=ordered.read_bytes();assert H(raw)=='68b017e36af9c66f241e811df2114bd26af5f82fcc74b1978f6a9d25c35b1029';ns={};exec(compile(raw,str(ordered),'exec'),ns);oldtable='docs/evidence/main-e177-final-source-binding-20261017/current-projection-inputs.tsv';before[oldtable]=(R/oldtable).read_bytes();old=before[oldtable].decode().splitlines();paths,grammar=ns['ordered_inputs'](R,after[carriers[1]],HEAD);assert len(paths)==len(set(paths));assert [p for p in paths if p in set(old)]==old;added=[dict(at=i,path=p)for i,p in enumerate(paths)if p not in set(old)];assert set(x['path']for x in added)<=actual
D.joinpath('union-candidate').mkdir();files=[]
for p,s in after.items():
 q=D/'union-candidate'/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_text(s);files.append(dict(path=p,length=len(s.encode()),sha256=H(s.encode())))
q=D/'union-candidate'/oldtable;q.parent.mkdir(parents=True,exist_ok=True);q.write_text('\n'.join(paths)+'\n');files.append(dict(path=oldtable,length=q.stat().st_size,sha256=H(q.read_bytes())))
for p,b in before.items():q=D/'union-before'/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b)
(D/'union-proof.json').write_text(json.dumps(dict(head=HEAD,new_physical=len(actual),business_new=len(new_rows),children=proof,old_ordered_rows=len(old),new_ordered_rows=len(paths),insertions=added,grammar=grammar,files=files,Cargo=False),indent=2)+'\n');print('rawnew',len(actual),'table',len(old),'->',len(paths),'insertions',len(added))
