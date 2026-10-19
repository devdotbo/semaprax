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
# V9 controls are additive: the complete preceding fixtures above stay intact.
def v9_fixture():
 old_rows=[record('src/v9-'+str(i)+'.rs',b'old') for i in range(519)]
 old_rows[0]['unknown_metadata']['pin_like']=[True,'not-a-digest','../not-a-pin']
 old_packet=dict(schema='fixture',source_cap=ns['SOURCE_CAP'],unknown_root={'pin_like':[0,'x']},rows=old_rows)
 predecessor=dict(extension=pred['extension'],rows=519,logical_sha256=H(canonical(old_packet)))
 first=copy.deepcopy(old_rows[0]);first['current']=record(first['source'],b'new')['current']
 first['views']=[dict(name='whole-pre-overlay-v2-current',**old_rows[0]['current'])]
 second_current=record(old_rows[1]['source'],b'next')['current']
 second_views=[dict(name='whole-pre-overlay-v2-current',**old_rows[1]['current'])]
 addition=record('src/v9-new.rs',b'new')
 addition['current']['extra']=None # Entire object fallback retains unknown fields.
 addition['views']=[dict(name='literal',length=0,sha256=H(b''),archive='old/channel'),
                    dict(name='unknown',length=0,sha256=H(b''),archive='e/'+H(b''),extra={'pin_like':[0,H(b'')]})]
 v8=dict(schema=ns['V8_SCHEMA'],index=1,total_pages=1,predecessor=predecessor,
         replacements=[first,{'old519_index':[1,second_current,second_views,None]}],additions=[addition])
 admitted=dict(schema='semaprax.finite-source-overlay-policy.v2',predecessor=predecessor,
               replacements=[r['source'] for r in old_rows[:2]],additions=[dict(source=addition['source'],compiled=False)],new_fragment_edges=[])
 return old_packet,admitted,v8
v9_before,v9_policy,v8_page=v9_fixture()
v9_page=ns['encode_v9_pins'](v8_page)
def v9_complete():
 assert canonical(ns['expand_v9_pins'](v9_page))==canonical(v8_page)
 assert compose(v9_before,v9_policy,v9_page)==compose(v9_before,v9_policy,v8_page)
 assert v9_page['additions'][0]['current']==v8_page['additions'][0]['current']
 assert v9_page['additions'][0]['views'][1]==v8_page['additions'][0]['views'][1]
 assert v9_page['additions'][0]['views'][0]==['literal',0,H(b''),'old/channel']
 assert v9_page['replacements'][0]['unknown_metadata']==v8_page['replacements'][0]['unknown_metadata']
 assert v9_page['replacements'][1]['old519_index'][3] is None
check('v9_entire_v8_and_composed_packet_unknowns_and_literal_archives',v9_complete)
def v9_fragment_slots():
 page=copy.deepcopy(v8_page);leaf=record('src/leaf.rs',b'leaf');leaf['compiled']=True
 leaf['views']=[dict(name='old',**leaf['current'])]
 leaf['unknown_metadata']['current']=[True,'not-a-digest']
 page['replacements'][0]['fragments']=[leaf,{'canonical_source':519}]
 page['replacements'][1]['old519_index'][3]=[copy.deepcopy(leaf),{'canonical_source':'src/leaf.rs'}]
 encoded=ns['encode_v9_pins'](page)
 assert type(encoded['replacements'][0]['fragments'][0]['current']) is list
 assert encoded['replacements'][0]['fragments'][1]=={'canonical_source':519}
 assert encoded['replacements'][1]['old519_index'][3][1]=={'canonical_source':'src/leaf.rs'}
 assert canonical(ns['expand_v9_pins'](encoded))==canonical(page)
check('v9_only_documented_one_level_fragment_slots',v9_fragment_slots)
for bad in [[],[0],[0,H(b''),'e/x','extra'],[True,H(b'')],[-1,H(b'')],
            [ns['SOURCE_CAP']+1,H(b'')],[2**64,H(b'')],[0.5,H(b'')],['0',H(b'')],
            [0,None],[0,'A'*64],[0,'g'*64],[0,'0'*63],
            [0,H(b''),'../archive'],[0,H(b''),'/archive'],[0,H(b''),'e//x'],[0,H(b''),None]]:
 p=copy.deepcopy(v9_page);p['replacements'][0]['current']=bad
 check('v9_current_tuple_'+H(canonical(bad)),lambda p=p:refuses(lambda:ns['expand_v9_pins'](p)))
for bad in [[],['name'],['name',0],['name',0,H(b''),'e/x','extra'],[None,0,H(b'')],['name',True,H(b'')]]:
 p=copy.deepcopy(v9_page);p['replacements'][0]['views'][0]=bad
 check('v9_view_tuple_'+H(canonical(bad)),lambda p=p:refuses(lambda:ns['expand_v9_pins'](p)))
def v9_bounds():
 page=copy.deepcopy(v9_page);page['replacements'][0]['current']=[ns['SOURCE_CAP'],H(b'')]
 expanded=ns['expand_v9_pins'](page)
 assert expanded['replacements'][0]['current']['length']==ns['SOURCE_CAP']
 page=copy.deepcopy(v8_page);page['unknown_padding']='x'*ns['PAGE_CAP']
 refuses(lambda:ns['encode_v9_pins'](page))
check('v9_exact_source_bound_and_unchanged_page_cap',v9_bounds)
print(json.dumps(dict(status='PASS_PYTHON_PROTOCOL_FIXTURES_ONLY',checks=len(checks),names=checks,Rust_executed=False,full_source_guard_executed=False,binder_executed=False),indent=2))
