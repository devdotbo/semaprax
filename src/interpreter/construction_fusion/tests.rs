//! Private registry protocol controls; these do not install source authority.
use super::{Allocator, Backing, Plan, Registry, State};
use crate::construction_fusion::domain::owner_core::Core;
use crate::construction_fusion::domain::{Issuer, ModuleIdentity};
use crate::hir::{DeclarationId, ResolvedExpr, ResolvedType};
use crate::interpreter::{OwnedRecordValue, Value};
use std::{alloc::Layout, collections::BTreeMap, sync::Arc};

fn sites() -> crate::hir::ResolvedProgram {
    let source = crate::check(
        "module fused.registry; @id(\"entry\") fn main()->i64 {0}",
        "registry.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}
fn plan(site: &ResolvedExpr, rows: usize) -> Registry<'_> {
    Registry {
        plan: Some(Plan {
            producer: site,
            push: site,
            rows,
            width: 3,
        }),
        state: None,
        failed: false,
    }
}
fn row(bytes: super::OwnedBytesValue) -> Value {
    Value::Record(Arc::new(OwnedRecordValue {
        record: DeclarationId::new("row"),
        fields: BTreeMap::from([(DeclarationId::new("row.bytes"), Value::Bytes(bytes))]),
    }))
}
fn required(rows: usize) -> usize {
    Layout::new::<State>().size()
        + rows * 3
        + Core::<Allocator>::metadata_layouts(rows)
            .unwrap()
            .iter()
            .map(Layout::size)
            .sum::<usize>()
        - Layout::new::<Core<'static, Allocator>>().size()
        - Layout::new::<Vec<crate::construction_fusion::domain::owner_core::Child<'static>>>()
            .size()
        + Layout::new::<Issuer>().size()
        + Layout::new::<ModuleIdentity>().size()
}
#[test]
fn held_registry_publishes_once_and_scopes_reads_stores_and_parent_cleanup() {
    let program = sites();
    let site = &program.functions[0].body;
    let ((events, payload), overflow, used) =
        crate::bounded_output::with_limit_usage(required(2), || {
            let mut registry = plan(site, 2);
            let (mut events, mut payload) = (0, 0);
            let _first = registry
                .produce(site, 3, &mut events, &mut payload)
                .unwrap()
                .unwrap();
            assert!(registry
                .produce(site, 3, &mut events, &mut payload)
                .is_err());
            // A refusal is sticky; use a separate invocation for the normal path.
            assert!(registry.failed);
            (events, payload)
        });
    assert_eq!((events, payload), (1, 6));
    assert!(!overflow);
    assert_eq!(used, required(2));
    let (result, overflow, used) = crate::bounded_output::with_limit_usage(required(2), || {
        let mut registry = plan(site, 2);
        let (mut events, mut payload) = (0, 0);
        let first = registry
            .produce(site, 3, &mut events, &mut payload)
            .unwrap()
            .unwrap();
        let mut first = row(first);
        registry.publish(site, &first).unwrap();
        let second = registry
            .produce(site, 3, &mut events, &mut payload)
            .unwrap()
            .unwrap();
        let mut second = row(second);
        registry.publish(site, &second).unwrap();
        let scope = registry.enter().unwrap();
        let Value::Record(record) = &first else {
            panic!()
        };
        let Value::Bytes(bytes) = &record.fields[&DeclarationId::new("row.bytes")] else {
            panic!()
        };
        let Backing::Leased(lease) = &bytes.bytes else {
            panic!()
        };
        let view = registry.borrow(lease).unwrap();
        assert_eq!(
            registry.read(view, 0, 3, |bytes| bytes.to_vec()).unwrap(),
            [0, 0, 0]
        );
        assert!(registry.preflight_value(&first).is_err());
        registry.leave(scope).unwrap();
        assert!(registry.read(view, 0, 3, <[u8]>::len).is_err());
        registry.preflight_value(&first).unwrap();
        let Value::Record(record) = &mut first else {
            panic!()
        };
        let Value::Bytes(bytes) = Arc::get_mut(record)
            .unwrap()
            .fields
            .get_mut(&DeclarationId::new("row.bytes"))
            .unwrap()
        else {
            panic!()
        };
        let Backing::Leased(lease) = &mut bytes.bytes else {
            panic!()
        };
        let stale = lease.clone();
        registry.store(lease, 0, &[4, 5, 6]).unwrap();
        assert!(registry.len(&stale).is_err());
        assert_eq!(
            bytes.read(&mut registry, |bytes| bytes.to_vec()).unwrap(),
            [4, 5, 6]
        );
        assert!(registry.settle_value(&mut second).is_err());
        let Value::Record(second) = second else {
            panic!()
        };
        let mut second = Arc::try_unwrap(second)
            .unwrap()
            .fields
            .into_values()
            .next()
            .unwrap();
        registry.settle_value(&mut second).unwrap();
        assert!(registry.state.as_ref().unwrap().core.failure().is_none());
        let Value::Record(first) = first else {
            panic!()
        };
        let mut first = Arc::try_unwrap(first)
            .unwrap()
            .fields
            .into_values()
            .next()
            .unwrap();
        registry.settle_value(&mut first).unwrap();
        assert_eq!((events, payload), (1, 6));
        42
    });
    assert_eq!(result, 42);
    assert!(!overflow);
    assert_eq!(used, required(2));
}
#[test]
fn registry_exact_metadata_one_short_and_default_route_have_no_free_credit() {
    let program = sites();
    let site = &program.functions[0].body;
    let ((refused, state, sticky), overflow, used) =
        crate::bounded_output::with_limit_usage(required(1) - 1, || {
            let mut registry = plan(site, 1);
            let (mut events, mut payload) = (0, 0);
            let refused = registry
                .produce(site, 3, &mut events, &mut payload)
                .is_err();
            let sticky = registry
                .produce(site, 3, &mut events, &mut payload)
                .is_err();
            assert_eq!((events, payload), (0, 0));
            (refused, registry.state.is_some(), sticky)
        });
    assert!(refused && sticky && !state);
    assert!(overflow);
    assert!(used < required(1));
    let ((selected, events, payload), overflow, used) =
        crate::bounded_output::with_limit_usage(0, || {
            let mut registry = Registry::default();
            let (mut events, mut payload) = (32, 2_097_152);
            let selected = registry
                .produce(site, 3, &mut events, &mut payload)
                .unwrap();
            (selected, events, payload)
        });
    assert!(selected.is_none());
    assert_eq!((events, payload), (32, 2_097_152));
    assert!(!overflow);
    assert_eq!(used, 0);
    assert_eq!(site.ty, ResolvedType::I64);
}
