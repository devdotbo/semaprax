//! Requested-Layout observation is independent of reservation arithmetic.
use super::*;
use crate::test_allocation_observer;
const EXPECTED: usize = 255 * std::mem::size_of::<u64>();
#[test]
fn geometric_growth_precharges_every_observed_replacement_and_reuses_spare() {
    let ((charged, observed), refused, used) =
        crate::bounded_output::with_limit_usage(EXPECTED, || {
            let mut physical = PhysicalReservation::new(EXPECTED);
            let (charged, observed) = test_allocation_observer::measure(|| {
                let mut values = Vec::<u64>::new();
                for value in 0..100 {
                    replay_reserve_vec(&mut physical, &mut values, 1).unwrap();
                    values.push(value);
                }
                assert_eq!(values.capacity(), 128);
                let before = physical.charged();
                replay_reserve_vec(&mut physical, &mut values, 1).unwrap();
                assert_eq!(physical.charged(), before);
                assert_eq!(values.iter().copied().sum::<u64>(), 4950);
                physical.charged()
            });
            (charged, observed)
        });
    assert!(!refused && !observed.invalid);
    assert_eq!(charged, EXPECTED);
    assert_eq!(used, EXPECTED);
    assert_eq!(observed.allocated, EXPECTED);
    assert!(observed.highwater <= charged);
    assert_eq!(observed.live, 0);
}
#[test]
fn one_short_growth_refuses_before_replacement_and_spare_cannot_reset_refusal() {
    let ((result, observed), refused, used) =
        crate::bounded_output::with_limit_usage(EXPECTED, || {
            let mut physical = PhysicalReservation::new(EXPECTED - 1);
            let (result, observed) = test_allocation_observer::measure(|| {
                let mut values = Vec::<u64>::new();
                for value in 0..64 {
                    replay_reserve_vec(&mut physical, &mut values, 1).unwrap();
                    values.push(value);
                }
                let before = physical.charged();
                assert_eq!(
                    replay_reserve_vec(&mut physical, &mut values, 1),
                    Err(PhysicalRefusal::LocalLimit)
                );
                assert_eq!(values.capacity(), 64);
                assert_eq!(values.len(), 64);
                assert_eq!(physical.charged(), before);
                assert_eq!(
                    replay_reserve_vec(&mut physical, &mut values, 0),
                    Err(PhysicalRefusal::LocalLimit)
                );
                before
            });
            (result, observed)
        });
    assert!(!refused && !observed.invalid);
    assert_eq!(used, result);
    assert_eq!(observed.allocated, result);
    assert_eq!(observed.live, 0);
}
