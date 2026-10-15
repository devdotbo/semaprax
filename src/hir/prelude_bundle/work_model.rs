//! Compile-time work census over the sole fixed base schema. There is no
//! runtime census allocation, authored input, replay credit or guessed factor.
use crate::prelude::base_schema::BASE;

const fn count() -> usize {
    let mut total = BASE.len();
    let mut t = 0;
    while t < BASE.len() {
        let mut c = 0;
        while c < BASE[t].cases.len() {
            total += 1 + BASE[t].cases[c].fields.len();
            c += 1;
        }
        t += 1;
    }
    total
}
const COUNT: usize = count();
const fn order(a: &str, b: &str) -> i8 {
    let a = a.as_bytes();
    let b = b.as_bytes();
    let mut i = 0;
    while i < a.len() && i < b.len() {
        if a[i] < b[i] {
            return -1;
        }
        if a[i] > b[i] {
            return 1;
        }
        i += 1;
    }
    if a.len() < b.len() {
        -1
    } else if a.len() > b.len() {
        1
    } else {
        0
    }
}
const fn find(ids: &[&str; COUNT], text: &str) -> usize {
    let mut work = 0;
    let mut i = 0;
    while i < COUNT {
        work += ids[i].len() + text.len();
        if order(ids[i], text) == 0 {
            return work;
        }
        i += 1;
    }
    panic!("fixed base work model references an absent descriptor")
}
const fn required() -> usize {
    // contents entry, descriptor identities and two name-byte passes.
    let mut work = 1;
    let mut ids = [""; COUNT];
    let mut at = 0;
    let mut t = 0;
    while t < BASE.len() {
        let ty = BASE[t];
        ids[at] = ty.id;
        at += 1;
        work += ty.id.len() + 2 * ty.name.len();
        let mut c = 0;
        while c < ty.cases.len() {
            let case = &ty.cases[c];
            ids[at] = case.id;
            at += 1;
            work += case.id.len() + 2 * case.name.len();
            let mut f = 0;
            while f < case.fields.len() {
                let field = &case.fields[f];
                ids[at] = field.id;
                at += 1;
                work += field.id.len() + 2 * field.name.len();
                f += 1;
            }
            c += 1;
        }
        t += 1;
    }
    assert!(at == COUNT);
    // Exact insertion-sort key comparison sequence, with the same source order
    // and byte ordering as producer::contents. The census itself is const eval.
    let mut i = 1;
    while i < COUNT {
        let mut j = i;
        while j > 0 {
            work += ids[j - 1].len() + ids[j].len();
            let ordering = order(ids[j - 1], ids[j]);
            assert!(ordering != 0, "fixed base IDs must be unique");
            if ordering < 0 {
                break;
            }
            let previous = ids[j - 1];
            ids[j - 1] = ids[j];
            ids[j] = previous;
            j -= 1;
        }
        i += 1;
    }
    t = 0;
    while t < BASE.len() {
        let ty = BASE[t];
        work += find(&ids, ty.id) + 2 * ty.name.len();
        let mut p = 0;
        while p < ty.parameters.len() {
            work += 1 + 2 * ty.parameters[p].len();
            p += 1;
        }
        let mut c = 0;
        while c < ty.cases.len() {
            let case = &ty.cases[c];
            work += find(&ids, case.id) + 2 * case.name.len();
            let mut f = 0;
            while f < case.fields.len() {
                let field = &case.fields[f];
                work += find(&ids, field.id) + 2 * field.name.len();
                p = 0;
                let mut found = false;
                while p < ty.parameters.len() {
                    work += ty.parameters[p].len() + field.parameter.len();
                    if order(ty.parameters[p], field.parameter) == 0 {
                        found = true;
                        break;
                    }
                    p += 1;
                }
                assert!(found, "fixed base field must name a declared parameter");
                f += 1;
            }
            c += 1;
        }
        t += 1;
    }
    work
}
/// Logical units at the producer's existing explicit work sites, not CPU time.
pub(super) const REQUIRED: usize = required();
