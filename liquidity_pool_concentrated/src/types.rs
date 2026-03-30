use soroban_sdk::{contracttype, Address, Vec, U256};

// Current pool price state. Stored in instance storage (DataKey::Slot0).
// Updated on every swap. sqrt_price_x96 = sqrt(token1/token0) * 2^96 (Q64.96 fixed-point).
// tick = floor(log_{1.0001}(price)), always satisfies: sqrt_ratio_at_tick(tick) <= sqrt_price_x96.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Slot0 {
    pub sqrt_price_x96: U256,
    pub tick: i32,
}

// Per-position state. Stored in persistent storage keyed by (owner, tick_lower, tick_upper).
// fee_growth_inside_*_last = snapshot of cumulative fee growth inside the range at last interaction.
// tokens_owed = uncollected fees + withdrawn tokens pending claim_position_fees.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PositionData {
    pub fee_growth_inside_0_last_x128: U256,
    pub fee_growth_inside_1_last_x128: U256,
    pub liquidity: u128,
    pub tokens_owed_0: u128,
    pub tokens_owed_1: u128,
}

// Per-tick state used throughout the contract and as the public API type.
// fee_growth_outside = fee growth accumulated on the "other side" of this tick.
//   Used to compute fee growth inside any [lower, upper] range via:
//   inside = global - below(lower) - above(upper).
// liquidity_gross = total liquidity referencing this tick.
//   Tick is considered initialized when liquidity_gross > 0.
// liquidity_net = signed delta applied to active liquidity when price crosses this tick.
//   Positive at lower boundaries (liquidity enters), negative at upper (liquidity exits).
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TickInfo {
    pub fee_growth_outside_0_x128: U256,
    pub fee_growth_outside_1_x128: U256,
    pub liquidity_gross: u128,
    pub liquidity_net: i128,
}

// Tuple-encoded storage representation for TickInfo.
// Eliminates XDR field-name key overhead (~50% of serialized size with named structs).
// Order: (fee_growth_outside_0, fee_growth_outside_1, liquidity_gross, liquidity_net).
// Only used inside storage accessors; immediately converted to/from TickInfo.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TickData(pub U256, pub U256, pub u128, pub i128);

impl From<TickData> for TickInfo {
    fn from(d: TickData) -> Self {
        TickInfo {
            fee_growth_outside_0_x128: d.0,
            fee_growth_outside_1_x128: d.1,
            liquidity_gross: d.2,
            liquidity_net: d.3,
        }
    }
}

impl From<TickInfo> for TickData {
    fn from(t: TickInfo) -> Self {
        TickData(
            t.fee_growth_outside_0_x128,
            t.fee_growth_outside_1_x128,
            t.liquidity_gross,
            t.liquidity_net,
        )
    }
}

// Returned by internal swap execution. Signed amounts: positive = user paid, negative = user received.
// Includes final pool state after swap.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SwapResult {
    pub amount0: i128,
    pub amount1: i128,
    pub liquidity: u128,
    pub sqrt_price_x96: U256,
    pub tick: i32,
}

// Accumulated protocol fees (admin's cut of swap fees). Stored in instance storage.
// Collected via claim_protocol_fees.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ProtocolFees {
    pub token0: u128,
    pub token1: u128,
}

// Tick range identifier for a position. Used in UserState.positions list.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PositionRange {
    pub tick_lower: i32,
    pub tick_upper: i32,
}

// Merged per-user state. Single persistent storage entry per user.
// positions = list of active tick ranges (max MAX_USER_POSITIONS).
// raw_liquidity = sum of all position liquidity amounts (unweighted).
// weighted_liquidity = raw * distance_multiplier (for rewards distribution).
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct UserState {
    pub positions: Vec<PositionRange>,
    pub raw_liquidity: u128,
    pub weighted_liquidity: u128,
}

// Full pool configuration + price state. Returned by get_full_pool_state.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PoolState {
    pub fee: u32,
    pub liquidity: u128,
    pub sqrt_price_x96: U256,
    pub tick: i32,
    pub tick_spacing: i32,
    pub token0: Address,
    pub token1: Address,
}

// Pool state + actual token balances. Returned by get_pool_state_with_balances.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PoolStateWithBalances {
    pub reserve0: i128,
    pub reserve1: i128,
    pub state: PoolState,
}

// Read-only view of user state. Returned by get_user_position_snapshot.
// Mirrors UserState fields for external queries.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct UserPositionSnapshot {
    pub ranges: Vec<PositionRange>,
    pub raw_liquidity: u128,
    pub weighted_liquidity: u128,
}
