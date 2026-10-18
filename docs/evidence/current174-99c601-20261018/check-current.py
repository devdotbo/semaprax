"""Replay current physical174 with original bounded FD/Git readers; never publish."""
from pathlib import Path
import argparse,copy,hashlib,json,os,re,runpy
p=argparse.ArgumentParser();p.add_argument('--root',required=True,type=Path);p.add_argument('--expected-head',required=True);a=p.parse_args()
R=a.root.absolute();assert R==R.resolve() and R.is_relative_to('/Volumes/Semaprax') and re.fullmatch('[0-9a-f]{40}',a.expected_head) and __debug__
D=Path(__file__).resolve().parent;S=D/'successor174';adapter=runpy.run_path(str(S/'bind-next.py'),run_name='actual174_reader_definitions')
raw=adapter['held'](S/'bind_current.before.py.txt',adapter['BINDER_LEN'],adapter['BINDER_SHA']);code=adapter['render_namespace_grammar'](adapter['render'](raw))
ns={'__name__':'actual174_current_physical_reader','__file__':str(S/'bind_current.before.py.txt')};exec(compile(code,ns['__file__'],'exec'),ns)
fd=os.open(R,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW|os.O_CLOEXEC);ns.update(R=R,EXPECTED=a.expected_head,ROOT_FD=fd,ROOT_ID=ns['identity'](os.fstat(fd)))
try:
 require=ns['require'];current=ns['current'];parse=ns['parse'];sha=ns['sha'];require(ns['head']()==a.expected_head,'explicit HEAD mismatch')
 relative=lambda path:str(path.relative_to(R))
 require(current(relative(S/'bind-next.py'))==(S/'bind-next.py').read_bytes(),'actual successor program differs from Git')
 descriptor_raw=current(ns['DESCRIPTOR']);require(len(descriptor_raw)==ns['DESCRIPTOR_LEN'] and sha(descriptor_raw)==ns['DESCRIPTOR_SHA'],'immutable descriptor drift');descriptor=parse(descriptor_raw)
 receipt=parse(current(relative(D/'actual-writer-receipt.json')));require(receipt['status']=='ACTUAL_ORIGINAL174_PUBLISHER_COMPLETED_SOURCE_ONLY_NOT_RUST_QUALIFIED','not an actual writer receipt');producer_head=receipt['head'];require(re.fullmatch('[0-9a-f]{40}',producer_head) is not None,'foreign producer head');ns['git']('merge-base','--is-ancestor',producer_head,a.expected_head)
 outputs={row['path']:row for row in receipt['outputs']};require(set(outputs)=={descriptor['manifest'],descriptor['module']},'foreign output set')
 actual={}
 for path,pin in outputs.items():
  data=current(path);require((len(data),sha(data))==(pin['length'],pin['sha256']),'actual produced output drift: '+path);actual[path]=data
 manifest=parse(actual[descriptor['manifest']]);namespace=actual[descriptor['module']]
 require((len(manifest['main']),len(manifest['physical']),len(manifest['legacy_current']),len(manifest['expected']))==(173,174,149,3),'original domain counts changed')
 length=re.findall(rb'const MAIN090_MANIFEST_LEN: usize = ([0-9]+);',namespace);digest=re.findall(rb'const MAIN090_MANIFEST_SHA: &str =(?: |\n    )"([0-9a-f]{64})";',namespace);require(len(length)==len(digest)==1 and int(length[0])==len(actual[descriptor['manifest']]) and digest[0].decode()==sha(actual[descriptor['manifest']]),'actual namespace pins do not bind complete manifest')
 require(sha(ns['mask'](namespace))=='ddfcff89f17fee40e83fe61a26c0e99e91e9b2106d8250b31e874410e0aba545','namespace body changed beyond original two pins')
 prior_raw=current(relative(S/'input-domains.before.json'));require(current(receipt['previous_manifest_archive'])==prior_raw,'whole preceding manifest archive drift');prior=parse(prior_raw);expected=copy.deepcopy(prior);physical={}
 for row in manifest['physical']:
  path=row['path'];require(path not in physical,'duplicate physical subject');data=current(path);pair=(len(data),sha(data));require(pair==(row['current_len'],row['current_sha256']),'stale physical subject: '+path);physical[path]=pair
 require({row['path']for row in manifest['main']}<=set(physical),'missing main domain')
 for domain in ('main','physical'):
  for row in expected[domain]:
   row['current_len'],row['current_sha256']=physical[row['path']]
   if 'binding_status'in row:row['binding_status']='assembled'
 expected['unassembled']=[];expected['bound_final_head']=producer_head
 require(manifest==expected,'whole original producer transformation or unknown fields drifted');ns['recheck']()
 print(json.dumps(dict(status='SOURCE_ONLY_CURRENT174_PHYSICAL_REPLAY_PASS',head=a.expected_head,actual_producer_invocation_head=producer_head,physical_subjects=174,main_subjects=173,all_current_sources_equal_original_bounded_Git_acquisitions=True,whole_manifest_matches_original_transform=True,namespace_body_and_binding_pins_match_actual_output=True,runtime='UNEXECUTED',publisher_reexecuted=False)))
finally:os.close(fd)
