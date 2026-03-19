use crate::calculator::price_weight;
use crate::constants::{FEE_MULTIPLIER, PRECISION};
use crate::plane::ConcentratedPoolData;
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::Env;

// Fixed-point scaling for geometric step growth approximation.
const STEP_GROWTH_SCALE: u128 = 1_000_000_000_000;
// sqrt(1.0001) in STEP_GROWTH_SCALE precision.
const SQRT_TICK_BASE_FP: u128 = 1_000_049_998_750;

fn step_amounts(data: &ConcentratedPoolData, in_idx: u32, step: u32) -> (u128, u128) {
    if in_idx == 0 {
        data.step_0_to_1(step)
    } else {
        data.step_1_to_0(step)
    }
}

fn relative_price_weight(price: u128, reference_price: u128) -> u128 {
    if price == 0 || reference_price == 0 {
        return 0;
    }
    if price >= reference_price {
        price_weight(price, reference_price)
    } else {
        price_weight(reference_price, price)
    }
}

fn chunk_liquidity(e: &Env, fee_fraction: u128, amount_in: u128) -> u128 {
    if amount_in == 0 {
        return 0;
    }

    amount_in.fixed_mul_floor(e, &FEE_MULTIPLIER, &(56 * (FEE_MULTIPLIER - fee_fraction)))
}

fn fixed_pow_step_growth(e: &Env, mut base: u128, mut exp: u32) -> u128 {
    let mut result = STEP_GROWTH_SCALE;
    while exp > 0 {
        if exp & 1 == 1 {
            result = result.fixed_mul_floor(e, &base, &STEP_GROWTH_SCALE);
        }
        exp >>= 1;
        if exp > 0 {
            base = base.fixed_mul_floor(e, &base, &STEP_GROWTH_SCALE);
        }
    }
    result
}

fn step_virtual_input(e: &Env, amount_in: u128, tick_spacing: i32, step: u32) -> u128 {
    if amount_in == 0 || tick_spacing <= 0 {
        return amount_in;
    }

    // step k spans (k+1)*tick_spacing ticks because boundaries grow as
    // triangular numbers: 1, 3, 6, ... spacing-intervals from the current tick.
    let delta_ticks = (tick_spacing as u32).saturating_mul(step.saturating_add(1));
    if delta_ticks == 0 {
        return amount_in;
    }

    // Local virtual reserve for the monotonic piece:
    //   x_piece = dx / (sqrt(price_ratio) - 1),
    // where sqrt(price_ratio) = (sqrt(1.0001))^delta_ticks.
    let sqrt_growth = fixed_pow_step_growth(e, SQRT_TICK_BASE_FP, delta_ticks);
    if sqrt_growth <= STEP_GROWTH_SCALE {
        return amount_in;
    }

    let sqrt_growth_minus_one = sqrt_growth.saturating_sub(STEP_GROWTH_SCALE);
    amount_in.fixed_mul_floor(e, &STEP_GROWTH_SCALE, &sqrt_growth_minus_one)
}

fn near_window_liquidity(
    e: &Env,
    data: &ConcentratedPoolData,
    in_idx: u32,
    steps: u32,
) -> (u128, u128, u128) {
    let fee_fraction = data.fee;
    let mut near_liquidity = 0u128;
    let mut raw_in = 0u128;
    let mut reference_price = 0u128;
    let mut edge_weight = 0u128;

    for step in 0..steps {
        let (amount_in, amount_out) = step_amounts(data, in_idx, step);
        if amount_in == 0 || amount_out == 0 {
            // Plane snapshots may contain zero-sized near ticks because of integer
            // rounding, while farther monotonic chunks still have non-zero depth.
            // Skip zeros instead of stopping the whole piecewise accumulation.
            continue;
        }

        // Each monotonic step is treated as an independent local pool segment.
        // Convert step swap depth into local virtual reserve and apply
        // the closed-form branch formula per segment.
        let virtual_in = step_virtual_input(e, amount_in, data.tick_spacing, step);
        near_liquidity =
            near_liquidity.saturating_add(chunk_liquidity(e, fee_fraction, virtual_in));
        raw_in = raw_in.saturating_add(amount_in);

        let price = amount_in.fixed_mul_floor(e, &PRECISION, &amount_out);
        if price == 0 {
            continue;
        }

        if reference_price == 0 {
            reference_price = price;
        }

        edge_weight = relative_price_weight(price, reference_price);
    }

    (near_liquidity, raw_in, edge_weight)
}

pub fn get_liquidity(e: &Env, data: &ConcentratedPoolData, in_idx: u32, _out_idx: u32) -> u128 {
    let reserve_in = data.reserve(in_idx);
    if reserve_in == 0 {
        return 0;
    }

    let full_range_in = data.full_range_in(in_idx).min(reserve_in);

    let fee_fraction = data.fee;
    let exact_steps = if data.tick_spacing > 0 { data.steps } else { 0 };

    // Near the current price we sum exact monotonic step contributions from plane snapshot.
    let (near_liquidity, near_raw_in, edge_weight) =
        near_window_liquidity(e, data, in_idx, exact_steps);

    // Remaining range is represented as one far-tail segment discounted by edge distance.
    let remaining_in = reserve_in
        .saturating_sub(full_range_in)
        .saturating_sub(near_raw_in);
    let tail_in = if edge_weight == 0 {
        // If no exact near-window steps are available, fallback to a single
        // unweighted segment so concentrated snapshots without steps don't
        // collapse to zero liquidity.
        remaining_in
    } else {
        remaining_in.fixed_mul_floor(e, &edge_weight, &PRECISION)
    };
    let full_range_liquidity = chunk_liquidity(e, fee_fraction, full_range_in);
    let tail_liquidity = chunk_liquidity(e, fee_fraction, tail_in);

    full_range_liquidity
        .saturating_add(near_liquidity)
        .saturating_add(tail_liquidity)
}
