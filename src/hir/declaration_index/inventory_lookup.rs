//! Allocation-free, read-only current metadata probes. Ordinary get and
//! invariant lookup stay unchanged. Refused comparison results never escape.
use super::*;
use crate::hir::renewal_probes::{units, Index, Work};
use std::borrow::Borrow;
use std::cell::{Cell, RefCell};
use std::cmp::Ordering;

mod compound;

trait Key {
    fn parts(&self) -> (&[u8], &[u8]);
    fn segments(&self) -> [&[u8]; 5] {
        let (head, tail) = self.parts();
        [head, tail, &[], &[], &[]]
    }
    fn plan(&self) -> Option<&compound::Plan<'_>> {
        None
    }
    fn metered(&self) -> bool {
        false
    }
    fn visit(&self, _units: usize) -> bool {
        true
    }
}
impl Key for DeclarationId {
    fn parts(&self) -> (&[u8], &[u8]) {
        (self.as_str().as_bytes(), &[])
    }
}
impl Key for String {
    fn parts(&self) -> (&[u8], &[u8]) {
        (self.as_bytes(), &[])
    }
}
impl<'a> Borrow<dyn Key + 'a> for String {
    fn borrow(&self) -> &(dyn Key + 'a) {
        self
    }
}
impl<'a> Borrow<dyn Key + 'a> for DeclarationId {
    fn borrow(&self) -> &(dyn Key + 'a) {
        self
    }
}
impl PartialEq for dyn Key + '_ {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for dyn Key + '_ {}
impl PartialOrd for dyn Key + '_ {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for dyn Key + '_ {
    fn cmp(&self, other: &Self) -> Ordering {
        let work = if self.metered() { self } else { other };
        if !work.visit(1) {
            return Ordering::Equal;
        }
        let mut a = self.segments().into_iter().flatten().copied();
        let mut b = other.segments().into_iter().flatten().copied();
        match (self.plan(), other.plan()) {
            (None, None) => compare_streams(|_| a.next(), |_| b.next(), work),
            (Some(plan), None) => {
                if !work.visit(compound::CURSOR_INITIALIZATION_WORK) {
                    return Ordering::Equal;
                }
                let mut cursor = plan.bytes();
                compare_streams(|visit| cursor.next(visit), |_| b.next(), work)
            }
            (None, Some(plan)) => {
                if !work.visit(compound::CURSOR_INITIALIZATION_WORK) {
                    return Ordering::Equal;
                }
                let mut cursor = plan.bytes();
                compare_streams(|_| a.next(), |visit| cursor.next(visit), work)
            }
            (Some(left), Some(right)) => {
                if !work.visit(2 * compound::CURSOR_INITIALIZATION_WORK) {
                    return Ordering::Equal;
                }
                let mut a = left.bytes();
                let mut b = right.bytes();
                compare_streams(|visit| a.next(visit), |visit| b.next(visit), work)
            }
        }
    }
}
// Only reads use this comparator. After first refusal no further callback is
// run and its stopping equality has no authority; every provisional map value
// is withheld by the receiving probe before use.
fn compare_streams(
    mut a: impl FnMut(&dyn Fn(usize) -> bool) -> Option<u8>,
    mut b: impl FnMut(&dyn Fn(usize) -> bool) -> Option<u8>,
    work: &dyn Key,
) -> Ordering {
    let visit = |n| work.visit(n);
    loop {
        if !work.visit(1) {
            return Ordering::Equal;
        }
        let left = a(&visit);
        if !work.visit(0) {
            return Ordering::Equal;
        }
        let right = b(&visit);
        if !work.visit(0) {
            return Ordering::Equal;
        }
        match (left, right) {
            (Some(a), Some(b)) => match a.cmp(&b) {
                Ordering::Equal => {}
                order => return order,
            },
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
        }
    }
}
struct Meter<'a, 'b> {
    work: RefCell<&'a mut Work<'b>>,
    failed: Cell<bool>,
}
impl Meter<'_, '_> {
    fn visit(&self, units: usize) -> bool {
        if self.failed.get() {
            return false;
        }
        if (*self.work.borrow_mut())(units) {
            return true;
        }
        self.failed.set(true);
        false
    }
}
struct Query<'a, 'b, 'c, 'd, 'e> {
    head: &'a [u8],
    tail: &'b [u8],
    meter: &'c Meter<'d, 'e>,
}
impl Key for Query<'_, '_, '_, '_, '_> {
    fn parts(&self) -> (&[u8], &[u8]) {
        (self.head, self.tail)
    }
    fn metered(&self) -> bool {
        true
    }
    fn visit(&self, units: usize) -> bool {
        self.meter.visit(units)
    }
}
fn get<'a, V>(
    map: &'a BTreeMap<DeclarationId, V>,
    key: &DeclarationId,
    suffix: &[u8],
    work: &mut Work<'_>,
) -> Result<Option<&'a V>, ()> {
    // Empty maps still authenticate the held caller and its sticky refusal.
    units(work, 0)?;
    let meter = Meter {
        work: RefCell::new(work),
        failed: Cell::new(false),
    };
    let query = Query {
        head: key.as_str().as_bytes(),
        tail: suffix,
        meter: &meter,
    };
    let result = map.get(&query as &dyn Key);
    if meter.failed.get() {
        // Equal is only a read-search stopping sentinel after refusal. It has
        // no semantic authority: discard the entire value, even if get found
        // one. This key may never be used for insertion/removal/mutation.
        Err(())
    } else {
        Ok(result)
    }
}

fn get_text<'a, V>(
    map: &'a BTreeMap<String, V>,
    key: &str,
    work: &mut Work<'_>,
) -> Result<Option<&'a V>, ()> {
    units(work, 0)?;
    let meter = Meter {
        work: RefCell::new(work),
        failed: Cell::new(false),
    };
    let query = Query {
        head: key.as_bytes(),
        tail: &[],
        meter: &meter,
    };
    let result = map.get(&query as &dyn Key);
    if meter.failed.get() {
        Err(())
    } else {
        Ok(result)
    }
}

pub(crate) struct Probes<'a>(&'a DeclarationIndex);
impl DeclarationIndex {
    pub(crate) fn inventory_probes(&self) -> Probes<'_> {
        Probes(self)
    }
}
impl Index for Probes<'_> {
    fn declaration(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&Declaration>, ()> {
        get(&self.0.declarations, key, &[], work)
    }
    fn parameters(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&Vec<ResolvedTypeParameterDeclaration>>, ()> {
        get(&self.0.type_parameters, key, &[], work).map(|found| found.map(|values| &**values))
    }
    fn fields(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&Vec<ResolvedFieldDeclaration>>, ()> {
        get(&self.0.record_fields, key, &[], work).map(|found| found.map(|values| &**values))
    }
}
impl<'a> Probes<'a> {
    pub(crate) fn cached_identity(
        &self,
        key: &str,
        work: &mut Work<'_>,
    ) -> Result<Option<&'a TypeFacts>, ()> {
        get_text(&self.0.type_facts_by_id, key, work)
    }
    pub(crate) fn named_type(
        &self,
        name: &str,
        work: &mut Work<'_>,
    ) -> Result<Option<&'a DeclarationId>, ()> {
        get_text(&self.0.types_by_name, name, work)
    }
    pub(crate) fn case_rows(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&'a Vec<ResolvedFieldDeclaration>>, ()> {
        get(&self.0.case_fields, key, &[], work).map(|found| found.map(|fields| &**fields))
    }

    pub(crate) fn declared(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&'a Declaration>, ()> {
        get(&self.0.declarations, key, &[], work)
    }
    pub(crate) fn parameter_rows(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&'a Vec<ResolvedTypeParameterDeclaration>>, ()> {
        get(&self.0.type_parameters, key, &[], work).map(|found| found.map(|values| &**values))
    }
    pub(crate) fn field_rows(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&'a Vec<ResolvedFieldDeclaration>>, ()> {
        get(&self.0.record_fields, key, &[], work).map(|found| found.map(|values| &**values))
    }
    pub(crate) fn cases(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&'a [ResolvedVariantCaseDeclaration]>, ()> {
        Ok(get(&self.0.variant_cases, key, &[], work)?.map(|values| values.as_slice()))
    }
    pub(crate) fn has_invariant(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<bool, ()> {
        Ok(get(&self.0.declarations, key, b"#invariant", work)?.is_some())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
impl DeclarationIndex {
    pub(crate) fn inventory_nested_forgeries_for_test(&self) -> Vec<Self> {
        (0..8)
            .map(|mode| {
                let mut index = self.clone();
                let outcome = DeclarationId::new("n.Outcome");
                let payload = DeclarationId::new("n.Payload");
                match mode {
                    0 => {
                        index
                            .declarations
                            .get_mut(&payload)
                            .unwrap()
                            .identity_origin = IdentityOrigin::Automatic
                    }
                    1 => index.variant_cases.get_mut(&outcome).unwrap().swap(0, 1),
                    2 => {
                        index.variant_cases.get_mut(&outcome).unwrap()[1].fields[1].ty =
                            ResolvedType::I64
                    }
                    3 => {
                        index.record_fields.get_mut(&payload).unwrap()[0].ty =
                            ResolvedType::Nominal {
                                declaration: payload.clone(),
                                arguments: Vec::new(),
                            }
                    }
                    4 => {
                        index.type_parameters.remove(&payload);
                    }
                    5 => {
                        let id = DeclarationId::new("n.Payload#invariant");
                        let mut declaration = index.declarations[&payload].clone();
                        declaration.id = id.clone();
                        index.declarations.insert(id, declaration);
                    }
                    6 => index
                        .declarations
                        .get_mut(&DeclarationId::new("n.Outcome.value"))
                        .unwrap()
                        .name
                        .push_str("foreign"),
                    _ => {
                        index.record_fields.remove(&payload);
                    }
                }
                index
            })
            .collect()
    }
}

mod nominal;

struct CompoundQuery<'a, 'b, 'c, 'd> {
    plan: &'a compound::Plan<'a>,
    meter: &'b Meter<'c, 'd>,
}
impl Key for CompoundQuery<'_, '_, '_, '_> {
    fn parts(&self) -> (&[u8], &[u8]) {
        (&[], &[])
    }
    fn plan(&self) -> Option<&compound::Plan<'_>> {
        Some(self.plan)
    }
    fn metered(&self) -> bool {
        true
    }
    fn visit(&self, units: usize) -> bool {
        self.meter.visit(units)
    }
}
impl<'a> Probes<'a> {
    /// None means only that the unchanged inline optimization census declined;
    /// the caller must then use its complete paid owned canonical key. Err
    /// withholds every provisional read after first work refusal.
    pub(crate) fn cached_compound(
        &self,
        ty: &ResolvedType,
        work: &mut Work<'_>,
    ) -> Result<Option<Option<&'a TypeFacts>>, ()> {
        units(work, 0)?;
        let meter = Meter {
            work: RefCell::new(work),
            failed: Cell::new(false),
        };
        let plan = compound::Plan::new(ty, &|n| meter.visit(n));
        if meter.failed.get() {
            return Err(());
        }
        let Some(plan) = plan else {
            return Ok(None);
        };
        let query = CompoundQuery {
            plan: &plan,
            meter: &meter,
        };
        let result = self.0.type_facts_by_id.get(&query as &dyn Key);
        if meter.failed.get() {
            Err(())
        } else {
            Ok(Some(result))
        }
    }
}

#[cfg(test)]
impl DeclarationIndex {
    pub(crate) fn inventory_insert_fact_for_test(&mut self, ty: &ResolvedType, facts: TypeFacts) {
        self.type_facts_by_id.insert(ty.identity_key(), facts);
    }
}
