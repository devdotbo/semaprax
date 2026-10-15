//! Complete zero-argument nominal cache key without a temporary owned String.
//! This immutable query never mutates a cache or publishes a refused result.
use super::*;
struct Nominal<'a, 'b, 'c, 'd> {
    identity: &'a [u8],
    decimal: [u8; usize::BITS as usize],
    start: usize,
    meter: &'b Meter<'c, 'd>,
}
impl Key for Nominal<'_, '_, '_, '_> {
    fn parts(&self) -> (&[u8], &[u8]) {
        (&[], &[])
    }
    fn segments(&self) -> [&[u8]; 5] {
        [
            b"nominal:",
            &self.decimal[self.start..],
            b":",
            self.identity,
            b":0:",
        ]
    }
    fn metered(&self) -> bool {
        true
    }
    fn visit(&self, units: usize) -> bool {
        self.meter.visit(units)
    }
}
pub(super) fn get<'a, V>(
    map: &'a BTreeMap<String, V>,
    identity: &DeclarationId,
    work: &mut Work<'_>,
) -> Result<Option<&'a V>, ()> {
    units(work, 0)?;
    let mut decimal = [0; usize::BITS as usize];
    let mut start = decimal.len();
    let mut length = identity.as_str().len();
    loop {
        units(work, 1)?;
        start -= 1;
        decimal[start] = b'0' + (length % 10) as u8;
        length /= 10;
        if length == 0 {
            break;
        }
    }
    let meter = Meter {
        work: RefCell::new(work),
        failed: Cell::new(false),
    };
    let query = Nominal {
        identity: identity.as_str().as_bytes(),
        decimal,
        start,
        meter: &meter,
    };
    let result = map.get(&query as &dyn Key);
    if meter.failed.get() {
        Err(())
    } else {
        Ok(result)
    }
}
impl<'a> Probes<'a> {
    pub(crate) fn cached_zero_argument(
        &self,
        declaration: &DeclarationId,
        work: &mut Work<'_>,
    ) -> Result<Option<&'a TypeFacts>, ()> {
        get(&self.0.type_facts_by_id, declaration, work)
    }
}
