"""Bounded protocol fixtures only. No compiler, full guard or source binder runs."""
from pathlib import Path
import ast,copy,hashlib,json,os,stat,tempfile
D=Path(__file__).resolve().parent
ns={'__file__':str(D/'overlay_page.py')}
exec(compile((D/'overlay_page.py').read_bytes(),str(D/'overlay_page.py'),'exec'),ns)
H=ns['H'];canonical=ns['canonical'];compose=ns['compose_values'];parse=ns['parse']
# Reuse the WHOLE original no-follow/identity reader, not a more permissive fixture.
old=(D/'before/docs__evidence__main-e177-final-source-binding-20261017__guard_payload.py.txt').read_bytes()
assert H(old)=='4b37a7356881976afd8168a4bd8029796a083f1463d6c040ce5b986592745332'
node=next(n for n in ast.parse(old).body if isinstance(n,ast.FunctionDef)and n.name=='bounded')
reader={'os':os,'stat':stat,'hashlib':hashlib}
exec(compile(ast.Module(body=[node],type_ignores=[]),'<whole-original-bounded>','exec'),reader)
bounded=reader['bounded'];checks=[]
def check(name,body):
 body();checks.append(name)
def refuses(body):
 try:body()
 except (AssertionError,ValueError,KeyError,TypeError,OSError,StopIteration):return
 raise AssertionError('hostile accepted')
def record(source,raw):
 return dict(source=source,compiled=False,current=dict(length=len(raw),sha256=H(raw),archive='e/'+H(raw)),views=[],unknown_metadata={'order':[3,1,2]})
before=dict(schema='fixture',source_cap=ns['SOURCE_CAP'],unknown_root={'order':[4,2,1]},rows=[record('src/item.rs',b'old'),record('src/other.rs',b'other')])
pred=dict(extension=dict(path=ns['EXTENSION_PATH'],length=ns['EXTENSION_LENGTH'],sha256=ns['EXTENSION_SHA256']),rows=2,logical_sha256=H(canonical(before)))
row=copy.deepcopy(before['rows'][0]);row['current']=record('src/item.rs',b'new')['current'];row['views'].append(dict(name='whole-pre-overlay-v2-current',**before['rows'][0]['current']))
page=dict(schema='semaprax.finite-source-overlay-page.v2',index=1,total_pages=1,predecessor=pred,replacements=[row],additions=[])
policy=dict(schema='semaprax.finite-source-overlay-policy.v2',predecessor=pred,replacements=['src/item.rs'],additions=[],new_fragment_edges=[])
def inverse():
 result=compose(before,policy,page);assert result['unknown_root']==before['unknown_root'] and result['rows'][1]==before['rows'][1]
 assert result['rows'][0]['unknown_metadata']==before['rows'][0]['unknown_metadata'];result['rows']=copy.deepcopy(before['rows']);assert result==before
check('whole_predecessor_and_unknown_metadata_inverse',inverse)
for key,value in [('schema','foreign'),('index',2),('total_pages',2),('extra',True),('replacements',[])]:
 p=copy.deepcopy(page);p[key]=value;check('page_'+key,lambda p=p:refuses(lambda:compose(before,policy,p)))
p=copy.deepcopy(page);p['predecessor']['logical_sha256']='0'*64;check('wrong_predecessor',lambda:refuses(lambda:compose(before,policy,p)))
for source in ['src/foreign.rs','../src/item.rs','/src/item.rs','src//item.rs',ns['PAGE_PATH']]:
 p=copy.deepcopy(page);p['replacements'][0]['source']=source;check('path_'+source,lambda p=p:refuses(lambda:compose(before,policy,p)))
for key,value in [('compiled',True),('views',[]),('unknown_metadata',{'order':[1,2,3]})]:
 p=copy.deepcopy(page);p['replacements'][0][key]=value;check('row_'+key,lambda p=p:refuses(lambda:compose(before,policy,p)))
p=copy.deepcopy(page);p['replacements'][0]['views'][0]['archive']='e/other-channel';check('old_archive_channel',lambda:refuses(lambda:compose(before,policy,p)))
p=copy.deepcopy(page);p['replacements'].append(copy.deepcopy(p['replacements'][0]));check('duplicate_replacement',lambda:refuses(lambda:compose(before,policy,p)))
p=copy.deepcopy(page);p['replacements'][0]['fragments']=[dict(source='src/item.rs',compiled=True)];check('fragment_self',lambda:refuses(lambda:compose(before,policy,p)))
p=copy.deepcopy(page);p['replacements'][0]['current']['length']=ns['SOURCE_CAP']+1;check('individual_source_bound',lambda:refuses(lambda:compose(before,policy,p)))
for raw in [b'{"a":1,"a":1}\n',b'{ "a":1}\n',b'{"a":1}\n ',b'']:
 check('JSON_'+H(raw),lambda raw=raw:refuses(lambda:parse(raw)))
with tempfile.TemporaryDirectory(prefix='semaprax-page-source-',dir='/Volumes/Semaprax/runtime-tmp')as temp:
 root=Path(temp);(root/'src').mkdir();(root/'e').mkdir()
 for raw in [b'old',b'new',b'other']:(root/'e'/H(raw)).write_bytes(raw)
 path=root/'src/item.rs';path.write_bytes(b'new') # Old physical source deliberately absent.
 historical=[]
 def read(pin):
  if pin['sha256']==H(b'old'):historical.append(pin['archive'])
  return bounded(root/pin['archive'],pin['length'],pin['sha256'])
 original=dict(source='src/item.rs',before=before['rows'][0]['current'],current=before['rows'][0]['current'])
 overlay=ns['Overlay'](root,bounded,read,lambda:None,lambda:copy.deepcopy(before),lambda:[original])
 overlay.load=lambda:(copy.deepcopy(policy),copy.deepcopy(page)) # Exact synthetic metadata fixture, not production admission.
 ns['REPLACEMENTS']=('src/item.rs',)
 check('changed_physical_before_history',lambda:None if overlay.stage_source('src/item.rs',before['rows'][0]['current'])==b'old' else (_ for _ in ()).throw(AssertionError()))
 assert historical
 historical.clear();path.write_bytes(b'BAD');check('wrong_current_before_history',lambda:refuses(lambda:overlay.stage_source('src/item.rs',before['rows'][0]['current'])));assert not historical
 path.write_bytes(b'new');historical.clear();(root/'e'/H(b'new')).write_bytes(b'BAD');check('wrong_current_archive_before_history',lambda:refuses(lambda:overlay.stage_source('src/item.rs',before['rows'][0]['current'])));assert not historical
 (root/'e'/H(b'new')).write_bytes(b'new');check('unlisted_historical_digest',lambda:refuses(lambda:overlay.stage_source('src/item.rs',dict(length=3,sha256='0'*64))))
 # The actual current-tree reader must refuse a changed audited child before
 # selecting even the unchanged parent's historical view.
 child=record('src/leaf.rs',b'leaf');child['compiled']=True
 root_row=copy.deepcopy(row);root_row['fragments']=[child]
 (root/'src/leaf.rs').write_bytes(b'leaf');(root/'e'/H(b'leaf')).write_bytes(b'leaf')
 def root_then_history():
  overlay.current(root_row);return read(before['rows'][0]['current'])
 historical.clear();assert root_then_history()==b'old' and historical
 historical.clear();(root/'src/leaf.rs').write_bytes(b'BAD!')
 check('changed_audit_leaf_physical_before_parent_history',lambda:refuses(root_then_history));assert not historical
 (root/'src/leaf.rs').write_bytes(b'leaf');historical.clear();(root/'e'/H(b'leaf')).write_bytes(b'BAD!')
 check('changed_audit_leaf_archive_before_parent_history',lambda:refuses(root_then_history));assert not historical
 ns['REPLACEMENTS']=()
 check('directory_refusal',lambda:refuses(lambda:bounded(root,0,H(b''))))
 if hasattr(os,'symlink'):
  os.symlink(path,root/'link');check('symlink_refusal',lambda:refuses(lambda:bounded(root/'link',3,H(b'new'))))
 if hasattr(os,'mkfifo'):
  os.mkfifo(root/'fifo');check('fifo_refusal',lambda:refuses(lambda:bounded(root/'fifo',0,H(b''))))
 raw=b'x'*ns['PAGE_CAP'];(root/'max').write_bytes(raw);check('exact_page_reader_bound',lambda:None if bounded(root/'max',len(raw),H(raw))==raw else (_ for _ in ()).throw(AssertionError()))
 check('page_reader_bound_plus_one',lambda:refuses(lambda:bounded(root/'max',len(raw)+1,H(raw))))
 class ReplaceInode:
  def __getattr__(self,name):return getattr(os,name)
  def open(self,p,flags):
   fd=os.open(p,flags);new=root/'swap';new.write_bytes(b'new');os.replace(new,p);return fd
 reader['os']=ReplaceInode();check('replaced_inode_refusal',lambda:refuses(lambda:bounded(path,3,H(b'new'))));reader['os']=os
 class MutateAfterRead:
  count=0
  def __getattr__(self,name):return getattr(os,name)
  def fstat(self,fd):
   self.count+=1
   if self.count==2:
    s=os.stat(path);os.utime(path,ns=(s.st_atime_ns,s.st_mtime_ns+1000000))
   return os.fstat(fd)
 reader['os']=MutateAfterRead();check('midread_identity_mutation_refusal',lambda:refuses(lambda:bounded(path,3,H(b'new'))));reader['os']=os
print(json.dumps(dict(status='PASS_PYTHON_PROTOCOL_FIXTURES_ONLY',checks=len(checks),names=checks,Rust_executed=False,full_source_guard_executed=False,binder_executed=False),indent=2))
