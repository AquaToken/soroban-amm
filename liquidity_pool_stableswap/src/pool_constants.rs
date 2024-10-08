pub const FEE_DENOMINATOR: u32 = 10000; // 0.01% = 0.0001 = 1 / 10000
pub const MAX_ADMIN_FEE: u32 = 10000; // maximum 100% of fee can go to admin
pub const MAX_FEE: u32 = 5000; // maximum allowed fee is 50%
pub const MAX_A: u128 = 1_000_000; // absolute maximum value for A
pub const MAX_A_CHANGE: u128 = 10; // maximum multiplier allowed for a change in 'A'

pub const ADMIN_ACTIONS_DELAY: u64 = 3 * 86400; // allow changes once per 3 days
pub const MIN_RAMP_TIME: u64 = 86400; // minimum time for ramping. ensures that changes occur
                                      //    over a minimum duration to prevent abrupt shifts.
