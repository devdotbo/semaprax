"""Exact formatted FINAL6 grammar adapter; original producer retained whole."""
from pathlib import Path
import os,stat,hashlib,subprocess
R=Path('/Volumes/Semaprax/worktrees/next-large-combined-20261017')
P=R/'docs/evidence/ordinary-plan-final-ingress-20261017/bind-current-inputs.py'
fd=os.open(P,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
try:
 st=os.fstat(fd);assert stat.S_ISREG(st.st_mode)and st.st_size==9387
 with os.fdopen(fd,'rb',closefd=False)as f:raw=f.read(st.st_size);assert len(raw)==st.st_size and not f.read(1)
 ident=lambda x:(x.st_dev,x.st_ino,x.st_mode,x.st_size,x.st_mtime_ns,x.st_ctime_ns)
 assert ident(os.fstat(fd))==ident(P.lstat())==ident(st)
finally:os.close(fd)
assert hashlib.sha256(raw).hexdigest()=='cb1a3b7568fa0490b7fabf826d3f4389d64d786ea1b62ea7aa0c242c0ea45562' and raw==subprocess.check_output(['git','show','HEAD:docs/evidence/ordinary-plan-final-ingress-20261017/bind-current-inputs.py'],cwd=R)
edits=[('def scrub(code):\n code,n=re.subn(r\'const FINAL_INGRESS_LENGTH: usize = [0-9]+;\',\'const FINAL_INGRESS_LENGTH: usize = SOURCE_LENGTH;\',code);assert n==1\n code,n=re.subn(r\'const FINAL_INGRESS_SHA256: &str = "[0-9a-f]{64}";\',\'const FINAL_INGRESS_SHA256: &str = "SOURCE_SHA256";\',code);assert n==1\n return code', 'def original_scrub(code):\n code,n=re.subn(r\'const FINAL_INGRESS_LENGTH: usize = [0-9]+;\',\'const FINAL_INGRESS_LENGTH: usize = SOURCE_LENGTH;\',code);assert n==1\n code,n=re.subn(r\'const FINAL_INGRESS_SHA256: &str = "[0-9a-f]{64}";\',\'const FINAL_INGRESS_SHA256: &str = "SOURCE_SHA256";\',code);assert n==1\n return code\n\ndef scrub(code):\n code,n=re.subn(r\'const FINAL_INGRESS_LENGTH: usize = [0-9]+;\',\'const FINAL_INGRESS_LENGTH: usize = SOURCE_LENGTH;\',code);assert n==1\n code,n=re.subn(r\'const FINAL_INGRESS_SHA256: &str =(?: |\\n    )"[0-9a-f]{64}";\',\'const FINAL_INGRESS_SHA256: &str = "SOURCE_SHA256";\',code);assert n==1\n return code'), ('r\'const FINAL_INGRESS_SHA256: &str = "([0-9a-f]{64})";\'', 'r\'const FINAL_INGRESS_SHA256: &str =(?: |\\n    )"([0-9a-f]{64})";\''), ("helper_path=R/spec['helper'];helper=read(helper_path).decode();assert scrub(helper)==scrub(template),'unexpected source helper drift'", "helper_path=R/spec['helper'];helper=read(helper_path).decode();prior_helper=read(R/'docs/evidence/fmt-a110/v8M15AIpSv-8kL6VHWd_Go87kMGIdJ7QfMfFIMH5XH4.txt',18589,'bfc335e402294affbc90be951d677f1a8f3b90c188749ed07cc7c520c1f95c7e').decode();assert original_scrub(prior_helper)==original_scrub(template),'original helper body drift';assert len(helper.encode())==19742 and sha(helper.encode())=='67daafbc387f420b4372ab4d5c59d301cae72fcdfd96cb8803533823db0cb8b3' and sha(scrub(helper).encode())=='43e676b2007cf196bd5ba38c2ac97c075af830986fff80c680038b0fb5a466b3','exact formatted helper drift'"), ('code,n=re.subn(r\'const FINAL_INGRESS_SHA256: &str = "[0-9a-f]{64}";\',f\'const FINAL_INGRESS_SHA256', 'code,n=re.subn(r\'const FINAL_INGRESS_SHA256: &str =(?: |\\n    )"[0-9a-f]{64}";\',f\'const FINAL_INGRESS_SHA256'), ('f\'const FINAL_INGRESS_SHA256: &str = "{sha(packet)}";\'', '(\'const FINAL_INGRESS_SHA256: &str =\'+(\'\\n    \' if \'const FINAL_INGRESS_SHA256: &str =\\n    \' in helper else \' \')+f\'"{sha(packet)}";\')'), ('assert scrub(code)==scrub(template)and len(code.splitlines())<=1500', 'assert scrub(code)==scrub(helper)and original_scrub(prior_helper)==original_scrub(template)and len(code.splitlines())<=1500')]
s=raw.decode()
for before,after in edits:assert s.count(before)==1;s=s.replace(before,after,1)
back=s
for before,after in reversed(edits):assert back.count(after)==1;back=back.replace(after,before,1)
assert back.encode()==raw
exec(compile(s,str(P),'exec'),{'__name__':'__main__','__file__':str(P)})
