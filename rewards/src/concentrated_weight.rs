// Distance-weighted rewards for concentrated liquidity positions.
//
// Positions that are in-range (overlapping current price) receive full reward weight.
// Out-of-range positions receive a quadratically decaying multiplier that reaches
// zero at `max_distance_for_fee(fee)` ticks from the nearest range boundary.
//
// ## Why distance weighting?
//
// In Uniswap V3 math, a position's liquidity (L) depends on range width and deposited
// capital, but NOT on where the range sits relative to current price. Two $100 positions
// with the same 1% width produce nearly identical L regardless of distance from price.
// Without distance weighting, a far out-of-range position that serves zero swaps earns
// the same AQUA rewards as an active in-range position with the same L.
//
// ## Why fee-based max distance?
//
// The fee tier is a proxy for pair volatility:
// - fee=10 (0.1%): stable pairs (USDC/PYUSD) — price stays within ~1%
// - fee=30 (0.3%): standard pairs (XLM/USDC) — price can move ~50% over months
// - fee=100 (1.0%): exotic pairs (meme tokens) — price can move 10x+
//
// A position 500 ticks (~5%) from price is useless for stablecoins but perfectly
// reasonable for volatile pairs. Tying max_distance to fee tier calibrates the
// penalty to the pair's expected price movement.
//
// ## Multiplier formula
//
// ```text
// max_distance = fee × 50
//
// multiplier(d) = | 10000 bps (100%)              if d == 0 (in-range)
//                 | ((max_distance - d) / max_distance)² × 10000 bps   if 0 < d < max_distance
//                 | 0 bps                          if d >= max_distance
// ```
//
// | Fee | Max distance | Price range to zero | Example: d=500 ticks (~5%) |
// |-----|-------------|---------------------|---------------------------|
// | 10  | 500 ticks   | ~5.1%               | 0% (cutoff)               |
// | 30  | 1,500 ticks | ~16.2%              | 44% (mild decay)          |
// | 100 | 5,000 ticks | ~64.9%              | 81% (barely affected)     |

pub const BPS_DENOMINATOR: u32 = 10_000;

// Floor multiplier for positions at or beyond max distance. Zero means
// far out-of-range positions receive no rewards at all.
pub const MIN_MULTIPLIER_BPS: u32 = 0;

// Multiplier for converting fee tier (in basis points) to max distance in ticks.
// fee=10 → 500 ticks (~5.1% price), fee=30 → 1500 (~16.2%), fee=100 → 5000 (~64.9%).
pub const FEE_TO_DISTANCE_MULTIPLIER: u32 = 50;

pub fn max_distance_for_fee(fee: u32) -> u32 {
    fee * FEE_TO_DISTANCE_MULTIPLIER
}

// Tick distance from current price to the nearest edge of [tick_lower, tick_upper).
// Returns 0 if current price is inside the range.
// Uses half-open interval matching active-liquidity semantics: tick_current == tick_upper
// is OUT of range (position does not supply active swap liquidity at this tick).
pub fn tick_distance_from_range(tick_current: i32, tick_lower: i32, tick_upper: i32) -> u32 {
    if tick_lower >= tick_upper {
        return 0;
    }

    if tick_current < tick_lower {
        (tick_lower as i64 - tick_current as i64) as u32
    } else if tick_current >= tick_upper {
        // tick_current == tick_upper → boundary, distance 0 from edge but NOT in-range.
        // Use max(1, ..) to ensure at least minimal out-of-range penalty.
        ((tick_current as i64 - tick_upper as i64) as u32).max(1)
    } else {
        0
    }
}

// Quadratic decay from BPS_DENOMINATOR (100%) at distance=0 to MIN_MULTIPLIER_BPS
// at distance=max_distance_ticks. Returns MIN_MULTIPLIER_BPS for any distance beyond max.
pub fn distance_multiplier_bps(distance_ticks: u32, max_distance_ticks: u32) -> u32 {
    if distance_ticks == 0 {
        return BPS_DENOMINATOR;
    }

    if max_distance_ticks == 0 || distance_ticks >= max_distance_ticks {
        return MIN_MULTIPLIER_BPS;
    }

    let max_dist = max_distance_ticks as u128;
    let rem = (max_distance_ticks - distance_ticks) as u128;
    let dynamic_range = (BPS_DENOMINATOR - MIN_MULTIPLIER_BPS) as u128;

    let scaled = dynamic_range * rem * rem / (max_dist * max_dist);
    MIN_MULTIPLIER_BPS + scaled as u32
}

// Reward multiplier for a position given current tick and pool fee.
// Combines tick_distance_from_range + distance_multiplier_bps + max_distance_for_fee.
pub fn position_multiplier_bps(
    tick_current: i32,
    tick_lower: i32,
    tick_upper: i32,
    fee: u32,
) -> u32 {
    let distance = tick_distance_from_range(tick_current, tick_lower, tick_upper);
    distance_multiplier_bps(distance, max_distance_for_fee(fee))
}

// Scale an amount by a multiplier in basis points. Capped at BPS_DENOMINATOR (100%).
pub fn apply_multiplier(amount: u128, multiplier_bps: u32) -> u128 {
    let multiplier = multiplier_bps.min(BPS_DENOMINATOR) as u128;
    amount.saturating_mul(multiplier) / BPS_DENOMINATOR as u128
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_range_has_full_weight() {
        assert_eq!(position_multiplier_bps(100, 90, 110, 30), BPS_DENOMINATOR);
    }

    #[test]
    fn test_fee_determines_max_distance() {
        // fee=10 → max_distance=500
        // At distance=250 (half): (250/500)² = 0.25 → 2500 bps
        assert_eq!(distance_multiplier_bps(250, 500), 2_500);
        assert_eq!(distance_multiplier_bps(500, 500), 0);
        assert_eq!(distance_multiplier_bps(600, 500), 0);

        // fee=100 → max_distance=5000
        // Same absolute distance=250 is only 5% of max → mild decay
        assert_eq!(distance_multiplier_bps(250, 5_000), 9_025);
    }

    #[test]
    fn test_stablecoin_penalizes_far_positions() {
        // Stablecoin pool (fee=10): position 903 ticks away exceeds max_distance=500 → 0
        assert_eq!(position_multiplier_bps(0, 903, 1003, 10), 0);
        // Same position in volatile pool (fee=100): max_distance=5000 → still rewarded
        assert!(position_multiplier_bps(0, 903, 1003, 100) > 0);
    }

    #[test]
    fn test_apply_multiplier() {
        assert_eq!(apply_multiplier(1_000, BPS_DENOMINATOR), 1_000);
        assert_eq!(apply_multiplier(1_000, 2_500), 250);
        // Capped at 100%
        assert_eq!(apply_multiplier(1_000, 20_000), 1_000);
    }

    #[test]
    fn test_upper_boundary_is_out_of_range() {
        // tick_current == tick_upper: half-open interval [lower, upper) → out-of-range
        assert_eq!(tick_distance_from_range(110, 90, 110), 1);
        // tick_current == tick_upper - 1: still in-range
        assert_eq!(tick_distance_from_range(109, 90, 110), 0);
        // tick_current == tick_lower: in-range (lower bound is inclusive)
        assert_eq!(tick_distance_from_range(90, 90, 110), 0);
        // tick_current == tick_lower - 1: out-of-range below
        assert_eq!(tick_distance_from_range(89, 90, 110), 1);
    }

    #[test]
    fn test_upper_boundary_not_full_weight() {
        // Position at tick_current == tick_upper should NOT get full rewards
        let mult = position_multiplier_bps(110, 90, 110, 30);
        assert!(mult < BPS_DENOMINATOR, "boundary position should not get full weight");
    }
}
