"""One literal page, composed after predecessor metadata and before raw admission."""
import copy, hashlib, json
PAGE_CAP = 262144
SOURCE_CAP = 9753094
POLICY_PATH = 'docs/evidence/finite-source-overlay-v2-20261018/policy.json'
PAGE_PATH = 'docs/evidence/finite-source-overlay-v2-20261018/page.json'
POLICY_LENGTH = 554
POLICY_SHA256 = '76f080a9e7b21aa25e317a0b41e75c2c40f3fcda6e2b3b59d82ba76f4516a175'
EXTENSION_PATH = 'docs/evidence/fmt-local-441-20261018/extension.json'
EXTENSION_LENGTH = 262076
EXTENSION_SHA256 = '2af7299253e5cfb027186ba4fc151011a937214ff381c9267bfc11ef7d0003b6'
REPLACEMENTS = ()
ADDITIONS = ()
CONTROL = {POLICY_PATH, PAGE_PATH, EXTENSION_PATH,
 'src/source_reflows_merge/main_e177_final_inputs.rs',
 'src/source_reflows_merge/main_e177_overlay_v2.rs',
 'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py',
 'docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py',
 'docs/evidence/merge-2e92-browser-20261018/guard-config.json'}
H = lambda b: hashlib.sha256(b).hexdigest()
def canonical(value):
 return (json.dumps(value, sort_keys=True, ensure_ascii=False, allow_nan=False, separators=(',', ':'))+'\n').encode()
def parse(raw):
 def unique(pairs):
  result = {}
  for key,value in pairs:
   assert key not in result, 'duplicate JSON key'
   result[key] = value
  return result
 value = json.loads(raw, object_pairs_hook=unique)
 assert canonical(value) == raw, 'noncanonical JSON'
 return value
def keys(value, expected):
 return type(value) is dict and set(value) == set(expected)
def normal(path):
 return type(path) is str and path and not any(c in path for c in ('\\', ':', '\0')) and all(p and p not in ('.','..') for p in path.split('/'))
def pin(value):
 length,sha,path = value['length'],value['sha256'],value['archive']
 assert type(length) is int and 0 <= length <= SOURCE_CAP
 assert type(sha) is str and len(sha)==64 and all(c in '0123456789abcdef' for c in sha)
 assert normal(path)
 return length,sha,path
def edges(packet):
 rows = packet['rows']; by = {r['source']:r for r in rows}
 assert len(by)==len(rows) and all(type(p) is str and p for p in by)
 result=[]
 for row in rows:
  for child in row.get('fragments',[]):
   assert 'fragments' not in child and child['compiled'] is True
   assert by[child['source']]==child and row['source']!=child['source']
   edge=(row['source'],child['source']); assert edge not in result; result.append(edge)
 return result
def compose_values(before, policy, page):
 assert keys(page, ('schema','index','total_pages','predecessor','replacements','additions'))
 assert page['schema']=='semaprax.finite-source-overlay-page.v2' and type(page['index']) is int and page['index']==1 and type(page['total_pages']) is int and page['total_pages']==1
 assert page['predecessor']==policy['predecessor']
 assert H(canonical(before))==policy['predecessor']['logical_sha256']
 old=before['rows']; assert len(old)==policy['predecessor']['rows'] and before['source_cap']==SOURCE_CAP
 positions={r['source']:i for i,r in enumerate(old)}; assert len(positions)==len(old)
 result=copy.deepcopy(before); rows=result['rows']; seen=set(); previous=-1
 assert len(page['replacements'])==len(policy['replacements'])
 for row,source in zip(page['replacements'],policy['replacements']):
  assert normal(source) and source not in CONTROL and row['source']==source and source not in seen; seen.add(source)
  index=positions[source]; assert previous < index; previous=index; prior=old[index]
  assert type(row['compiled']) is bool and row['compiled']==prior['compiled']
  views=row['views']; old_views=prior['views']; assert views[:len(old_views)]==old_views and len(views)>=len(old_views)
  pin(row['current'])
  for view in views[len(old_views):]:pin(view)
  if row['current']!=prior['current']:assert dict(name='whole-pre-overlay-v2-current',**prior['current']) in views[len(old_views):]
  restored=copy.deepcopy(row); restored['current']=prior['current']; restored['views']=old_views
  if 'fragments' in prior:restored['fragments']=prior['fragments']
  else:restored.pop('fragments',None)
  assert restored==prior, 'whole row inverse/unknown fields'; rows[index]=copy.deepcopy(row)
 assert len(page['additions'])==len(policy['additions'])
 for row,binding in zip(page['additions'],policy['additions']):
  assert keys(binding, ('source','compiled')); source=binding['source']
  assert normal(source) and source not in CONTROL and row['source']==source and source not in positions and source not in seen; seen.add(source)
  assert type(binding['compiled']) is bool and type(row['compiled']) is bool and row['compiled']==binding['compiled']
  pin(row['current'])
  for view in row['views']:pin(view)
  rows.append(copy.deepcopy(row))
 assert len(rows)==len(old)+len(page['additions'])
 old_edges=edges(before); new_edges=edges(result)
 assert [e for e in new_edges if e in old_edges]==old_edges
 assert [dict(parent=p,child=c) for p,c in new_edges if (p,c) not in old_edges]==policy['new_fragment_edges']
 restored=copy.deepcopy(result); restored['rows']=copy.deepcopy(old); assert restored==before, 'ENTIRE predecessor object inverse'
 return result

class Overlay:
 def __init__(self, root, bounded, read, configuration_current, predecessor, stage_rows):
  self.root=root; self.bounded=bounded; self.read=read; self.configuration_current=configuration_current
  self.predecessor=predecessor; self.stage_rows=stage_rows
 def load(self):
  self.configuration_current()
  assert POLICY_LENGTH<=PAGE_CAP
  policy=parse(self.bounded(self.root/POLICY_PATH,POLICY_LENGTH,POLICY_SHA256))
  assert keys(policy, ('schema','page','predecessor','replacements','additions','new_fragment_edges'))
  assert policy['schema']=='semaprax.finite-source-overlay-policy.v2'
  p=policy['page']; assert keys(p,('path','length','sha256')) and p['path']==PAGE_PATH
  pred=policy['predecessor']; assert keys(pred,('extension','rows','logical_sha256')) and pred['rows']==519
  assert pred['extension']==dict(path=EXTENSION_PATH,length=EXTENSION_LENGTH,sha256=EXTENSION_SHA256)
  self.bounded(self.root/EXTENSION_PATH,EXTENSION_LENGTH,EXTENSION_SHA256)
  assert policy['replacements']==list(REPLACEMENTS) and [r['source'] for r in policy['additions']]==list(ADDITIONS)
  assert type(p['length']) is int and 0<=p['length']<=PAGE_CAP
  page=parse(self.bounded(self.root/PAGE_PATH,p['length'],p['sha256']))
  return policy,page
 def compose(self, before):
  policy,page=self.load(); return compose_values(before,policy,page)
 def affected_row(self, source):
  if source not in REPLACEMENTS and source not in ADDITIONS:return None
  packet=self.compose(self.predecessor())
  return next(r for r in packet['rows'] if r['source']==source)
 def current(self,row):
  length,sha,_=pin(row['current'])
  # Keep bounded()'s original Python per-read cap; no aggregate bound or
  # oversized-source splitting is introduced by the metadata page.
  actual=self.bounded(self.root/row['source'],length,sha)
  assert actual==self.read(row['current'])
  for child in row.get('fragments',[]):self.current(child)
  return actual
 def stage_source(self, source, requested):
  row=self.affected_row(source)
  if row is None:return None
  actual=self.current(row)
  if (requested['length'],requested['sha256'])==(row['current']['length'],row['current']['sha256']):return actual
  # Only literal authenticated predecessor stage metadata can supply an
  # additional historical pin. No caller archive/path is used as authority.
  original=next((r for r in self.stage_rows() if r['source']==source),None)
  choices=[row['current'],*row['views']]
  if original is not None:choices.extend([original['before'],original['current'],*original.get('views',[])])
  matches=[p for p in choices if (p['length'],p['sha256'])==(requested['length'],requested['sha256'])]
  assert matches, 'foreign overlay tuple'
  prior=self.read(matches[0]); assert all(self.read(p)==prior for p in matches)
  return prior
