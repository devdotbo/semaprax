//! Independent full-state storage, publication and allocator owners.
use super::*;
use crate::test_allocation_observer::measure;
fn rich(count: usize)->BTreeMap<ValueId,ValidationBinding> {
    (0..count).map(|index| {
        let path=vec![PlaceProjection::Field(DeclarationId::new("scope.field"));3];
        let mut moved_places=BTreeMap::new();moved_places.insert(path.clone(),Availability::MaybeMoved);
        let mut definitely_partial=BTreeSet::new();definitely_partial.insert(path);
        let mut active_loans=BTreeSet::new();active_loans.insert(LoanId(index as u16));
        (ValueId::new(format!("scope.binding.{index:04}")),ValidationBinding {
            contract_local:index%2==0,mutable:index%3==0,
            ty:ResolvedType::Function{parameters:vec![ResolvedType::Nominal{
                declaration:DeclarationId::new("scope.nominal"),arguments:vec![ResolvedType::I64,ResolvedType::Bool],
            };2],result:Box::new(ResolvedType::I64)},
            ownership:OwnershipMode::Own,availability:Availability::Available,
            moved_places,definitely_partial,active_loans,
        })
    }).collect()
}
#[test]
fn untouched_checkpoints_remove_real_full_scope_clone_requests() {
    for count in [0,1,2,17,128] {
        let input=rich(count);
        let ((),one)=measure(||drop(input.clone()));
        let ((),old)=measure(|| {for _ in 0..17 {drop(input.clone());}});
        Scope::prepare(count).unwrap();
        let checkpoint=Scope::from_prepaid(input);
        let ((),new)=measure(|| {for _ in 0..17 {drop(checkpoint.clone());drop(checkpoint.ids());}});
        assert!(!old.invalid && !new.invalid);
        assert_eq!(old.live,0);assert_eq!(new.live,0);
        // Literal constructor count: Rc root/key snapshots have no new heap
        // backing; every old request is removed, not replaced by a forecast.
        assert_eq!(new.allocations,0);assert_eq!(new.allocated,0);assert_eq!(new.highwater,0);
        assert_eq!(old.allocated,one.allocated*17);
        assert_eq!(old.allocations,one.allocations*17);
        assert_eq!(old.allocated,new.allocated+one.allocated*17);
        if count>0 {assert!(old.allocated>0 && old.allocations>0);}
    }
}
fn receiver(input: BTreeMap<ValueId,ValidationBinding>,fail: bool)->Result<(),Diagnostic> {
    Scope::prepare(input.len())?;
    let mut target=BTreeMap::new();
    {
        let mut publication=Publication::new(&mut target,Scope::from_prepaid(input));
        let current=publication.published.clone();
        // All children are declared after the guard and release their handles
        // first on ordinary completion and on an injected late semantic error.
        let mut children=Storage::new();
        for _ in 0..17 {children.push(current.clone())?;}
        let mut changed=current.clone();
        let id=changed.keys().next().unwrap().clone();
        let value=changed.get_mut(&id)?.unwrap();
        value.availability=Availability::Moved;
        value.contract_local=!value.contract_local;
        value.mutable=!value.mutable;
        value.ownership=OwnershipMode::Borrow;
        value.ty=ResolvedType::Bool;
        value.moved_places.clear();value.definitely_partial.clear();value.active_loans.clear();
        publication.publish(&changed)?;
        if fail {return Err(hir_error("injected late checkpoint error"));}
    }
    assert_eq!(target.len(),17);
    let value=target.values().next().unwrap();
    assert_eq!(value.availability,Availability::Moved);
    assert_eq!(value.ty,ResolvedType::Bool);
    assert_eq!(value.ownership,OwnershipMode::Borrow);
    assert!(!value.contract_local && !value.mutable);
    assert!(value.moved_places.is_empty() && value.definitely_partial.is_empty() && value.active_loans.is_empty());
    drop(target);
    Ok(())
}
#[test]
fn actual_checkpoint_storage_and_publication_are_paid_through_drop_and_exact_short() {
    let input=rich(17);
    let ((wide,metrics),refused,used)=crate::bounded_output::with_limit_usage(64*1024*1024,|| {
        measure(||receiver(input,false))
    });
    wide.unwrap();
    assert!(!refused && !metrics.invalid);
    assert!(metrics.allocated>0 && used>0);
    assert!(metrics.allocated<=used && metrics.highwater<=used);
    assert_eq!(metrics.live,0);
    let input=rich(17);
    let ((exact,metrics),refused,actual)=crate::bounded_output::with_limit_usage(used,|| {
        measure(||receiver(input,false))
    });
    exact.unwrap();assert!(!refused && !metrics.invalid);assert_eq!(actual,used);
    assert!(metrics.allocated<=used && metrics.highwater<=used);assert_eq!(metrics.live,0);
    let input=rich(17);
    let ((short,metrics),refused,_)=crate::bounded_output::with_limit_usage(used-1,|| {
        measure(|| {
            let result=receiver(input,false);
            assert!(result.is_err());
            assert!(!crate::bounded_output::reserve_active_required(0));
            drop(result);
        })
    });
    let ()=short;
    assert!(refused && !metrics.invalid);assert_eq!(metrics.live,0);
    // Report backing on a semantic error is observed separately, with no
    // assertion that the storage success allowance also funds that report.
    let input=rich(17);
    let ((error,metrics),_,_)=crate::bounded_output::with_limit_usage(64*1024*1024,|| {
        measure(|| {let result=receiver(input,true);assert!(result.is_err());drop(result);})
    });
    let ()=error;assert!(!metrics.invalid);assert_eq!(metrics.live,0);
}
#[test]
fn changed_paths_and_full_field_joins_match_the_unchanged_owning_helpers() {
    let input=rich(17);let ids=input.keys().cloned().collect::<Vec<_>>();
    let mut old=input.clone();let mut source=input.clone();
    let value=source.get_mut(&ids[0]).unwrap();
    value.availability=Availability::Moved;value.moved_places.clear();
    value.definitely_partial.clear();value.active_loans.insert(LoanId(999));
    HirValidator::join_conditional(&mut old,&source,&ids);
    Scope::prepare(input.len()).unwrap();let mut current=Scope::from_prepaid(input);
    Scope::prepare(source.len()).unwrap();let source=Scope::from_prepaid(source);
    let keys=current.ids();merge::conditional(&mut current,&source,&keys).unwrap();
    assert!(current.iter().eq(old.iter()));
    // AVL insertion and path copies preserve exact sorted keys and all fields.
    for index in (17..128).rev() {
        let (id,value)=rich(index+1).pop_last().unwrap();
        old.insert(id.clone(),value.clone());current.insert(id,value).unwrap();
    }
    assert!(current.iter().eq(old.iter()));
    assert_eq!(current.len(),old.len());
}
#[test]
fn suppression_and_late_error_restore_the_last_complete_publication() {
    let input=rich(17);let expected=input.clone();
    Scope::prepare(input.len()).unwrap();let mut target=BTreeMap::new();
    {
        let mut publication=Publication::new(&mut target,Scope::from_prepaid(input));
        let mut child=publication.published.clone();
        publication.enabled=false;
        let id=child.keys().next().unwrap().clone();
        child.get_mut(&id).unwrap().unwrap().availability=Availability::Moved;
        publication.publish(&child).unwrap();
        drop(child);
    }
    assert_eq!(target,expected);
}


#[test]
fn equal_nonempty_projection_states_still_run_the_full_prefix_join() {
    let input=rich(2);
    let ids=input.keys().cloned().collect::<Vec<_>>();
    let mut old=input.clone();
    HirValidator::join_conditional(&mut old,&input,&ids);
    Scope::prepare(input.len()).unwrap();
    let mut current=Scope::from_prepaid(input);
    let source=current.clone();let keys=current.ids();
    merge::conditional(&mut current,&source,&keys).unwrap();
    assert!(current.iter().eq(old.iter()));
    assert!(old.values().all(|value|value.definitely_partial.contains(&Vec::new())));
    let mut branches=old.clone();
    HirValidator::join_branches(&mut branches,&old,&old,&ids);
    let left=current.clone();let right=current.clone();
    merge::branches(&mut current,&left,&right,&keys).unwrap();
    assert!(current.iter().eq(branches.iter()));
}

fn late_publication(input: BTreeMap<ValueId,ValidationBinding>,
    target: &mut BTreeMap<ValueId,ValidationBinding>) -> Result<(),Diagnostic> {
    Scope::prepare(input.len())?;
    let mut publication=Publication::new(target,Scope::from_prepaid(input));
    let mut current=publication.published.clone();
    let id=current.keys().next().unwrap().clone();
    current.get_mut(&id)?.unwrap().availability=Availability::Moved;
    publication.publish(&current)?;
    let mut failed_child=current.clone();
    failed_child.get_mut(&id)?.unwrap().availability=Availability::MaybeMoved;
    Err(hir_error("late child preserves the last complete publication"))
}

#[test]
fn real_late_error_moves_the_last_published_full_binding_on_unwind() {
    let input=rich(17);let mut expected=input.clone();
    expected.values_mut().next().unwrap().availability=Availability::Moved;
    let mut target=BTreeMap::new();
    let result=late_publication(input,&mut target);
    assert_eq!(result.unwrap_err().message,"late child preserves the last complete publication");
    assert_eq!(target,expected);
}


#[test]
fn nested_guard_publication_shares_untouched_backing_and_restores_full_changed_state() {
    let input=rich(17);let expected=input.clone();
    Scope::prepare(input.len()).unwrap();let mut scope=Scope::from_prepaid(input);
    let baseline=scope.clone();
    let (((),metrics),refused,used)=crate::bounded_output::with_limit_usage(0,||measure(|| {
        for _ in 0..17 {
            let mut publication=Publication::take(&mut scope).unwrap();
            let current=publication.published.clone();
            publication.publish(&current).unwrap();
        }
    }));
    assert!(!refused && !metrics.invalid);assert_eq!(used,0);
    assert_eq!(metrics.allocations,0);assert_eq!(metrics.allocated,0);
    assert_eq!(metrics.highwater,0);assert_eq!(metrics.live,0);
    assert!(scope.iter().eq(expected.iter()));
    {
        let mut publication=Publication::take(&mut scope).unwrap();
        let mut current=publication.published.clone();
        let id=current.keys().next().unwrap().clone();
        current.get_mut(&id).unwrap().unwrap().availability=Availability::Moved;
        publication.publish(&current).unwrap();
        let mut failed_child=current.clone();
        failed_child.get_mut(&id).unwrap().unwrap().availability=Availability::MaybeMoved;
    }
    let mut changed=expected.clone();changed.values_mut().next().unwrap().availability=Availability::Moved;
    assert!(scope.iter().eq(changed.iter()));
    assert!(baseline.iter().eq(expected.iter()));
}


#[test]
fn actual_continuation_allocator_and_excess_capacity_failures_keep_first_receipts() {
    for (request,bytes,message) in [
        (storage::Request::AllocationFailure,8,"scope continuation directory allocation failed"),
        (storage::Request::ExcessCapacity,16,"scope continuation directory capacity changed"),
    ] {
        let (((),metrics),refused,used,receipt)=crate::bounded_output::with_limit_usage_refusal(usize::MAX,||measure(|| {
            let Err(error)=Storage::<u64>::with_frontier(1,request) else {panic!("injected frontier must fail");};
            assert_eq!(error.message,message);
            assert!(!crate::bounded_output::reserve_active_required(0));
            drop(error);
        }));
        assert!(refused && !metrics.invalid);assert_eq!(metrics.live,0);
        assert_eq!(used,bytes);
        let receipt=receipt.expect("actual active payer first refusal");
        assert_eq!(receipt.requested,bytes);assert_eq!(receipt.remaining,usize::MAX-bytes);
        // This owner observes reports through Drop without claiming that the
        // continuation allowance pays the separate failure-report carrier.
    }
    let (((),metrics),refused,used,receipt)=crate::bounded_output::with_limit_usage_refusal(usize::MAX,||measure(|| {
        let error=charge::reserve(None).unwrap_err();
        assert!(!crate::bounded_output::reserve_active_required(0));drop(error);
    }));
    assert!(refused && !metrics.invalid);assert_eq!(metrics.live,0);assert_eq!(used,0);
    assert_eq!(receipt.unwrap().requested,usize::MAX);
}

#[test]
fn contract_baseline_copies_all_fields_once_and_pays_real_backing_through_drop() {
    let input=rich(17);
    let mut expected=input.clone();for value in expected.values_mut() {value.contract_local=false;}
    let ((),one)=measure(|| {let mut old=input.clone();for value in old.values_mut(){value.contract_local=false;}drop(old);});
    let ((),old)=measure(|| {for _ in 0..17 {let mut clause=input.clone();for value in clause.values_mut(){value.contract_local=false;}drop(clause);}});
    let (((),actual),refused,used)=crate::bounded_output::with_limit_usage(64*1024*1024,||measure(|| {
        let baseline=Scope::fresh_baseline(&input).unwrap();
        assert!(baseline.iter().eq(expected.iter()));drop(baseline);
    }));
    assert!(!refused && !actual.invalid);assert_eq!(actual.live,0);
    assert!(actual.allocated<=used && actual.highwater<=used);
    let baseline=Scope::fresh_baseline(&input).unwrap();
    let ((),checkpoints)=measure(|| {for _ in 0..17 {drop(baseline.clone());}});
    assert_eq!(checkpoints.allocated,0);assert_eq!(checkpoints.allocations,0);
    assert_eq!(old.allocated,checkpoints.allocated+17*one.allocated);
    assert_eq!(old.allocations,checkpoints.allocations+17*one.allocations);
    // The retained baseline has its own observed backing and payment. Its
    // allocation/highwater is reported separately from absent clause copies.
    let (((),exact),refused,paid)=crate::bounded_output::with_limit_usage(used,||measure(|| {
        drop(Scope::fresh_baseline(&input).unwrap());
    }));
    assert!(!refused && !exact.invalid);assert_eq!(exact.live,0);assert_eq!(paid,used);
    let (((),short),refused,_,receipt)=crate::bounded_output::with_limit_usage_refusal(used-1,||measure(|| {
        let Err(error)=Scope::fresh_baseline(&input) else {panic!("one short baseline must fail");};
        assert!(!crate::bounded_output::reserve_active_required(0));drop(error);
    }));
    assert!(refused && !short.invalid);assert_eq!(short.live,0);assert!(receipt.is_some());
    assert_eq!(input,rich(17));
}
