use soroban_sdk::{contracttype, Address, Vec, U256};

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Slot0 {
    pub sqrt_price_x96: U256,
    pub tick: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PositionData {
    pub fee_growth_inside_0_last_x128: U256,
    pub fee_growth_inside_1_last_x128: U256,
    pub liquidity: u128,
    pub tokens_owed_0: u128,
    pub tokens_owed_1: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TickInfo {
    pub fee_growth_outside_0_x128: U256,
    pub fee_growth_outside_1_x128: U256,
    pub initialized: bool,
    pub liquidity_gross: u128,
    pub liquidity_net: i128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SwapResult {
    pub amount0: i128,
    pub amount1: i128,
    pub liquidity: u128,
    pub sqrt_price_x96: U256,
    pub tick: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ProtocolFees {
    pub token0: u128,
    pub token1: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PositionRange {
    pub tick_lower: i32,
    pub tick_upper: i32,
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PoolStateWithBalances {
    pub reserve0: i128,
    pub reserve1: i128,
    pub state: PoolState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct UserPositionSnapshot {
    pub ranges: Vec<PositionRange>,
    pub raw_liquidity: u128,
    pub weighted_liquidity: u128,
}
