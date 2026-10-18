from pathlib import Path
import ast,json,hashlib,re
R=Path('/Volumes/Semaprax/worktrees/next-large-combined-20261017');D=Path(__file__).parent;P='docs/evidence/885-c273-current444-174-20261018/successor174/bind-next.py';b=(R/P).read_bytes();s=b.decode();binder=(R/'docs/evidence/main09013-source-namespaces-v1/bind_current.py').read_text();tree=ast.parse(binder);node=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='mask');old=ast.get_source_segment(binder,node);new=old.replace('def mask(body):','def original_mask(body):',1)+'''\n\ndef mask(body):
 for field,value in (('LEN',b'<MANIFEST_LENGTH>'),('SHA',b'<MANIFEST_SHA256>')):
  pattern=(rb'const MAIN090_MANIFEST_LEN: usize = [0-9]+;' if field=='LEN' else rb'const MAIN090_MANIFEST_SHA: &str =(?: |\\n    )"[0-9a-f]{64}";')
  body,n=re.subn(pattern,value,body);require(n==1,'nonunique namespace pin '+field)
 return body'''
prior='docs/evidence/fmt-a110/QVrrvI1QE586vYpZYgkccKu1C-fpfJ47fkN9k0OI4aA.txt';current=(R/'src/source_reflows/incoming090_semantic_receivers.rs').read_bytes();ns={'re':re,'require':lambda c,m:None if c else (_ for _ in ()).throw(ValueError(m))};exec(new,ns);h=lambda b:hashlib.sha256(b).hexdigest();expected=h(ns['mask'](current));descriptor=json.loads((R/'docs/evidence/main09013-source-namespaces-v1/binding-descriptor.json').read_bytes());assert h(ns['original_mask']((R/prior).read_bytes()))==descriptor['masked_module_sha256'];assert h(current)=='77aec79d1e5eea8bbfebc4ee70c516027851d0d4003137e6e3f5364341a3b882'
x="source=current(d['module']);require(sha(mask(source))==d['masked_module_sha256'],'namespace body changed beyond two pins')"
y=f"source=current(d['module']);prior_source=current({prior!r});require(len(prior_source)==48349 and sha(prior_source)=='415aebbc8d50139f3abd8a5962091c70abb50be7e97c9e3b7e437d934388e1a0','whole preformat namespace drift');require(sha(original_mask(prior_source))==d['masked_module_sha256'],'original namespace body changed beyond two pins');require(len(source)==49904 and sha(source)=='77aec79d1e5eea8bbfebc4ee70c516027851d0d4003137e6e3f5364341a3b882' and sha(mask(source))=={expected!r},'exact formatted namespace drift')"
oldpins="require(('const MAIN090_MANIFEST_LEN: usize = '+str(len(before))+';').encode() in source and ('const MAIN090_MANIFEST_SHA: &str = \"'+sha(before)+'\";').encode() in source,'old module pins do not authenticate template')"
newpins="require(('const MAIN090_MANIFEST_LEN: usize = '+str(len(before))+';').encode() in source and (('const MAIN090_MANIFEST_SHA: &str = \"'+sha(before)+'\";').encode() in source or ('const MAIN090_MANIFEST_SHA: &str =\\n    \"'+sha(before)+'\";').encode() in source),'old module pins do not authenticate template')"
sha_pattern="rb'const MAIN090_MANIFEST_SHA: &str = \"[0-9a-f]{64}\";'"
new_pattern="rb'const MAIN090_MANIFEST_SHA: &str =(?: |\\n    )\"[0-9a-f]{64}\";'"
replacement="('const MAIN090_MANIFEST_SHA: &str = \"'+sha(after)+'\";').encode()"
newreplacement="('const MAIN090_MANIFEST_SHA: &str ='+('\\n    ' if b'const MAIN090_MANIFEST_SHA: &str =\\n    ' in source else ' ')+'\"'+sha(after)+'\";').encode()"
patches=[(old,new),(x,y),(oldpins,newpins),('updated,n=re.subn('+sha_pattern,'updated,n=re.subn('+new_pattern),(replacement,newreplacement)]
# A separate rendering layer retains the original render/full inverse intact.
addition='\ndef render_namespace_grammar(text):\n edits='+repr(patches)+'\n original=text\n for before,after in edits:\n  assert text.count(before)==1,(before,text.count(before));text=text.replace(before,after,1)\n restored=text\n for before,after in reversed(edits):assert restored.count(after)==1;restored=restored.replace(after,before,1)\n assert restored==original\n return text\n\n'
a="exec(compile(render(raw),str(D/ 'bind_current.before.py.txt'),'exec'),{";z="exec(compile(render_namespace_grammar(render(raw)),str(D/ 'bind_current.before.py.txt'),'exec'),{";assert s.count(a)==1;s=s.replace(a,z);assert s.count('def main():')==1;s=s.replace('def main():',addition+'def main():',1);ast.parse(s)
# Prove transformed unchanged original rendered binder still has all finite matches.
t=binder
for x,y in patches:assert t.count(x)==1,(x,t.count(x));t=t.replace(x,y,1)
ast.parse(t)
# Genuine original and formatted positive, exact missing/duplicate pin and body drift negatives.
results=[]
for name,raw,fn in [('original',(R/prior).read_bytes(),'original_mask'),('formatted',current,'mask')]:
 baseline=ns[fn](raw);results.append(name+' positive')
 for kind,bad in [('missing',raw.replace(b'const MAIN090_MANIFEST_LEN:',b'const WRONG_LEN:',1)),('duplicate',raw+b'\nconst MAIN090_MANIFEST_LEN: usize = 1;\n')]:
  try:ns[fn](bad)
  except ValueError:results.append(name+' '+kind+' refused')
  else:raise AssertionError(kind)
 assert ns[fn](raw+b'//foreign-body\n')!=baseline;results.append(name+' body drift refused')
(D/'bind-next.pre-mask.whole-before.py.txt').write_bytes(b);(D/'bind-next.mask-successor.py').write_text(s);(D/'namespace-mask-inverse.json').write_text(json.dumps(dict(before_sha256=h(b),after_sha256=h(s.encode()),edits=[dict(before=a,after=z),dict(before='def main():',after=addition+'def main():')],binder_render_edits=[dict(before=x,after=y)for x,y in patches],source_regressions=results,original_archive=prior,formatted_source_sha256=h(current)),indent=2)+'\n');print(h(s.encode()),results)
