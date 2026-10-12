//! Fixed byte-pass index: pointer ranks affect access, never charged work.
use super::*;
fn reserve<T>(count: usize, admission: &mut Admission<'_>) -> Result<Vec<T>, Refusal> {
    let bytes = admission.arithmetic(count.checked_mul(std::mem::size_of::<T>()))?;
    admission.reserve(bytes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Refusal::Allocation)?;
    if values.capacity() != count {
        if values.capacity() > count {
            let delta = admission
                .arithmetic((values.capacity() - count).checked_mul(std::mem::size_of::<T>()))?;
            admission.reserve(delta)?;
        }
        return admission.refuse(Refusal::Capacity);
    }
    Ok(values)
}
pub(super) fn positions(
    directories: &[Directory<'_>],
    work: &mut dyn FnMut(usize) -> bool,
) -> Result<Vec<(usize, usize)>, Diagnostic> {
    let mut backing = crate::bounded_output::reserve_active_required;
    let mut admission = Admission::new(work, &mut backing);
    build(directories, &mut admission).map_err(|_| resolved::census_error())
}
fn build(
    directories: &[Directory<'_>],
    admission: &mut Admission<'_>,
) -> Result<Vec<(usize, usize)>, Refusal> {
    admission.reserve(std::mem::size_of::<Vec<(usize, usize)>>())?;
    let mut entries = reserve(directories.len(), admission)?;
    for (ordinal, directory) in directories.iter().enumerate() {
        admission.units(1)?;
        entries.push((directory.function_key(), ordinal));
    }
    radix(&mut entries, admission)?;
    Ok(entries)
}
fn radix(entries: &mut [(usize, usize)], admission: &mut Admission<'_>) -> Result<(), Refusal> {
    if entries.len() < 2 {
        return Ok(());
    }
    admission.reserve(
        std::mem::size_of::<[usize; 256]>() + std::mem::size_of::<Vec<(usize, usize)>>(),
    )?;
    admission.units(256)?;
    let mut offsets = [0usize; 256];
    let mut scratch = reserve(entries.len(), admission)?;
    admission.units(entries.len())?;
    scratch.extend_from_slice(&entries);
    for byte in 0..std::mem::size_of::<usize>() {
        admission.units(256)?;
        offsets.fill(0);
        for item in entries.iter() {
            admission.units(1)?;
            offsets[(item.0 >> (byte * 8)) & 255] += 1;
        }
        let mut start = 0;
        for offset in &mut offsets {
            admission.units(1)?;
            let length = *offset;
            *offset = start;
            start += length;
        }
        for item in entries.iter() {
            admission.units(1)?;
            let digit = (item.0 >> (byte * 8)) & 255;
            scratch[offsets[digit]] = *item;
            offsets[digit] += 1;
        }
        admission.units(entries.len())?;
        entries.copy_from_slice(&scratch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_opposing_ranks_have_literal_equal_work_backing_and_one_short() {
        let expected_work = 256 + 3 + std::mem::size_of::<usize>() * (512 + 9);
        let expected_backing = (256 + 3 + 6) * std::mem::size_of::<usize>();
        for keys in [[1usize, 2, 3], [3, 2, 1], [usize::MAX, 0, 17]] {
            for allowance in [expected_work, expected_work - 1] {
                let remaining = std::cell::Cell::new(allowance);
                let backing = std::cell::Cell::new(expected_backing);
                let failed = std::cell::Cell::new(false);
                let mut entries = [(keys[0], 0), (keys[1], 1), (keys[2], 2)];
                let (success, observed) = crate::test_allocation_observer::measure(|| {
                    let mut charge = |units| match remaining.get().checked_sub(units) {
                        Some(n) if !failed.get() => {
                            remaining.set(n);
                            true
                        }
                        _ => {
                            failed.set(true);
                            false
                        }
                    };
                    let mut reserve = |bytes| match backing.get().checked_sub(bytes) {
                        Some(n) if !failed.get() => {
                            backing.set(n);
                            true
                        }
                        _ => {
                            failed.set(true);
                            false
                        }
                    };
                    let mut admission = Admission::new(&mut charge, &mut reserve);
                    let success = radix(&mut entries, &mut admission).is_ok();
                    if !success {
                        assert!(admission.reserve(0).is_err());
                    }
                    success
                });
                assert_eq!(success, allowance == expected_work);
                if success {
                    assert!(entries.windows(2).all(|w| w[0].0 < w[1].0));
                    assert_eq!(remaining.get(), 0);
                }
                assert!(!observed.invalid);
                assert_eq!(observed.live, 0);
                assert_eq!(observed.allocated, 6 * std::mem::size_of::<usize>());
                assert!(observed.allocated <= expected_backing - backing.get());
            }
        }
    }
}
