pub const N_COINS: usize = 2; // <- change
                              // pub const N_COINS: usize = 3; // <- change
pub const FEE_DENOMINATOR: u128 = 10000; // 0.01% = 0.0001 = 1 / 10000
pub const LENDING_PRECISION: u128 = 1_0000000;
pub const PRECISION: u128 = 1_0000000; // The precision to convert to
pub const PRECISION_MUL: [u128; N_COINS] = [1, 1];
// pub const PRECISION_MUL: [u128; N_COINS] = [1, 1, 1];
pub const RATES: [u128; N_COINS] = [1_0000000, 1_0000000];
// pub const RATES: [u128; N_COINS] = [1_0000000, 1_0000000, 1_0000000];
pub const MAX_ADMIN_FEE: u128 = 100_0000000;
pub const MAX_FEE: u128 = 100000;
pub const MAX_A: u128 = 10000000;
pub const MAX_A_CHANGE: u128 = 100;

pub const ADMIN_ACTIONS_DELAY: u64 = 3 * 86400;
pub const MIN_RAMP_TIME: u64 = 86400;

pub const KILL_DEADLINE_DT: u64 = 2 * 30 * 86400;
