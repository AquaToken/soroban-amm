use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum ConcentratedPoolError {
    Unauthorized = 102,

    PoolAlreadyInitialized = 201,
    PlaneAlreadyInitialized = 202,
    RewardsAlreadyInitialized = 203,
    DepositKilled = 205,
    SwapKilled = 206,
    ClaimKilled = 207,
    PoolNotInitialized = 211,
    PositionNotFound = 212,
    InsufficientLiquidity = 213,
    LiquidityOverflow = 214,
    LiquidityUnderflow = 215,

    InvalidTickRange = 2101,
    InvalidAmount = 2103,
    InvalidSqrtPrice = 2104,
    InvalidFee = 2105,
    InvalidTickSpacing = 2106,
    TickOutOfBounds = 2107,
    PriceOutOfBounds = 2108,
    TickNotSpacedCorrectly = 2109,
    TickLowerNotLessThanUpper = 2110,
    TickLowerTooLow = 2111,
    TickUpperTooHigh = 2112,
    InvalidPriceLimit = 2113,
    AmountShouldBeGreaterThanZero = 2114,
    InsufficientToken0 = 2116,
    InsufficientToken1 = 2117,
    TooManyPositions = 2119,
    LiquidityAmountTooLarge = 2120,
    TokensNotSorted = 2121,
}
