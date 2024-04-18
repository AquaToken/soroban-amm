use soroban_sdk::U256;

pub fn assert_approx_eq_abs(a: u128, b: u128, delta: u128) {
    assert!(
        a > b - delta && a < b + delta,
        "assertion failed: `(left != right)` \
         (left: `{:?}`, right: `{:?}`, epsilon: `{:?}`)",
        a,
        b,
        delta
    );
}

pub fn assert_approx_eq_abs_u256(a: U256, b: U256, delta: U256) {
    assert!(
        a > b.sub(&delta) && a < b.add(&delta),
        "assertion failed: `(left != right)` \
         (left: `{:?}`, right: `{:?}`, epsilon: `{:?}`)",
        a,
        b,
        delta
    );
}
