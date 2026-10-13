//! Allocation-free, read-only current metadata probes. Ordinary get and
//! invariant lookup stay unchanged. Refused comparison results never escape.
use super::*;
use crate::hir::renewal_probes::{units, Index, Work};
use std::borrow::Borrow;
use std::cell::{Cell, RefCell};
use std::cmp::Ordering;

trait Key {
    fn parts(&self) -> (&[u8], &[u8]);
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
        let (a, a_tail) = self.parts();
        let (b, b_tail) = other.parts();
        let mut a = a.iter().chain(a_tail).copied();
        let mut b = b.iter().chain(b_tail).copied();
        loop {
            // Pay before fetching/comparing the next complete UTF-8 byte pair.
            if !work.visit(1) {
                return Ordering::Equal;
            }
            match (a.next(), b.next()) {
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
        get(&self.0.type_parameters, key, &[], work)
    }
    fn fields(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&Vec<ResolvedFieldDeclaration>>, ()> {
        get(&self.0.record_fields, key, &[], work)
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
        get(&self.0.case_fields, key, &[], work)
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
        get(&self.0.type_parameters, key, &[], work)
    }
    pub(crate) fn field_rows(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&'a Vec<ResolvedFieldDeclaration>>, ()> {
        get(&self.0.record_fields, key, &[], work)
    }
    pub(crate) fn cases(
        &self,
        key: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&'a [ResolvedVariantCaseDeclaration]>, ()> {
        Ok(get(&self.0.variant_cases, key, &[], work)?.map(Vec::as_slice))
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
