use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum ConcentratedPoolError {
    // Shared with access_control, gauge, provider_swap_fee
    Unauthorized = 102,

    // Shared with standard pool, stableswap (201-207)
    PoolAlreadyInitialized = 201,
    PlaneAlreadyInitialized = 202,
    RewardsAlreadyInitialized = 203,
    DepositKilled = 205,
    SwapKilled = 206,
    ClaimKilled = 207,

    // Shared with router
    TokensNotSorted = 2002,

    // Concentrated pool specific (21xx)
    InvalidTickRange = 2101,
    InvalidAmount = 2103,
    InvalidSqrtPrice = 2104,
    InvalidTickSpacing = 2106,
    TickOutOfBounds = 2107,
    PriceOutOfBounds = 2108,
    TickNotSpacedCorrectly = 2109,
    TickLowerNotLessThanUpper = 2110,
    TickLowerTooLow = 2111,
    TickUpperTooHigh = 2112,
    InvalidPriceLimit = 2113,
    PositionNotFound = 2118,
    TooManyPositions = 2119,
    LiquidityAmountTooLarge = 2120,
    InsufficientLiquidity = 2121,
    LiquidityOverflow = 2122,
    LiquidityUnderflow = 2123,
}
