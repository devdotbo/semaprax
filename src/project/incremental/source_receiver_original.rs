//! Complete original AST lookup and retention receivers. No shared key helper.
use super::*;
struct OriginalSourceReceiver {
    entries: BTreeMap<String, Arc<OriginalCachedModule>>,
    reused: usize,
    reused_bytes: usize,
    resolved: usize,
    checked_reused: usize,
    semantic: bool,
    next_checked: BTreeMap<String, Arc<CheckedModule>>,
    retained_source_bytes: usize,
    ast_budget: usize,
}
struct OriginalCachedModule {
    source: String,
    program: Arc<Program>,
}
impl OriginalSourceReceiver {
    pub(crate) fn lookup(&mut self, path: &str, source: &str) -> Option<Program> {
        let entry = self.entries.get(path)?;
        if entry.source != source || entry.program.path != path {
            return None;
        }
        self.reused += 1;
        self.reused_bytes += source.len();
        Some(entry.program.as_ref().clone())
    }
    /// Borrow the exact compiler-created source AST retained by this pass.
    /// Unlike lookup, this post-admission use does not clone or count a cache hit.
    pub(crate) fn retained_source_program(&self, path: &str, source: &str) -> Result<&Program> {
        self.entries
            .get(path)
            .filter(|entry| entry.source == source && entry.program.path == path)
            .map(|entry| entry.program.as_ref())
            .ok_or_else(|| invalid("frontend retained source AST does not match Project source"))
    }

    pub(crate) fn retain(
        &mut self,
        sources: &[WorkspaceSource],
        programs: Vec<Program>,
        ast_budget: usize,
    ) -> Result<()> {
        if sources.len() != programs.len()
            || programs.len() > MAX_PROJECT_FRONTEND_CACHE_MODULES
            || ast_budget > MAX_PROJECT_FRONTEND_CACHE_AST_BUDGET
        {
            return Err(capacity(
                "frontend retained AST inventory exceeds its bound",
            ));
        }
        let mut entries = BTreeMap::new();
        let mut bytes = 0usize;
        for (source, program) in sources.iter().zip(programs) {
            if source.path != program.path {
                return Err(invalid("frontend AST source origin disagrees"));
            }
            bytes = bytes
                .checked_add(source.source.len())
                .ok_or_else(|| capacity("frontend retained source accounting overflow"))?;
            if bytes > MAX_PROJECT_FRONTEND_CACHE_SOURCE_BYTES {
                return Err(capacity(
                    "frontend retained source inventory exceeds its byte bound",
                ));
            }
            let entry = if let Some(existing) = self
                .entries
                .get(&source.path)
                .filter(|entry| entry.source == source.source)
            {
                Arc::clone(existing)
            } else {
                Arc::new(OriginalCachedModule {
                    source: source.source.clone(),
                    program: Arc::new(program),
                })
            };
            entries.insert(source.path.clone(), entry);
        }
        if self.resolved + self.checked_reused != entries.len()
            || (self.semantic && self.next_checked.len() != entries.len())
        {
            return Err(invalid(
                "frontend checked module inventory disagrees with admitted modules",
            ));
        }
        self.entries = entries;
        self.retained_source_bytes = bytes;
        self.ast_budget = ast_budget;
        Ok(())
    }
}

#[test]
fn complete_original_source_receivers_keep_owned_clone_and_same_source_retention() {
    let source = WorkspaceSource {
        path: "original.spx".to_owned(),
        source: "module original; @id(\"original.main\") fn main()->i64 {0}".to_owned(),
    };
    let program = crate::parse(&source.source, &source.path).unwrap();
    let mut old = OriginalSourceReceiver {
        entries: BTreeMap::new(),
        reused: 0,
        reused_bytes: 0,
        resolved: 1,
        checked_reused: 0,
        semantic: false,
        next_checked: BTreeMap::new(),
        retained_source_bytes: 0,
        ast_budget: 0,
    };
    assert!(old.lookup(&source.path, &source.source).is_none());
    old.retain(std::slice::from_ref(&source), vec![program.clone()], 0)
        .unwrap();
    let owner = Arc::clone(&old.entries[&source.path]);
    assert_eq!(
        old.retained_source_program(&source.path, &source.source)
            .unwrap(),
        &program
    );
    let cloned = old.lookup(&source.path, &source.source).unwrap();
    assert_eq!(cloned, program);
    assert_ne!(cloned.functions.as_ptr(), owner.program.functions.as_ptr());
    assert!(old.lookup(&source.path, "different bytes").is_none());
    old.retain(std::slice::from_ref(&source), vec![program], 0)
        .unwrap();
    assert!(Arc::ptr_eq(&owner, &old.entries[&source.path]));
    assert_eq!((old.reused, old.reused_bytes), (1, source.source.len()));
    assert_eq!(old.retained_source_bytes, source.source.len());
}

#[test]
fn full_original_source_receiver_bodies_are_exact_saved_text() {
    let old = include_str!("../../../docs/evidence/immutable-cache-source-split/src__project__incremental.rs.before.txt");
    let current = include_str!("source_receiver_original.rs");
    for name in ["lookup", "retained_source_program", "retain"] {
        let marker = format!("    pub(crate) fn {name}(");
        let body = |source: &str| {
            let start = source.find(&marker).unwrap();
            let end = start + source[start..].find("\n    }").unwrap() + 6;
            source[start..end].to_owned()
        };
        assert_eq!(
            body(old),
            body(current).replace("OriginalCachedModule {", "CachedModule {")
        );
    }
}
