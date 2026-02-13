use soroban_sdk::contracttype;

pub const BPS_DENOMINATOR: u32 = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub struct DistanceWeightConfig {
    // Distance in ticks from nearest range edge where multiplier reaches min_multiplier_bps.
    pub max_distance_ticks: u32,
    // Lower reward floor for far out-of-range positions (0..=10_000).
    pub min_multiplier_bps: u32,
}

pub fn tick_distance_from_range(tick_current: i32, tick_lower: i32, tick_upper: i32) -> u32 {
    if tick_lower >= tick_upper {
        return 0;
    }

    if tick_current < tick_lower {
        (tick_lower as i64 - tick_current as i64) as u32
    } else if tick_current > tick_upper {
        (tick_current as i64 - tick_upper as i64) as u32
    } else {
        0
    }
}

pub fn distance_multiplier_bps(distance_ticks: u32, cfg: DistanceWeightConfig) -> u32 {
    let min_multiplier = cfg.min_multiplier_bps.min(BPS_DENOMINATOR);
    if distance_ticks == 0 {
        return BPS_DENOMINATOR;
    }

    if cfg.max_distance_ticks == 0 || distance_ticks >= cfg.max_distance_ticks {
        return min_multiplier;
    }

    // Quadratic decay from 1.0 at distance=0 to min_multiplier at max_distance_ticks.
    let max_dist = cfg.max_distance_ticks as u128;
    let rem = (cfg.max_distance_ticks - distance_ticks) as u128;
    let dynamic_range = (BPS_DENOMINATOR - min_multiplier) as u128;

    let scaled = dynamic_range * rem * rem / (max_dist * max_dist);
    min_multiplier + scaled as u32
}

pub fn position_multiplier_bps(
    tick_current: i32,
    tick_lower: i32,
    tick_upper: i32,
    cfg: DistanceWeightConfig,
) -> u32 {
    let distance = tick_distance_from_range(tick_current, tick_lower, tick_upper);
    distance_multiplier_bps(distance, cfg)
}

pub fn apply_multiplier(amount: u128, multiplier_bps: u32) -> u128 {
    let multiplier = multiplier_bps.min(BPS_DENOMINATOR) as u128;
    amount.saturating_mul(multiplier) / BPS_DENOMINATOR as u128
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_range_has_full_weight() {
        let cfg = DistanceWeightConfig {
            max_distance_ticks: 1_000,
            min_multiplier_bps: 0,
        };
        assert_eq!(position_multiplier_bps(100, 90, 110, cfg), 10_000);
    }

    #[test]
    fn test_out_of_range_decays_quadratically() {
        let cfg = DistanceWeightConfig {
            max_distance_ticks: 100,
            min_multiplier_bps: 0,
        };

        assert_eq!(distance_multiplier_bps(50, cfg), 2_500);
        assert_eq!(distance_multiplier_bps(100, cfg), 0);
        assert_eq!(distance_multiplier_bps(120, cfg), 0);
    }

    #[test]
    fn test_non_zero_min_multiplier() {
        let cfg = DistanceWeightConfig {
            max_distance_ticks: 100,
            min_multiplier_bps: 500,
        };

        assert_eq!(distance_multiplier_bps(100, cfg), 500);
        assert_eq!(distance_multiplier_bps(120, cfg), 500);
    }

    #[test]
    fn test_apply_multiplier() {
        assert_eq!(apply_multiplier(1_000, 10_000), 1_000);
        assert_eq!(apply_multiplier(1_000, 2_500), 250);
        assert_eq!(apply_multiplier(1_000, 20_000), 1_000);
    }
}
