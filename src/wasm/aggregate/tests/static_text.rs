//! Exact selected pool bounds, independent of source or guest input authority.
use super::super::OwnedUtf8Literals;

#[test]
fn selected_literal_pool_exact_bound_empty_and_one_over_are_truthful() {
    let mut pool = OwnedUtf8Literals {
        static_pool: true,
        ..Default::default()
    };
    let full = "a".repeat(65_536);
    let base = pool.base();
    assert_eq!(pool.intern(&full).unwrap(), (base, 65_536));
    assert_eq!(pool.intern("").unwrap(), (base, 0));
    assert_eq!(pool.bytes.len(), 65_536);
    assert_eq!(pool.intern(&full).unwrap(), (base, 65_536));
    assert!(pool.intern("b").is_err());
    assert_eq!(pool.bytes.len(), 65_536);
    let mut legacy = OwnedUtf8Literals::default();
    assert_eq!(legacy.intern(&full).unwrap(), (base, 65_536));
    assert_eq!(legacy.intern("").unwrap(), (base + 65_536, 0));
}
