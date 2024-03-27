use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum LiquidityPoolRouterError {
    PoolNotFound = 301,
    BadFee = 302,
    StableswapHashMissing = 303,
    StableswapUnsupportedTokensCount = 304,
    PoolsOverMax = 305,
    StableswapPoolsOverMax = 306,
    PathIsEmpty = 307,
}
