from pathlib import Path
import subprocess,json,hashlib,difflib,argparse
args=argparse.ArgumentParser();args.add_argument('--basis',required=True);BASE=args.parse_args().basis
sha=lambda x:hashlib.sha256(x).hexdigest()
changed=set(subprocess.check_output(['git','diff',BASE,'--name-only']).decode().splitlines())
reverse={'loan-candidate-inventory-reuse','exit-status-directory-reuse','referenced-edge-ownership-proof','prefix-certified-expression-lookup','receiving-transitive-results-816','exit-prefix-reference-proof','status-fact-borrow-reuse'}
packets=[]
for p in Path('docs/evidence').rglob('*.json'):
 if p.name not in ['exact-source-inverses.json','whole-source-inverses.json','source-inverses.json'] or 'nested-byte-traversal' in str(p):continue
 try:x=json.loads(subprocess.check_output(['git','show',BASE+':'+str(p)]))
 except:continue
 rows=x if isinstance(x,list)else x.get('whole_sources',[]) if isinstance(x,dict)else[]
 if isinstance(rows,list) and any(isinstance(r,dict)and r.get('source',r.get('file'))in changed for r in rows):packets.append((p,x,rows,None))
p=Path('docs/evidence/nominal-payload-827/exact-source-inverses-activation-v14.json');invpath=p.with_name('source-inventory-activation-v14.json');inventory=json.loads(subprocess.check_output(['git','show',BASE+':'+str(invpath)]));nominal={r['source']:r for r in inventory['rows']};x=json.loads(subprocess.check_output(['git','show',BASE+':'+str(p)]));packets.append((p,x,x,nominal))
def bridges(before,current,nk):
 a,b=before.splitlines(True),current.splitlines(True)
 for context in [2,4,8,16,32,64,128,1024,10000]:
  edits=[];checked=current
  for group in difflib.SequenceMatcher(None,a,b,autojunk=False).get_grouped_opcodes(context):
   old=''.join(a[group[0][1]:group[-1][2]]);new=''.join(b[group[0][3]:group[-1][4]])
   if not new or checked.count(new)!=1:break
   edits.append({nk:new,'before':old});checked=checked.replace(new,old,1)
  else:
   if checked==before:return edits
 raise AssertionError(('ambiguous bridge',nk))
summary=[];write=[]
for p,packet,rows,nom in packets:
 repairs=[];original_rows=0;independent_new_rows=0
 for row in rows:
  file=row.get('source',row.get('file'));current=Path(file).read_text();key='inverse'if'inverse'in row else'hooks'if'hooks'in row else'edits';edits=row.get(key,[]);nk='current'if key=='inverse'else'after'if(key=='hooks'or(edits and'after'in edits[0]))else'new'
  if file in changed:
   old=subprocess.check_output(['git','show',BASE+':'+file]).decode();added=bridges(old,current,nk)
   for e in added:
    if nk=='after':e.update(name='nested-byte-current-whole-source-preinverse',count=1)
    if key=='inverse':e['hook']='nested-byte-current-whole-source-preinverse'
   if not all(any(e.get(nk)==b[nk] and e.get('before')==b['before'] for e in edits) for b in added):
    if p.parent.name in reverse:row[key]=edits+list(reversed(added))
    else:row[key]=added+edits
   repairs.append({'source':file,'exact_later_blocks':len(added)})
  r=nom[file]if nom else row
  if r.get('new_file') and not row.get(key):
   binding=r.get('current_sha256',r.get('sha256'))
   assert binding==sha(current.encode()),(str(p),file,'independent new source binding')
   independent_new_rows+=1
   continue
  original_rows+=1
  projected=current;order=list(reversed(row.get(key,[])))if p.parent.name in reverse else row.get(key,[])
  for e in order:
   assert projected.count(e[nk])==e.get('count',1),(str(p),file,'count',repr(e[nk][:120]),projected.count(e[nk]))
   projected=projected.replace(e[nk],e['before'],e.get('count',1))
  r=nom[file]if nom else row
  ar=r.get('archive',r.get('original'));expected=r.get('original_sha256',r.get('before_sha256',r.get('old_sha256',r.get('sha256'))))
  if expected:assert sha(projected.encode())==expected,(str(p),file,'old sha')
  if ar:
   a=Path(ar)
   if not a.exists():a=p.parent/ar
   assert a.is_file(),(str(p),file,'archive missing',str(a));assert a.read_bytes()==projected.encode(),(str(p),file,'full archive')
  elif r.get('new_file'):assert not projected,(str(p),file,'new original')
  if 'current_sha256'in row:row['current_sha256']=sha(current.encode())
  if 'current_bytes'in row:row['current_bytes']=len(current.encode())
  if nom:nom[file]['sha256']=sha(current.encode())
 write.append((p,packet));summary.append({'packet':str(p),'rows':len(rows),'repairs':repairs,'all_originals_verified':True,'projected_original_rows':original_rows,'independently_bound_new_rows':independent_new_rows})
for p,x in write:p.write_text(json.dumps(x,indent=2)+'\n')
inventory['repair_basis']='actual 5b562965d composed with private nested-byte source; no runtime claim';invpath.write_text(json.dumps(inventory,indent=2)+'\n')
Path('docs/evidence/nested-byte-traversal-780/later-source-proof.json').write_text(json.dumps({'status':'SOURCE_ONLY_NO_RUST_EXECUTION','basis':BASE,'packets':summary},indent=2)+'\n')
print('PASS',len(summary),'packets',sum(r['rows']for r in summary),'full source rows; unchanged archives')
