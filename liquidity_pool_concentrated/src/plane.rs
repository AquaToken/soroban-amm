pub mod pool_plane {
    soroban_sdk::contractimport!(file = "../contracts/soroban_liquidity_pool_plane_contract.wasm");
}

pub use crate::plane::pool_plane::Client as PoolPlaneClient;

use crate::math::{amount0_delta, amount1_delta, sqrt_ratio_at_tick};
use crate::storage::{
    get_fee, get_liquidity, get_plane, get_protocol_fees, get_slot0, get_tick,
    get_tick_bitmap_word, get_tick_spacing, get_token0, get_token1, MAX_TICK, MIN_TICK,
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

fn push_empty_steps(out: &mut Vec<u128>, steps: u32) {
    for _ in 0..steps {
        out.push_back(0);
        out.push_back(0);
    }
}

// ── Bitmap helpers (duplicated from contract/internal.rs for plane use) ──

fn u256_to_array(v: &U256) -> [u8; 32] {
    let bytes = v.to_be_bytes();
    let mut out = [0u8; 32];
    bytes.copy_into_slice(&mut out);
    out
}

fn position(compressed_tick: i32) -> (i32, u32) {
    let word_pos = compressed_tick >> 8;
    let bit_pos = (compressed_tick & 255) as u32;
    (word_pos, bit_pos)
}

fn find_prev_set_bit(word: &[u8; 32], from_bit: u32) -> Option<u32> {
    let from_bit = from_bit.min(255);
    let start_byte = (255 - from_bit) / 8;
    let start_bit_in_byte = from_bit % 8;

    let mask = ((1u16 << (start_bit_in_byte + 1)) - 1) as u8;
    let masked = word[start_byte as usize] & mask;
    if masked != 0 {
        let top_bit = 7 - masked.leading_zeros();
        return Some((31 - start_byte) * 8 + top_bit);
    }

    for byte_idx in (start_byte + 1)..32 {
        if word[byte_idx as usize] != 0 {
            let top_bit = 7 - word[byte_idx as usize].leading_zeros();
            return Some((31 - byte_idx) * 8 + top_bit);
        }
    }

    None
}

fn find_next_set_bit(word: &[u8; 32], from_bit: u32) -> Option<u32> {
    let from_bit = from_bit.min(255);
    let start_byte = (255 - from_bit) / 8;
    let start_bit_in_byte = from_bit % 8;

    let mask = !((1u8 << start_bit_in_byte).wrapping_sub(1));
    let masked = word[start_byte as usize] & mask;
    if masked != 0 {
        let low_bit = masked.trailing_zeros();
        return Some((31 - start_byte) * 8 + low_bit);
    }

    if start_byte > 0 {
        for byte_idx in (0..start_byte).rev() {
            if word[byte_idx as usize] != 0 {
                let low_bit = word[byte_idx as usize].trailing_zeros();
                return Some((31 - byte_idx) * 8 + low_bit);
            }
        }
    }

    None
}

/// Find the next initialized tick using cross-word bitmap scanning.
/// For zero_for_one (lte): scans from `compressed` downward toward `limit_compressed`.
/// For one_for_zero (!lte): scans from `compressed + 1` upward toward `limit_compressed`.
/// Returns (initialized_tick, found) or None if no initialized tick within range.
fn find_initialized_tick(
    e: &Env,
    compressed: i32,
    limit_compressed: i32,
    spacing: i32,
    lte: bool,
) -> Option<i32> {
    if lte {
        // Scan downward: from `compressed` toward `limit_compressed`
        let (mut word_pos, bit_pos) = position(compressed);
        let (limit_word_pos, _) = position(limit_compressed);

        // First (partial) word
        let word = u256_to_array(&get_tick_bitmap_word(e, word_pos));
        if let Some(msb) = find_prev_set_bit(&word, bit_pos) {
            let found_compressed = (word_pos << 8) + msb as i32;
            if found_compressed >= limit_compressed {
                let tick = found_compressed.saturating_mul(spacing);
                return Some(tick.max(MIN_TICK));
            }
        }

        // Subsequent full words
        word_pos -= 1;
        while word_pos >= limit_word_pos {
            let word = u256_to_array(&get_tick_bitmap_word(e, word_pos));
            if let Some(msb) = find_prev_set_bit(&word, 255) {
                let found_compressed = (word_pos << 8) + msb as i32;
                if found_compressed >= limit_compressed {
                    let tick = found_compressed.saturating_mul(spacing);
                    return Some(tick.max(MIN_TICK));
                }
                return None;
            }
            word_pos -= 1;
        }

        None
    } else {
        // Scan upward: from `compressed + 1` toward `limit_compressed`
        let start_compressed = compressed.saturating_add(1);
        let (mut word_pos, bit_pos) = position(start_compressed);
        let (limit_word_pos, _) = position(limit_compressed);

        // First (partial) word
        let word = u256_to_array(&get_tick_bitmap_word(e, word_pos));
        if let Some(lsb) = find_next_set_bit(&word, bit_pos) {
            let found_compressed = (word_pos << 8) + lsb as i32;
            if found_compressed <= limit_compressed {
                let tick = found_compressed.saturating_mul(spacing);
                return Some(tick.min(MAX_TICK));
            }
        }

        // Subsequent full words
        word_pos += 1;
        while word_pos <= limit_word_pos {
            let word = u256_to_array(&get_tick_bitmap_word(e, word_pos));
            if let Some(lsb) = find_next_set_bit(&word, 0) {
                let found_compressed = (word_pos << 8) + lsb as i32;
                if found_compressed <= limit_compressed {
                    let tick = found_compressed.saturating_mul(spacing);
                    return Some(tick.min(MAX_TICK));
                }
                return None;
            }
            word_pos += 1;
        }

        None
    }
}

/// Cumulative boundary count at step `i`: triangular number (i+1)*(i+2)/2.
/// Step 0: 1, Step 1: 3, Step 2: 6, ..., Step 19: 210.
fn step_target(step: u32) -> i32 {
    (((step + 1) * (step + 2)) / 2) as i32
}

/// Compute amounts between two sqrt prices at given liquidity.
/// Returns (amount_in, amount_out).
fn compute_amounts(
    e: &Env,
    sqrt_a: &U256,
    sqrt_b: &U256,
    liquidity: u128,
    zero_for_one: bool,
) -> (u128, u128) {
    if zero_for_one {
        // Selling token0 for token1: price goes down (sqrt_a > sqrt_b)
        (
            amount0_delta(e, sqrt_b, sqrt_a, liquidity, true).unwrap_or(0),
            amount1_delta(e, sqrt_b, sqrt_a, liquidity, false).unwrap_or(0),
        )
    } else {
        // Selling token1 for token0: price goes up (sqrt_a < sqrt_b)
        (
            amount1_delta(e, sqrt_a, sqrt_b, liquidity, true).unwrap_or(0),
            amount0_delta(e, sqrt_a, sqrt_b, liquidity, false).unwrap_or(0),
        )
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
    let mut sqrt_cursor = slot.sqrt_price_x96;
    let mut liquidity = get_liquidity(e);
    let mut exhausted = false;

    // Track the compressed tick of the cursor for bitmap scanning.
    // For zero_for_one, cursor starts at compressed_current and moves down.
    // For !zero_for_one, cursor starts at compressed_current and moves up.
    let mut cursor_compressed = compressed_current;

    for step in 0..steps {
        if exhausted {
            result.push_back(0);
            result.push_back(0);
            continue;
        }

        // Target tick for this step: current ± step_target(step) * spacing
        let target_boundary_count = step_target(step);
        let target_tick = if zero_for_one {
            (compressed_current.saturating_sub(target_boundary_count))
                .saturating_mul(spacing)
                .max(MIN_TICK)
        } else {
            (compressed_current
                .saturating_add(target_boundary_count)
                .saturating_add(1))
            .saturating_mul(spacing)
            .min(MAX_TICK)
        };

        let sqrt_step_target = match sqrt_ratio_at_tick(e, target_tick) {
            Ok(value) => value,
            Err(_) => {
                result.push_back(0);
                result.push_back(0);
                exhausted = true;
                continue;
            }
        };

        if sqrt_step_target == sqrt_cursor || liquidity == 0 {
            result.push_back(0);
            result.push_back(0);
            exhausted = true;
            continue;
        }

        let mut step_in: u128 = 0;
        let mut step_out: u128 = 0;

        // Target compressed tick for this step
        let target_compressed = if zero_for_one {
            compressed_current.saturating_sub(target_boundary_count)
        } else {
            compressed_current
                .saturating_add(target_boundary_count)
                .saturating_add(1)
        };

        // Walk through initialized ticks from cursor to target using bitmap scanning
        loop {
            let limit_compressed = target_compressed;

            let maybe_init_tick = find_initialized_tick(
                e,
                cursor_compressed,
                limit_compressed,
                spacing,
                zero_for_one,
            );

            let init_tick_in_range = match maybe_init_tick {
                Some(tick) => {
                    if zero_for_one {
                        tick >= target_tick
                    } else {
                        tick <= target_tick
                    }
                }
                None => false,
            };

            if init_tick_in_range {
                let init_tick = maybe_init_tick.unwrap();
                let sqrt_init = match sqrt_ratio_at_tick(e, init_tick) {
                    Ok(v) => v,
                    Err(_) => break,
                };

                // Compute amounts from cursor to initialized tick
                let (amt_in, amt_out) =
                    compute_amounts(e, &sqrt_cursor, &sqrt_init, liquidity, zero_for_one);
                step_in = step_in.saturating_add(amt_in);
                step_out = step_out.saturating_add(amt_out);

                // Cross the tick: apply liquidity delta
                let tick_info = get_tick(e, init_tick);
                liquidity = apply_liquidity_net(liquidity, tick_info.liquidity_net, zero_for_one);
                sqrt_cursor = sqrt_init;
                cursor_compressed = compress_tick(init_tick, spacing);

                // After crossing, move cursor past this tick for next bitmap scan
                if zero_for_one {
                    cursor_compressed -= 1;
                }
                // For !zero_for_one, find_initialized_tick searches from compressed+1,
                // so no adjustment needed.
            } else {
                // No more initialized ticks before target; compute remaining segment
                break;
            }
        }

        // Compute amounts from cursor to the step target (constant liquidity)
        if sqrt_cursor != sqrt_step_target && liquidity > 0 {
            let (amt_in, amt_out) =
                compute_amounts(e, &sqrt_cursor, &sqrt_step_target, liquidity, zero_for_one);
            step_in = step_in.saturating_add(amt_in);
            step_out = step_out.saturating_add(amt_out);
        }

        result.push_back(step_in);
        result.push_back(step_out);

        // Advance cursor to the step target boundary for the next step
        sqrt_cursor = sqrt_step_target;
        cursor_compressed = target_compressed;
        if zero_for_one {
            // cursor_compressed already points to the boundary we just reached
        } else {
            // For 1→0, cursor_compressed should be target_compressed - 1 because
            // we are sitting AT target_compressed * spacing, which is the lower bound
            // of the tick range at target_compressed.
            cursor_compressed -= 1;
        }

        if target_tick <= MIN_TICK || target_tick >= MAX_TICK {
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
