pub(crate) const FEE_MULTIPLIER: u128 = 10_000;

// `PRECISION` is a constant that is used to maintain the precision of calculations.
// It's a factor by which values are multiplied or divided to maintain precision during arithmetic operations.
// This is particularly useful when dealing with fractional values in integer calculations,
// as it allows the code to effectively handle decimal points.
pub(crate) const PRECISION: u128 = 1_000_000;

// `RESERVES_NORM` is a constant that is used to normalize the reserves in the liquidity pool.
// It's a reference value against which the actual reserves are compared and adjusted.
// This normalization process helps to maintain the balance in the liquidity pool and prevent
// any single reserve from becoming too large or too small.
pub(crate) const RESERVES_NORM: u128 = 1_000_0000000;
