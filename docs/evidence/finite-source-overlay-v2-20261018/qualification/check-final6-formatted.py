"""Pure original FINAL6 controls plus exact formatted current helper binding."""
from pathlib import Path
import os,stat,hashlib,re,subprocess
R=Path('/Volumes/Semaprax/worktrees/focused146-structural-repairs-20261018')
P=R/'docs/evidence/accepted-final6-source-binding-20261017/check-source.py'
fd=os.open(P,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
try:
 st=os.fstat(fd);assert stat.S_ISREG(st.st_mode)and st.st_size==2253
 with os.fdopen(fd,'rb',closefd=False)as f:raw=f.read(st.st_size);assert len(raw)==st.st_size and not f.read(1)
 ident=lambda x:(x.st_dev,x.st_ino,x.st_mode,x.st_size,x.st_mtime_ns,x.st_ctime_ns)
 assert ident(os.fstat(fd))==ident(P.lstat())==ident(st)
finally:os.close(fd)
assert hashlib.sha256(raw).hexdigest()=='2b3f41f3ee3643468e2229a5097d71deb0994758fc3c9984c6349ea7b405ea0e' and raw==subprocess.check_output(['git','show','HEAD:docs/evidence/accepted-final6-source-binding-20261017/check-source.py'],cwd=R)
def _formatted_pins(code):
 length=re.findall(r'const FINAL_INGRESS_LENGTH: usize = ([0-9]+);',code)
 digest=re.findall(r'const FINAL_INGRESS_SHA256: &str =(?: |\n    )"([0-9a-f]{64})";',code)
 assert len(length)==len(digest)==1
 return int(length[0]),digest[0]
edits=[("helper=b.read(R/recipe['helper']).decode();template=", "actual_helper=b.read(R/recipe['helper']).decode();assert actual_helper.encode()==git('show',a.expected_head+':'+recipe['helper']);assert len(actual_helper.encode())==19742 and sha(actual_helper.encode())=='c470186c3d0aa988a34d85218f3417c2d83a455f8c6cf7588e65a05776499fb6';helper=b.read(R/'docs/evidence/fmt-a110/v8M15AIpSv-8kL6VHWd_Go87kMGIdJ7QfMfFIMH5XH4.txt',18589,'bfc335e402294affbc90be951d677f1a8f3b90c188749ed07cc7c520c1f95c7e').decode();template="), ("length,digest=b.pins(helper);b.read(D/'current-inputs.json',length,digest)", "length,digest=_formatted_pins(actual_helper);b.read(D/'current-inputs.json',length,digest)"), ("if 'ledger_capacity'in path:assert len(raw.splitlines())==1500", "if 'ledger_capacity'in path:\n   assert path=='crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs'\n   original_native=b.read(R/'docs/evidence/main-e177-final-source-binding-20261017/views/08dc10fd9d158b8bb35bd6e7659f50a2d849638aa79537200282b15fcbec83df.txt',165305,'08dc10fd9d158b8bb35bd6e7659f50a2d849638aa79537200282b15fcbec83df').decode()\n   assert original_native.encode()==git('show','530073a528dcd226bb9260627da189a1fffcf8a8:'+path)\n   assert len(original_native.splitlines())==1500\n   assert len(raw.splitlines())<=1500")]
def _format_qualifier(code):
 original=code
 for before,after in edits:assert code.count(before)==1;code=code.replace(before,after,1)
 back=code
 for before,after in reversed(edits):assert back.count(after)==1;back=back.replace(after,before,1)
 assert back==original
 return code
s=raw.decode();before="exec(compile(current,str(P),'exec'),{'__file__':str(P),'__name__':'__main__','_accepted_guard_extensions':ns['accepted_guard_extensions']})";after="exec(compile(_format_qualifier(current),str(P),'exec'),{'__file__':str(P),'__name__':'__main__','_accepted_guard_extensions':ns['accepted_guard_extensions'],'_formatted_pins':_formatted_pins})"
assert s.count(before)==1;s=s.replace(before,after,1);assert s.replace(after,before,1).encode()==raw
exec(compile(s,str(P),'exec'),{'__file__':str(P),'__name__':'__main__','_format_qualifier':_format_qualifier,'_formatted_pins':_formatted_pins})
