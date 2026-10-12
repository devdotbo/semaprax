//! Prelude presence is not declaration, descriptor or execution authority.
use super::*;
use crate::ast::{Expr, ExprKind, Program, Span, TypeDeclarationKind};
fn saved(program: &dyn crate::ast::program_source::ProgramSource) -> bool {
    let uses = |function: &crate::ast::Function| {
        let mut pending = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .collect::<Vec<_>>();
        while let Some(expr) = pending.pop() {
            if matches!(&expr.kind, crate::ast::ExprKind::Call { name, .. } if name == NAME) {
                return true;
            }
            let mut i = 0;
            while let Some(child) = expr.child(i) {
                pending.push(child);
                i += 1;
            }
        }
        false
    };
    program.functions().iter().any(uses)
        || program.types().iter().any(|ty| {
            matches!(&ty.kind, crate::ast::TypeDeclarationKind::Class { methods, .. }
                if methods.iter().any(uses))
        })
}
fn parsed(source: &str) -> Program {
    crate::parse(source, "unique-probe.spx").unwrap()
}
fn call(name: &str) -> Expr {
    Expr {
        span: Span::default(),
        kind: ExprKind::Call {
            name: name.to_owned(),
            type_arguments: Vec::new(),
            args: Vec::new(),
        },
    }
}
fn query(program: &Program, expected: bool) {
    assert_eq!(saved(program), expected);
    let (actual, observation) = crate::test_allocation_observer::measure(|| source_uses(program));
    assert_eq!(actual, expected);
    assert_eq!(observation.allocated, 0);
    assert_eq!(observation.allocations, 0);
}
#[test]
fn uniqueness_presence_query_lends_all_roots_and_class_methods_without_heap_frontiers() {
    let absent = parsed("module probe; @id(\"app.main\") fn main()->i64 {0}");
    query(&absent, false);
    let (_, old) = crate::test_allocation_observer::measure(|| saved(&absent));
    assert!(old.allocated >= std::mem::size_of::<&Expr>());
    assert_eq!(old.allocated, old.freed);
    const SOURCE: &str="module unique; @id(\"row\") record Row{@id(\"row.id\") id:i64,} @id(\"app.main\") fn main()->i64 {let rows=vec_with_capacity<Row>(0usize);let result=vec_unique_by<Row>(rows,\"id\");if result {0}else{1}}";
    let valid = crate::check(SOURCE, "unique-probe.spx").unwrap();
    query(&valid, true);
    let mut shapes = absent.clone();
    // Presence scans contracts/dead branches even if a later semantic gate
    // refuses the shape. It grants no validity to this deliberately raw AST.
    shapes.functions[0].requires.push(call(NAME));
    query(&shapes, true);
    shapes.functions[0].requires.clear();
    shapes.functions[0].ensures.push(call(NAME));
    query(&shapes, true);
    shapes.functions[0].ensures.clear();
    let mut nested = call(NAME);
    for _ in 0..240 {
        nested = Expr {
            span: Span::default(),
            kind: ExprKind::Unary {
                op: crate::ast::UnaryOp::Not,
                value: Box::new(nested),
            },
        };
    }
    shapes.functions[0].body = Expr {
        span: Span::default(),
        kind: ExprKind::If {
            condition: Box::new(Expr {
                span: Span::default(),
                kind: ExprKind::Bool(false),
            }),
            then_branch: Box::new(nested),
            else_branch: Box::new(call("vec_len")),
        },
    };
    query(&shapes, true);
    let mut class=parsed("module probe; @id(\"counter\") class Counter{@id(\"counter.n\") n:i64,@id(\"counter.read\") fn read(self:Counter)->i64 {self.n}} @id(\"app.main\") fn main()->i64{0}");
    query(&class, false);
    let TypeDeclarationKind::Class { methods, .. } = &mut class.types[0].kind else {
        panic!("class")
    };
    methods[0].ensures.push(call(NAME));
    query(&class, true);
    // Exact prior contracts remain selected for old programs; materializing
    // their contract bytes/digest is outside the zero-allocation probe scope.
    let legacy = crate::prelude::selected_for_program(&absent);
    assert_eq!(
        legacy,
        (
            crate::prelude::SCHEMA_V1,
            crate::prelude::contract_bytes_v1(),
            crate::prelude::digest_text_v1()
        )
    );
    let additive = crate::prelude::selected_for_program(&valid);
    assert_eq!(
        additive,
        (
            crate::prelude::SCHEMA_V18,
            crate::prelude::field_unique::contract_bytes(),
            crate::prelude::field_unique::digest_text()
        )
    );
    let vector=crate::check("module probe; @id(\"app.main\") fn main()->i64{let rows=vec_with_capacity<i64>(0usize);if vec_len<i64>(rows)==0usize{0}else{1}}","old-vector.spx").unwrap();
    query(&vector, false);
    assert_eq!(
        crate::prelude::selected_for_program(&vector),
        (
            crate::prelude::SCHEMA_V2,
            crate::prelude::contract_bytes_v2(),
            crate::prelude::digest_text_v2()
        )
    );
}
#[test]
fn source_probe_retains_saved_presence_beyond_the_fixed_visitor_depth() {
    let mut program = parsed("module probe; @id(\"app.main\") fn main()->i64{0}");
    for name in [NAME, "vec_len"] {
        let mut nested = call(name);
        for _ in 0..600 {
            nested = Expr {
                span: Span::default(),
                kind: ExprKind::Unary {
                    op: crate::ast::UnaryOp::Not,
                    value: Box::new(nested),
                },
            };
        }
        program.functions[0].body = nested;
        assert_eq!(source_uses(&program), saved(&program));
        assert_eq!(source_uses(&program), name == NAME);
    }
    // Existing AST overflow uses an owned frontier: no zero-allocation claim
    // or new depth refusal is attached to this non-admitted structural probe.
}
