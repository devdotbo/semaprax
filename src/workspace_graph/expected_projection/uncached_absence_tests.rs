    use super::{fixed_hir_discount, StructuralCost};

    // Independent saved modes-5/6 formula: the former literal-only ceiling.
    fn old_discount(raw: &StructuralCost, runtime: &StructuralCost) -> usize {
        raw.literal_fixed_discount + raw.exact_fixed_discount + runtime.exact_fixed_discount
    }
    fn raw(body: &str) -> StructuralCost {
        let source = format!("module absence; @id(\"absence.main\") fn main() -> i64 {{ {body} }}");
        let program = crate::parse(&source, std::path::Path::new("absence.spx")).unwrap();
        let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
        super::super::declaration_cost::ast_expr_cost(&program.functions[0].body, &mut cost)
            .unwrap();
        cost
    }
    fn absent_storage() -> usize {
        // Independent actual-layout oracle, not the implementation constant.
        std::mem::size_of::<crate::cleanup::CleanupStorageSlot>()
            + std::mem::size_of::<crate::cleanup::CleanupFlag>()
            + std::mem::size_of::<crate::cleanup::CleanupPlace>()
            + std::mem::size_of::<crate::cleanup_plan::CleanupSlot>()
    }
    fn carriers() -> usize {
        // Independent actual-layout oracle for the six auxiliary carriers.
        std::mem::size_of::<crate::hir::ResolvedBinding>()
            + std::mem::size_of::<crate::hir::ResolvedStatement>()
            + std::mem::size_of::<crate::hir::ResolvedFieldInitializer>()
            + std::mem::size_of::<crate::hir::ResolvedMatchArm>()
            + std::mem::size_of::<crate::hir::ResolvedMatchPatternField>()
            + std::mem::size_of::<crate::hir::ResolvedRecordMatchPatternField>()
    }

    #[test]
    fn uncached_modes_remove_only_one_copy_of_proved_operator_root_storage() {
        // Hand-counted carrier-free roots: block tails, operands, `if`
        // conditions and call arguments. `let` values stay delegated. `if`
        // roots in those positions are counted apart: they keep storage.
        for (body, census, free, ifs, vars, blocks) in [
            ("1", [1, 0, 0], 1, 0, 0, 0),
            ("-(7 / 2) + (1 - 3)", [4, 1, 3], 8, 0, 0, 0),
            (
                "let a = text(); if a == \"s\" && !flag { -(7 / 2) } else { (1 - 3) * f(2) }",
                [5, 2, 5],
                12,
                1,
                2,
                2,
            ),
            ("let x = 1 + 2; let y = -x; y", [2, 1, 1], 2, 0, 2, 0),
            // The `let` value `if` is delegated; the tail, the `else if`
            // (a block tail) and the call-argument `if` are free.
            (
                "let v = if flag { 1 } else { 2 }; if v > 0 { v } else if flag { 3 } else { g(if flag { 4 } else { -v }) }",
                [5, 1, 1],
                7,
                3,
                6,
                8,
            ),
        ] {
            let raw = raw(body);
            assert_eq!(raw.exact_absent_roots, census);
            assert_eq!(raw.exact_carrier_free_roots, free);
            assert_eq!(raw.exact_carrier_free_ifs, ifs, "{body}");
            assert_eq!(raw.exact_carrier_free_vars, vars, "{body}");
            assert_eq!(raw.exact_carrier_free_blocks, blocks, "{body}");
            let mut runtime = StructuralCost::new();
            runtime.exact_fixed_discount = 71;
            // Even a hostile runtime census cannot remove runtime storage.
            runtime.exact_absent_roots = [usize::MAX; 3];
            runtime.exact_carrier_free_roots = usize::MAX;
            runtime.exact_carrier_free_ifs = usize::MAX;
            runtime.exact_carrier_free_vars = usize::MAX;
            runtime.exact_carrier_free_blocks = usize::MAX;
            let old = old_discount(&raw, &runtime);
            for mode in [7, 8] {
                assert_eq!(fixed_hir_discount(mode, &raw, &runtime).unwrap(), old);
            }
            // An `if` root keeps its four storage footprints.
            let additional =
                (census[1] + census[2]) * absent_storage() + (free + ifs + vars + blocks) * carriers();
            for mode in [5, 6] {
                assert_eq!(
                    fixed_hir_discount(mode, &raw, &runtime).unwrap(),
                    old + additional
                );
                let original_upper = old + additional + 123;
                assert_eq!(
                    original_upper - fixed_hir_discount(mode, &raw, &runtime).unwrap(),
                    123
                );
                // No semantic-cache second copy, literal double count, or
                // removed transitions/edges/control/identity backing.
                assert_eq!(
                    old - raw.literal_fixed_discount,
                    raw.exact_fixed_discount + 71
                );
            }
            for mode in [0, 1, 2, 3, 4] {
                assert_eq!(
                    fixed_hir_discount(mode, &raw, &runtime).unwrap(),
                    raw.literal_fixed_discount
                );
            }
        }
    }

    #[test]
    fn uncached_absence_rejects_malformed_census_and_preserves_exact_limit() {
        let mut raw = raw("-(7 / 2) + (1 - 3)");
        let runtime = StructuralCost::new();
        let expected = old_discount(&raw, &runtime) + 4 * absent_storage() + 8 * carriers();
        struct Restore(usize);
        impl Drop for Restore {
            fn drop(&mut self) {
                crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(self.0));
            }
        }
        let previous =
            crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(expected));
        let _restore = Restore(previous);
        for mode in [5, 6] {
            assert_eq!(fixed_hir_discount(mode, &raw, &runtime).unwrap(), expected);
            crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(expected - 1));
            assert!(fixed_hir_discount(mode, &raw, &runtime)
                .unwrap_err()
                .iter()
                .all(|error| error.code == "SPX-G171"));
            crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(expected));
        }
        let census = raw.exact_absent_roots;
        // Nine expressions: the body block and eight censused roots.
        assert_eq!(raw.exact_expressions, 9);
        // A carrier-free count above its storage census, an `if` count that
        // does not fit beside it, or bytes that overflow, refuses instead of
        // saturating into a larger discount.
        for (malformed, free, ifs) in [
            ([usize::MAX, 1, 0], 0, 0),
            ([0, 0, raw.exact_expressions + 1], 0, 0),
            (census, 9, 0),
            ([0, 0, 0], 1, 0),
            (census, usize::MAX, 0),
            (census, 8, 2),
            (census, 0, usize::MAX),
            ([0, 0, 0], 0, raw.exact_expressions + 1),
        ] {
            raw.exact_absent_roots = malformed;
            raw.exact_carrier_free_roots = free;
            raw.exact_carrier_free_ifs = ifs;
            for mode in [5, 6] {
                assert!(fixed_hir_discount(mode, &raw, &runtime)
                    .unwrap_err()
                    .iter()
                    .all(|error| error.code == "SPX-G171"));
            }
        }
        // Counts that fit beside a huge expression census still refuse when
        // their carrier bytes overflow.
        // `self::raw`: the local `raw` shadows the fixture helper here.
        let mut huge = self::raw("1");
        huge.exact_expressions = usize::MAX;
        huge.exact_carrier_free_ifs = usize::MAX / carriers() + 1;
        for mode in [5, 6] {
            assert!(fixed_hir_discount(mode, &huge, &runtime)
                .unwrap_err()
                .iter()
                .all(|error| error.code == "SPX-G171"));
        }
        // A real free `if`: body block, `if`, `flag`, two branch blocks and
        // two literal tails. Literals, `if` and `flag` omit carriers; only
        // the literals omit storage (already in the literal discount).
        let branch = self::raw("if flag { 1 } else { 2 }");
        assert_eq!(branch.exact_expressions, 7);
        assert_eq!(
            (
                branch.exact_carrier_free_roots,
                branch.exact_carrier_free_ifs,
                branch.exact_carrier_free_vars,
                branch.exact_carrier_free_blocks
            ),
            (2, 1, 1, 2)
        );
        let exact = old_discount(&branch, &runtime) + 6 * carriers();
        for mode in [5, 6] {
            crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(exact));
            assert_eq!(fixed_hir_discount(mode, &branch, &runtime).unwrap(), exact);
            crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(exact - 1));
            assert!(fixed_hir_discount(mode, &branch, &runtime)
                .unwrap_err()
                .iter()
                .all(|error| error.code == "SPX-G171"));
        }
        crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(expected));
        // Modes 0-4 and 7+ never consult either census.
        for mode in [0, 4, 7, 8] {
            assert!(fixed_hir_discount(mode, &raw, &runtime).is_ok());
        }
    }
