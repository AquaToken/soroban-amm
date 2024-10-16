pub const FEE_DENOMINATOR: u32 = 10000; // 0.01% = 0.0001 = 1 / 10000
pub const PRICE_PRECISION: u128 = 1_0000000; // Price precision
pub const MAX_FEE: u32 = 5000; // maximum allowed fee is 50%
pub const MAX_A: u128 = 1_000_000; // absolute maximum value for A
pub const MAX_A_CHANGE: u128 = 10; // maximum multiplier allowed for a change in 'A'

pub const MIN_RAMP_TIME: u64 = 86400; // minimum time for ramping. ensures that changes occur
                                      //    over a minimum duration to prevent abrupt shifts.
