"""Read-only exact74/current471/Main6d7/46 module/451 ordered-table auth."""
from pathlib import Path
import argparse,json,hashlib,os,stat,subprocess,ast
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);p.add_argument('--expected-head',required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();R=a.repo.resolve();H=lambda b:hashlib.sha256(b).hexdigest();git=lambda *v:subprocess.check_output(['git',*v],cwd=R);assert git('rev-parse','HEAD').decode().strip()==a.expected_head;assert not git('status','--porcelain','--untracked-files=all').strip();checks=[]
def held(p,length,sha,cap):
 assert 0<=length<=cap;fd=os.open(R/p,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC);I=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  st=os.fstat(fd);assert stat.S_ISREG(st.st_mode)and st.st_size==length
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(length);assert len(b)==length and not f.read(1)
  assert I(st)==I(os.fstat(fd))==I(os.lstat(R/p));assert H(b)==sha,(p,length,sha);checks.append((p,length,sha));return b
 finally:os.close(fd)
def cfg(path,expected):
 b=held(path,expected['length'],expected['sha256'],65536);return next(ast.literal_eval(n.value)for n in ast.parse(b).body if isinstance(n,ast.Assign)and isinstance(n.targets[0],ast.Name)and n.targets[0].id=='CONFIG')
SEALS=json.loads(Path(__file__).with_name('postbind-seals.json').read_bytes());config=cfg(SEALS['guard']['path'],SEALS['guard']);q=config['packet'];v=json.loads(held(q['path'],q['length'],q['sha256'],262144));assert len(v['rows'])==74;assert v['source_cap']==9753094;assert len({x['source']for x in v['rows']})==74
for row in v['rows']:
 c=row['current'];raw=held(row['source'],c['length'],c['sha256'],9753094);assert raw==held(c['archive'],c['length'],c['sha256'],9753094)
 for b in row['views']:held(b['archive'],b['length'],b['sha256'],9753094)
for b in v['complete_old_table_anchors']:assert held(b['path'],b['length'],b['sha256'],9753094)==held(b['archive'],b['length'],b['sha256'],9753094)
assert len(config['modules'])==46
for path,b in config['modules'].items():held(path,b['length'],b['sha256'],262144)
t=config['table'];raw=held(t['path'],t['length'],t['sha256'],262144);prior=t['prior'];order=held(prior['archive'],prior['length'],prior['sha256'],262144).decode().splitlines()
for x in t['insertions']:assert x['path']not in order;order.insert(x['at'],x['path'])
assert len(order)==451 and raw==('\n'.join(order)+'\n').encode()
for path,b in config['carriers'].items():
 s=held(path,b['length'],b['sha256'],262144).decode();assert all(s.count('include_str!("'+b['base']+m+'")')==1 for m in config['modules'])
for name,count in [('e177',471),('main6d',7)]:
 b=SEALS[name];p=json.loads(held(b['path'],b['length'],b['sha256'],262144));assert len(p['rows'])==count
 for row in p['rows']:
  if name=='e177':held(row['path'],row['current_length'],row['current_sha256'],9753094);held(row['main_archive'],row['main_length'],row['main_sha256'],9753094)
  else:
   for field in ['current','incoming','before']:
    c=row[field];held(c['path'],c['length'],c['sha256'],262144)
   c=row['current'];held(row['source'],c['length'],c['sha256'],262144)
assert git('rev-parse','HEAD').decode().strip()==a.expected_head and not git('status','--porcelain','--untracked-files=all').strip();a.out.write_text(json.dumps(dict(status='PASS_SOURCE_ONLY',head=a.expected_head,current_rows=74,incoming471_current_rows=471,main6d_rows=7,required_modules=46,ordered_table_rows=451,all_exact_held_channels=len(checks),no_source_drift=True,Cargo=False,Rust_executed=False),indent=2)+'\n');print(a.out)
