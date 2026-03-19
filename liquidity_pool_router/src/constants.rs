pub(crate) const MAX_POOLS_FOR_PAIR: u32 = 10;
pub(crate) const CONSTANT_PRODUCT_FEE_AVAILABLE: [u32; 3] = [10, 30, 100];
pub(crate) const CONCENTRATED_FEE_AVAILABLE: [u32; 3] = [10, 30, 100];
pub(crate) const STABLESWAP_MAX_POOLS: u32 = 3;
pub(crate) const CONCENTRATED_MAX_POOLS: u32 = 3;
pub(crate) const STABLESWAP_MAX_FEE: u32 = 100; // 1%
pub(crate) const STABLESWAP_DEFAULT_A: u128 = 750;
pub(crate) const STABLESWAP_MAX_TOKENS: u32 = 3;

// Derives tick spacing from fee tier.
// Spacing is chosen to keep tick crossings within Soroban's 200 read-entry
// storage footprint limit for realistic single-tx price moves (~30-50%).
// At spacing=20 the 0.1% tier handles a 1.3x move (131 crossings) comfortably;
// a 1.5x move (203 crossings) reverts — acceptable for correlated pairs.
pub(crate) fn concentrated_tick_spacing(fee: u32) -> i32 {
    match fee {
        10 => 20,
        30 => 60,
        100 => 200,
        _ => panic!("unsupported concentrated fee tier"),
    }
}
