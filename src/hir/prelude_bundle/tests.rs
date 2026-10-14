use super::{BasePreludeBundle, Failure, Payer};
use crate::hir::{DeclarationId, DeclarationIndex, ResolvedTypeDeclarationKind};
use crate::test_allocation_observer::measure;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Denied {
    Bytes,
    Work,
    Site,
}
struct Account {
    bytes: usize,
    work: usize,
    byte_limit: usize,
    work_limit: usize,
    sites: usize,
    fail_site: Option<usize>,
    first: Option<Denied>,
}
impl Account {
    fn unlimited() -> Self {
        Self {
            bytes: 0,
            work: 0,
            byte_limit: usize::MAX,
            work_limit: usize::MAX,
            sites: 0,
            fail_site: None,
            first: None,
        }
    }
    fn deny(&mut self, error: Denied) -> Result<(), Denied> {
        Err(*self.first.get_or_insert(error))
    }
}
impl Payer for Account {
    type Error = Denied;
    fn bytes(&mut self, requested: usize) -> Result<(), Denied> {
        if let Some(first) = self.first {
            return Err(first);
        }
        if requested > 0 {
            self.sites += 1;
            if self.fail_site == Some(self.sites) {
                return self.deny(Denied::Site);
            }
        }
        if requested > self.byte_limit - self.bytes {
            return self.deny(Denied::Bytes);
        }
        self.bytes += requested;
        Ok(())
    }
    fn work(&mut self, requested: usize) -> Result<(), Denied> {
        if let Some(first) = self.first {
            return Err(first);
        }
        if requested > self.work_limit - self.work {
            return self.deny(Denied::Work);
        }
        self.work += requested;
        Ok(())
    }
}
fn program() -> crate::ast::Program {
    crate::parse(
        "module prelude.bundle; @id(\"bundle.main\") fn main()->i64 {0}",
        "bundle.spx",
    )
    .unwrap()
}

#[test]
fn complete_original_base_types_and_every_lookup_keep_exact_order_ids_and_payloads() {
    let program = program();
    let original = [super::original::option(), super::original::result()];
    let original_index =
        DeclarationIndex::nominal_original_with_prelude(&program, &original).unwrap();
    let resolved = crate::hir::resolve(&program).unwrap();
    let local = DeclarationIndex::from_verified_with_prelude(&program, &[]).unwrap();
    let mut payer = Account::unlimited();
    let bundle = BasePreludeBundle::acquire(&mut payer).unwrap();
    assert_eq!(bundle.types(), resolved.types.as_slice());
    assert_eq!(bundle.types()[0].id.as_str(), crate::prelude::OPTION_ID);
    assert_eq!(bundle.types()[1].id.as_str(), crate::prelude::RESULT_ID);
    let overlay = bundle.overlay(&local);
    let all = overlay
        .declarations(&mut payer)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(all, original_index.declarations().collect::<Vec<_>>());
    for declaration in original_index.declarations() {
        assert_eq!(
            overlay.declaration(&declaration.id, &mut payer).unwrap(),
            Some(declaration)
        );
        assert_eq!(
            overlay
                .type_parameters(&declaration.id, &mut payer)
                .unwrap(),
            original_index.type_parameters(&declaration.id)
        );
    }
    for ty in bundle.types() {
        assert_eq!(
            overlay.type_id(&ty.name, &mut payer).unwrap(),
            original_index.type_id(&ty.name)
        );
        assert_eq!(
            overlay.variant_cases(&ty.id, &mut payer).unwrap(),
            original_index.variant_cases(&ty.id)
        );
        let ResolvedTypeDeclarationKind::Variant { cases } = &ty.kind else {
            panic!("base variant");
        };
        for case in cases.iter() {
            assert_eq!(
                overlay.case_id(&ty.id, &case.name, &mut payer).unwrap(),
                original_index.case_id(&ty.id, &case.name)
            );
            assert_eq!(
                overlay.case_fields(&case.id, &mut payer).unwrap(),
                original_index.case_fields(&case.id)
            );
            for field in &case.fields {
                assert_eq!(
                    overlay.field_id(&case.id, &field.name, &mut payer).unwrap(),
                    original_index.field_id(&case.id, &field.name)
                );
            }
        }
    }
    let foreign = DeclarationId::new("foreign.bundle");
    assert!(overlay.declaration(&foreign, &mut payer).unwrap().is_none());
    assert!(overlay.case_fields(&foreign, &mut payer).unwrap().is_none());
    assert!(overlay
        .variant_cases(&foreign, &mut payer)
        .unwrap()
        .is_none());
    assert!(overlay.type_id("Foreign", &mut payer).unwrap().is_none());
}

#[test]
fn two_borrowed_module_overlays_and_explicit_lease_share_actual_payload_addresses() {
    let local_a = DeclarationIndex::from_verified_with_prelude(&program(), &[]).unwrap();
    let local_b = DeclarationIndex::from_verified_with_prelude(&program(), &[]).unwrap();
    let id = DeclarationId::new(crate::prelude::OPTION_SOME_ID);
    let mut payer = Account::unlimited();
    let bundle = BasePreludeBundle::acquire(&mut payer).unwrap();
    let before = payer.bytes;
    let (lease, allocation) = measure(|| {
        let lease = bundle.retain(&mut payer).unwrap();
        let a = bundle.overlay(&local_a);
        let b = lease.overlay(&local_b);
        assert!(std::ptr::eq(
            a.declaration(&id, &mut payer).unwrap().unwrap(),
            b.declaration(&id, &mut payer).unwrap().unwrap()
        ));
        assert_eq!(
            a.case_fields(&id, &mut payer).unwrap().unwrap().as_ptr(),
            b.case_fields(&id, &mut payer).unwrap().unwrap().as_ptr()
        );
        lease
    });
    assert_eq!(payer.bytes, before);
    assert!(!allocation.invalid);
    assert_eq!(allocation.allocated, 0);
    assert_eq!(bundle.types().as_ptr(), lease.types().as_ptr());
    drop(bundle);
    assert_eq!(lease.types().len(), 2);
    drop(lease);
    // Observe the last retained owner, not only the original bundle's Drop.
    let mut retained_account = Account::unlimited();
    let (_, retained_drop) = measure(|| {
        let original = BasePreludeBundle::acquire(&mut retained_account).unwrap();
        let retained = original.retain(&mut retained_account).unwrap();
        let compatibility = retained.clone();
        assert_eq!(
            compatibility.types_slice().as_ptr(),
            retained.types_slice().as_ptr()
        );
        drop(original);
        drop(retained);
        assert_eq!(compatibility.types_slice().len(), 2);
        drop(compatibility);
    });
    assert!(!retained_drop.invalid);
    assert_eq!(retained_drop.live, 0);
    assert_eq!(retained_drop.allocated, retained_drop.freed);
    assert!(retained_drop.highwater <= retained_account.bytes);
}

#[test]
fn whole_owner_exact_one_short_zero_work_and_each_acquisition_refusal_drop_every_owner() {
    let mut baseline = Account::unlimited();
    let (_, observed) = measure(|| drop(BasePreludeBundle::acquire(&mut baseline).unwrap()));
    assert!(!observed.invalid);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    assert!(observed.allocated <= baseline.bytes);
    assert!(observed.highwater <= baseline.bytes);
    for (bytes, work, succeeds) in [
        (baseline.bytes, baseline.work, true),
        (baseline.bytes - 1, baseline.work, false),
        (0, baseline.work, false),
        (baseline.bytes, baseline.work - 1, false),
        (baseline.bytes, 0, false),
    ] {
        let mut account = Account::unlimited();
        account.byte_limit = bytes;
        account.work_limit = work;
        let (_, allocation) = measure(|| {
            let result = BasePreludeBundle::acquire(&mut account);
            assert_eq!(result.is_ok(), succeeds);
            drop(result);
        });
        assert!(!allocation.invalid);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= account.bytes);
        if !succeeds {
            let first = account.first.unwrap();
            let (_, after) = measure(|| {
                assert_eq!(
                    BasePreludeBundle::acquire(&mut account).err().unwrap(),
                    Failure::Payer(first)
                );
            });
            assert_eq!(after.allocated, 0);
            assert_eq!(account.first, Some(first));
        }
    }
    for site in 1..=baseline.sites {
        let mut account = Account::unlimited();
        account.fail_site = Some(site);
        let (_, allocation) = measure(|| {
            assert!(BasePreludeBundle::acquire(&mut account).is_err());
        });
        assert!(!allocation.invalid);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert_eq!(account.first, Some(Denied::Site));
    }
}

#[test]
fn lookup_iteration_and_lease_work_keep_the_same_sticky_invocation_account() {
    let local = DeclarationIndex::from_verified_with_prelude(&program(), &[]).unwrap();
    let mut payer = Account::unlimited();
    let bundle = BasePreludeBundle::acquire(&mut payer).unwrap();
    let id = DeclarationId::new(crate::prelude::RESULT_ID);
    let start = payer.work;
    let result = bundle.overlay(&local).declaration(&id, &mut payer).unwrap();
    assert!(result.is_some());
    let cost = payer.work - start;
    payer.work_limit = payer.work + cost;
    assert!(bundle
        .overlay(&local)
        .declaration(&id, &mut payer)
        .unwrap()
        .is_some());
    let (_, allocation) = measure(|| {
        assert_eq!(
            bundle
                .overlay(&local)
                .declaration(&id, &mut payer)
                .err()
                .unwrap(),
            Failure::Payer(Denied::Work)
        );
        assert_eq!(
            bundle.retain(&mut payer).err().unwrap(),
            Failure::Payer(Denied::Work)
        );
        let overlay = bundle.overlay(&local);
        let mut values = overlay.declarations(&mut payer);
        assert_eq!(
            values.next().unwrap().err().unwrap(),
            Failure::Payer(Denied::Work)
        );
        assert!(values.next().is_none());
    });
    assert!(!allocation.invalid);
    assert_eq!(allocation.allocated, 0);
}

#[test]
fn unused_and_parser_only_overlapping_names_keep_unconditional_base_selection() {
    // These overlap inputs are parser/index controls only. They grant no
    // resolution admission to source declarations shadowing compiler names.
    for declaration in [
        "",
        "@id(\"authored.option\") record Option {}",
        "@id(\"authored.result\") record Result {}",
        "@id(\"authored.option\") record Option {} @id(\"authored.result\") record Result {}",
    ] {
        let source =
            format!("module bundle.overlap; {declaration} @id(\"entry\") fn main()->i64 {{0}}");
        let program = crate::parse(&source, "overlap.spx").unwrap();
        let original = [super::original::option(), super::original::result()];
        let old = DeclarationIndex::nominal_original_with_prelude(&program, &original).unwrap();
        let local = DeclarationIndex::from_verified_with_prelude(&program, &[]).unwrap();
        let mut payer = Account::unlimited();
        let bundle = BasePreludeBundle::acquire(&mut payer).unwrap();
        assert_eq!(bundle.types().len(), 2);
        assert_eq!(bundle.declarations().len(), 9);
        let overlay = bundle.overlay(&local);
        assert_eq!(
            overlay.type_id("Option", &mut payer).unwrap(),
            old.type_id("Option")
        );
        assert_eq!(
            overlay.type_id("Result", &mut payer).unwrap(),
            old.type_id("Result")
        );
        assert_eq!(
            overlay
                .declarations(&mut payer)
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            old.declarations().collect::<Vec<_>>()
        );
    }
}

#[test]
fn initializer_and_receiving_carriers_are_paid_before_any_producer_work() {
    // These are separately live values, in addition to the final Arc backing.
    let required = std::mem::size_of::<super::Contents>()
        + std::mem::size_of::<BasePreludeBundle>()
        + std::mem::size_of::<Result<BasePreludeBundle, Failure<Denied>>>();
    for limit in [0, required - 1] {
        let mut account = Account::unlimited();
        account.byte_limit = limit;
        let (result, physical) = measure(|| BasePreludeBundle::acquire(&mut account));
        assert!(matches!(result, Err(Failure::Payer(Denied::Bytes))));
        assert_eq!(account.bytes, 0);
        assert_eq!(account.work, 0);
        assert_eq!(account.sites, 1);
        assert_eq!(physical.allocated, 0);
        assert_eq!(physical.live, 0);
        assert!(!physical.invalid);
    }
    let mut account = Account::unlimited();
    account.byte_limit = required;
    let (result, physical) = measure(|| BasePreludeBundle::acquire(&mut account));
    // The exact carrier request succeeds; the following vector request refuses.
    assert!(matches!(result, Err(Failure::Payer(Denied::Bytes))));
    assert_eq!(account.bytes, required);
    assert_eq!(account.work, 1);
    assert_eq!(account.sites, 2);
    assert_eq!(physical.allocated, 0);
    assert_eq!(physical.live, 0);
    assert!(!physical.invalid);
}

#[test]
fn fixed_base_unmetered_index_views_borrow_complete_old_facts_without_allocation() {
    let program = program();
    let original = [super::original::option(), super::original::result()];
    let old = DeclarationIndex::nominal_original_with_prelude(&program, &original).unwrap();
    let mut account = Account::unlimited();
    let bundle = BasePreludeBundle::acquire(&mut account).unwrap();
    let paid_before = (account.bytes, account.work);
    account.first = Some(Denied::Work);
    let (_, physical) = measure(|| {
        let compatibility = bundle.clone();
        assert_eq!(
            bundle.types_slice().as_ptr(),
            compatibility.types_slice().as_ptr()
        );
        drop(compatibility);
        assert_eq!(bundle.declarations_slice().len(), 9);
        assert_eq!(bundle.types_slice().len(), 2);
        for entry in bundle.declarations_slice() {
            assert_eq!(
                bundle.declaration_by_text(entry.id.as_str()),
                old.declaration(&entry.id)
            );
            assert_eq!(
                bundle.type_parameters(&entry.id),
                old.type_parameters(&entry.id)
            );
        }
        for ty in bundle.types_slice() {
            assert_eq!(bundle.type_id(&ty.name), old.type_id(&ty.name));
            assert_eq!(bundle.variant_cases(&ty.id), old.variant_cases(&ty.id));
            for case in bundle.variant_cases(&ty.id).unwrap() {
                assert_eq!(
                    bundle.case_id(&ty.id, &case.name),
                    old.case_id(&ty.id, &case.name)
                );
                assert_eq!(bundle.case_fields(&case.id), old.case_fields(&case.id));
                for field in &case.fields {
                    assert_eq!(
                        bundle.field_id(&case.id, &field.name),
                        old.field_id(&case.id, &field.name)
                    );
                }
            }
        }
        assert!(bundle.declaration_by_text("foreign.base").is_none());
        assert!(bundle.type_id("Foreign").is_none());
    });
    assert_eq!((account.bytes, account.work), paid_before);
    assert_eq!(account.first, Some(Denied::Work));
    assert_eq!(physical.allocated, 0);
    assert_eq!(physical.live, 0);
    assert!(!physical.invalid);
}
