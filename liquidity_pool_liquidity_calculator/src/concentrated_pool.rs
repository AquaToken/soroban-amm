use crate::calculator::price_weight;
use crate::constants::{FEE_MULTIPLIER, PRECISION};
use crate::plane::ConcentratedPoolData;
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::Env;

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

fn weighted_near_window(
    e: &Env,
    data: &ConcentratedPoolData,
    in_idx: u32,
    steps: u32,
) -> (u128, u128, u128, u128) {
    let mut weighted_in = 0u128;
    let mut weighted_out = 0u128;
    let mut raw_in = 0u128;
    let mut reference_price = 0u128;
    let mut edge_weight = 0u128;

    for step in 0..steps {
        let (amount_in, amount_out) = step_amounts(data, in_idx, step);
        if amount_in == 0 || amount_out == 0 {
            break;
        }

        let price = amount_in.fixed_mul_floor(e, &PRECISION, &amount_out);
        if price == 0 {
            continue;
        }

        if reference_price == 0 {
            reference_price = price;
        }

        let weight = relative_price_weight(price, reference_price);
        weighted_in = weighted_in.saturating_add(amount_in.fixed_mul_floor(e, &weight, &PRECISION));
        weighted_out =
            weighted_out.saturating_add(amount_out.fixed_mul_floor(e, &weight, &PRECISION));
        raw_in = raw_in.saturating_add(amount_in);
        edge_weight = weight;
    }

    (weighted_in, weighted_out, raw_in, edge_weight)
}

pub fn get_liquidity(e: &Env, data: &ConcentratedPoolData, in_idx: u32, out_idx: u32) -> u128 {
    let reserve_in = data.reserve(in_idx);
    let reserve_out = data.reserve(out_idx);
    if reserve_in == 0 || reserve_out == 0 {
        return 0;
    }

    let full_range_in = data.full_range_in(in_idx).min(reserve_in);
    let full_range_out = data.full_range_out(out_idx).min(reserve_out);

    let fee_fraction = data.fee;
    let exact_steps = if data.tick_spacing > 0 { data.steps } else { 0 };

    // Near the current price we take exact full-tick depth from plane snapshot.
    // Step contributions are distance-weighted by relative price to the current region.
    let (near_weighted_in, near_weighted_out, near_raw_in, edge_weight) =
        weighted_near_window(e, data, in_idx, exact_steps);

    // Remaining range is approximated with one average factor.
    let remaining_in = reserve_in
        .saturating_sub(full_range_in)
        .saturating_sub(near_raw_in);
    let covered_out = near_weighted_out.saturating_add(full_range_out);
    let coverage = covered_out
        .fixed_mul_floor(e, &PRECISION, &reserve_out)
        .min(PRECISION);
    let base_tail_multiplier = (PRECISION.saturating_add(coverage)) / 2;
    let distance_tail_multiplier = if edge_weight == 0 {
        PRECISION / 2
    } else {
        edge_weight
    };
    let tail_multiplier =
        base_tail_multiplier.fixed_mul_floor(e, &distance_tail_multiplier, &PRECISION);
    let tail_in = remaining_in.fixed_mul_floor(e, &tail_multiplier, &PRECISION);

    // Full-range positions are scored separately using standard-pool math
    // and do not participate in the concentrated near/tail approximation.
    let effective_input = full_range_in
        .saturating_add(near_weighted_in)
        .saturating_add(tail_in);
    if effective_input == 0 {
        return 0;
    }

    effective_input.fixed_mul_floor(e, &FEE_MULTIPLIER, &(56 * (FEE_MULTIPLIER - fee_fraction)))
}
