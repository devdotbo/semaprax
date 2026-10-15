from pathlib import Path
import json,hashlib
base=Path(__file__).resolve().parent
root=base.parents[2]
d=json.loads((base/'exact-inverses.json').read_text())
for source,hooks,archive in [('src/ast.rs','whole_ast_hooks','ast.rs.before.txt'),('src/ast/iterative_program_drop_tests.rs','whole_current_test_hooks','iterative-program-drop-tests.before.txt')]:
 current=(root/source).read_text()
 for added,original in d[hooks]:
  assert current.count(added)==1
  current=current.replace(added,original,1)
 assert current==(base/archive).read_text()
original=(root/'src/ast/drop_method_original.rs').read_text()
start=original.index('impl Drop for Original {')
end=original.index('// This lexical adapter')
assert original[start:end].strip().replace('impl Drop for Original {','impl Drop for Program {',1)==(base/'program-drop.before.txt').read_text().strip()
binding=json.loads((base/'source-binding.json').read_text())
for entry in binding['complete_old_archives_untouched']:
 assert hashlib.sha256((root/entry['path']).read_bytes()).hexdigest()==entry['sha256']
print('PASS complete AST/test source reconstruction; complete original/prior Drop and deep receiver retained')
