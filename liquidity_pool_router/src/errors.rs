use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum LiquidityPoolRouterError {
    PoolNotFound = 301,
    BadFee = 302,
    StableswapHashMissing = 303,
    PoolsOverMax = 305,
    StableswapPoolsOverMax = 306,
    PathIsEmpty = 307,
    TokensAreNotForReward = 308, // unable to find tokens in reward map
    LiquidityNotFilled = 309,    // liquidity info not available yet. run `fill_liquidity` first
    LiquidityAlreadyFilled = 310,
    VotingShareExceedsMax = 311, // total voting share exceeds 100%
    LiquidityCalculationError = 312,
    RewardsNotConfigured = 313, // unable to find rewards tokens. please run `config_rewards` first
    RewardsAlreadyConfigured = 314,
    DuplicatesNotAllowed = 315,
    InvalidPoolType = 316,

    TokensNotSorted = 2002,
    InMaxNotSatisfied = 2020,
}
