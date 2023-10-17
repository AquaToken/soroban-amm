// pool constants
// todo: cleanup
// pub const N_COINS: usize = 3;  // <- change
pub const N_COINS: usize = 2; // <- change
pub const FEE_DENOMINATOR: u128 = 10 ^ 4;
pub const LENDING_PRECISION: u128 = 10 ^ 7;
pub const PRECISION: u128 = 10 ^ 7; // The precision to convert to
pub const PRECISION_MUL: [u128; N_COINS] = [1, 1];
pub const RATES: [u128; N_COINS] = [1_0000000, 1_0000000];
// pub const FEE_INDEX: int128 = 2;  // Which coin may potentially have fees (USDT)

// pub const MAX_ADMIN_FEE: uint256 = 10 * 10 ** 9;
pub const MAX_FEE: u128 = 5 * 10 ^ 9;
pub const MAX_A: u128 = 10 ^ 6;
pub const MAX_A_CHANGE: u128 = 10;

pub const ADMIN_ACTIONS_DELAY: u128 = 3 * 86400;
pub const MIN_RAMP_TIME: u128 = 86400;

// const coins: public(address[N_COINS])
// const balances: public(uint256[N_COINS])
// const fee: public(uint256)  // fee * 1e10
// const admin_fee: public(uint256)  // admin_fee * 1e10
//
// const owner: public(address)
// const token: CurveToken
//
// const initial_A: public(uint256)
// const future_A: public(uint256)
// const initial_A_time: public(uint256)
// const future_A_time: public(uint256)
//
// const admin_actions_deadline: public(uint256)
// const transfer_ownership_deadline: public(uint256)
// const future_fee: public(uint256)
// const future_admin_fee: public(uint256)
// const future_owner: public(address)
//
// const is_killed: bool = false
// const kill_deadline: uint256
pub const KILL_DEADLINE_DT: u64 = 2 * 30 * 86400;
