pub mod pool_plane {
    soroban_sdk::contractimport!(file = "../contracts/soroban_liquidity_pool_plane_contract.wasm");
}

pub use crate::plane::pool_plane::Client as PoolPlaneClient;

use crate::math::{amount0_delta, amount1_delta, sqrt_ratio_at_tick};
use crate::storage::{
    get_fee, get_liquidity, get_plane, get_protocol_fees, get_slot0, get_tick, get_tick_spacing,
    get_token0, get_token1, MAX_TICK, MIN_TICK,
};
use soroban_sdk::token::TokenClient as SorobanTokenClient;
use soroban_sdk::{Env, Symbol, Vec, U256};

const PLANE_DATA_VERSION: u128 = 1;
const EXACT_TICK_STEPS: u32 = 20;

fn compress_tick(tick: i32, spacing: i32) -> i32 {
    let mut compressed = tick / spacing;
    if tick < 0 && tick % spacing != 0 {
        compressed -= 1;
    }
    compressed
}

fn apply_liquidity_net(liquidity: u128, liquidity_net: i128, zero_for_one: bool) -> u128 {
    if zero_for_one {
        if liquidity_net >= 0 {
            liquidity.saturating_sub(liquidity_net as u128)
        } else {
            liquidity.saturating_add((-liquidity_net) as u128)
        }
    } else if liquidity_net >= 0 {
        liquidity.saturating_add(liquidity_net as u128)
    } else {
        liquidity.saturating_sub((-liquidity_net) as u128)
    }
}

fn next_boundary_tick(compressed_current: i32, spacing: i32, step: u32, zero_for_one: bool) -> i32 {
    if zero_for_one {
        compressed_current
            .saturating_sub(step as i32)
            .saturating_mul(spacing)
            .max(MIN_TICK)
    } else {
        compressed_current
            .saturating_add(step as i32)
            .saturating_add(1)
            .saturating_mul(spacing)
            .min(MAX_TICK)
    }
}

fn push_empty_steps(out: &mut Vec<u128>, steps: u32) {
    for _ in 0..steps {
        out.push_back(0);
        out.push_back(0);
    }
}

fn collect_exact_direction_steps(
    e: &Env,
    zero_for_one: bool,
    steps: u32,
    spacing: i32,
) -> Vec<u128> {
    let mut result = Vec::new(e);
    if spacing <= 0 {
        push_empty_steps(&mut result, steps);
        return result;
    }

    let slot = get_slot0(e);
    let zero = U256::from_u32(e, 0);
    if slot.sqrt_price_x96 == zero {
        push_empty_steps(&mut result, steps);
        return result;
    }

    let compressed_current = compress_tick(slot.tick, spacing);
    let mut sqrt_current = slot.sqrt_price_x96;
    let mut liquidity = get_liquidity(e);
    let mut exhausted = false;

    for step in 0..steps {
        if exhausted {
            result.push_back(0);
            result.push_back(0);
            continue;
        }

        let boundary_tick = next_boundary_tick(compressed_current, spacing, step, zero_for_one);
        let sqrt_target = match sqrt_ratio_at_tick(e, boundary_tick) {
            Ok(value) => value,
            Err(_) => {
                result.push_back(0);
                result.push_back(0);
                exhausted = true;
                continue;
            }
        };

        if sqrt_target == sqrt_current || liquidity == 0 {
            result.push_back(0);
            result.push_back(0);
            exhausted = true;
            continue;
        }

        let (amount_in, amount_out) = if zero_for_one {
            (
                amount0_delta(e, &sqrt_target, &sqrt_current, liquidity, true),
                amount1_delta(e, &sqrt_target, &sqrt_current, liquidity, false),
            )
        } else {
            (
                amount1_delta(e, &sqrt_current, &sqrt_target, liquidity, true),
                amount0_delta(e, &sqrt_current, &sqrt_target, liquidity, false),
            )
        };

        let in_value = amount_in.unwrap_or(0);
        let out_value = amount_out.unwrap_or(0);
        result.push_back(in_value);
        result.push_back(out_value);

        sqrt_current = sqrt_target;
        let liquidity_net = get_tick(e, boundary_tick).liquidity_net;
        liquidity = apply_liquidity_net(liquidity, liquidity_net, zero_for_one);

        if boundary_tick <= MIN_TICK || boundary_tick >= MAX_TICK {
            exhausted = true;
        }
    }

    result
}

fn get_pool_data(e: &Env) -> (Vec<u128>, Vec<u128>) {
    let contract = e.current_contract_address();
    let fees = get_protocol_fees(e);

    let balance0 = SorobanTokenClient::new(e, &get_token0(e)).balance(&contract) as u128;
    let balance1 = SorobanTokenClient::new(e, &get_token1(e)).balance(&contract) as u128;

    let reserve0 = balance0.saturating_sub(fees.token0);
    let reserve1 = balance1.saturating_sub(fees.token1);
    let spacing = get_tick_spacing(e);
    let spacing_u128 = if spacing > 0 { spacing as u128 } else { 0 };

    let mut reserves = Vec::from_array(e, [reserve0, reserve1]);
    let steps_0_to_1 = collect_exact_direction_steps(e, true, EXACT_TICK_STEPS, spacing);
    for value in steps_0_to_1.iter() {
        reserves.push_back(value);
    }

    let steps_1_to_0 = collect_exact_direction_steps(e, false, EXACT_TICK_STEPS, spacing);
    for value in steps_1_to_0.iter() {
        reserves.push_back(value);
    }

    (
        Vec::from_array(
            e,
            [
                PLANE_DATA_VERSION,
                get_fee(e) as u128,
                spacing_u128,
                EXACT_TICK_STEPS as u128,
            ],
        ),
        reserves,
    )
}

pub fn update_plane(e: &Env) {
    let (init_args, reserves) = get_pool_data(e);
    PoolPlaneClient::new(e, &get_plane(e)).update(
        &e.current_contract_address(),
        &Symbol::new(e, "concentrated"),
        &init_args,
        &reserves,
    );
}
