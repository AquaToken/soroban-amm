pub mod pool_plane {
    soroban_sdk::contractimport!(file = "../contracts/soroban_liquidity_pool_plane_contract.wasm");
}

pub use crate::plane::pool_plane::Client as PoolPlaneClient;

use crate::bitmap::{
    chunk_bitmap_position, compress_tick, compressed_to_tick, find_next_set_bit, find_prev_set_bit,
    u256_to_array, word_bitmap_position,
};
use crate::constants::{MAX_TICK, MIN_TICK, TICKS_PER_CHUNK};
use crate::math::{amount0_delta, amount1_delta, sqrt_ratio_at_tick};
use crate::storage::{
    chunk_address, get_chunk_bitmap_word, get_fee, get_full_range_liquidity, get_liquidity,
    get_plane, get_reserve0, get_reserve1, get_slot0, get_tick_spacing, get_word_bitmap,
    ChunkCache,
};
use soroban_sdk::{Env, Symbol, Vec, U256};

const PLANE_DATA_VERSION: u128 = 1;
const MIN_EXACT_TICK_STEPS: u32 = 8;
const MAX_EXACT_TICK_STEPS: u32 = 20;
// Weight function is (p_ref/p)^8. Around 1% tail weight is reached near ~5.7k ticks.
const TARGET_PRICE_DISTANCE_TICKS: u32 = 5_700;

fn exact_tick_steps_for_spacing(spacing: i32) -> u32 {
    if spacing <= 0 {
        return 0;
    }

    let spacing_u32 = spacing as u32;
    let mut steps = MIN_EXACT_TICK_STEPS;
    while steps < MAX_EXACT_TICK_STEPS {
        // For steps = N, far boundary is N*(N+1)/2 spacing-intervals away.
        let boundary_count = steps.saturating_mul(steps.saturating_add(1)) / 2;
        let covered_ticks = boundary_count.saturating_mul(spacing_u32);
        if covered_ticks >= TARGET_PRICE_DISTANCE_TICKS {
            break;
        }
        steps += 1;
    }
    steps
}

fn full_range_ticks_for_spacing(spacing: i32) -> Option<(i32, i32)> {
    if spacing <= 0 {
        return None;
    }

    let mut tick_lower = MIN_TICK - (MIN_TICK % spacing);
    if tick_lower < MIN_TICK {
        tick_lower = tick_lower.saturating_add(spacing);
    }

    let mut tick_upper = MAX_TICK - (MAX_TICK % spacing);
    if tick_upper > MAX_TICK {
        tick_upper = tick_upper.saturating_sub(spacing);
    }

    if tick_lower >= tick_upper {
        return None;
    }

    Some((tick_lower, tick_upper))
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

fn find_adjacent_chunk_bitmap_word(e: &Env, current_word_pos: i32, lte: bool) -> Option<i32> {
    let (l2_pos, l2_bit) = word_bitmap_position(current_word_pos);

    let search = |l2_pos: i32, from: u32| -> Option<i32> {
        let l2_word = u256_to_array(&get_word_bitmap(e, l2_pos));
        let found = if lte {
            find_prev_set_bit(&l2_word, from)
        } else {
            find_next_set_bit(&l2_word, from)
        };
        found.map(|bit| (l2_pos << 8) + bit as i32)
    };

    // Try current L2 word (skip own bit)
    let adjacent = if lte && l2_bit > 0 {
        search(l2_pos, l2_bit - 1)
    } else if !lte && l2_bit < 255 {
        search(l2_pos, l2_bit + 1)
    } else {
        None
    };
    if adjacent.is_some() {
        return adjacent;
    }

    // Try adjacent L2 word
    let l2_adj = if lte { l2_pos - 1 } else { l2_pos + 1 };
    let from = if lte { 255 } else { 0 };
    search(l2_adj, from)
}

// Three-level chunk-based tick search.
// For lte (zero_for_one): scans from `compressed` downward toward `limit_compressed`.
// For !lte (one_for_zero): scans from `compressed + 1` upward toward `limit_compressed`.
// Returns (tick, liquidity_net) of the first initialized tick found, or None.
// Since the chunk is already loaded during the search, liquidity_net is returned directly
// to avoid a redundant storage read.
fn find_initialized_tick(
    e: &Env,
    compressed: i32,
    limit_compressed: i32,
    spacing: i32,
    lte: bool,
    cc: &mut ChunkCache,
) -> Option<(i32, i128)> {
    if lte {
        // --- Scanning downward ---
        let (chunk_pos, slot) = chunk_address(compressed);

        // 1. Check current chunk: scan slots [0..=slot] downward
        if let Some(chunk) = cc.get_chunk(e, chunk_pos) {
            for s in (0..=slot).rev() {
                let td = chunk.get(s).unwrap();
                if td.2 > 0 {
                    let found_compressed = chunk_pos * TICKS_PER_CHUNK + s as i32;
                    if found_compressed >= limit_compressed {
                        return Some((compressed_to_tick(found_compressed, spacing), td.3));
                    }
                }
            }
        }

        // 2. Use chunk bitmap to find previous chunk with initialized ticks
        let (bm_word_pos, bm_bit_pos) = chunk_bitmap_position(chunk_pos);
        let word = u256_to_array(&get_chunk_bitmap_word(e, bm_word_pos));

        if bm_bit_pos > 0 {
            if let Some(found_bit) = find_prev_set_bit(&word, bm_bit_pos - 1) {
                let found_chunk_pos = (bm_word_pos << 8) + found_bit as i32;
                if let Some(chunk) = cc.get_chunk(e, found_chunk_pos) {
                    for s in (0..TICKS_PER_CHUNK as u32).rev() {
                        let td = chunk.get(s).unwrap();
                        if td.2 > 0 {
                            let found_compressed = found_chunk_pos * TICKS_PER_CHUNK + s as i32;
                            if found_compressed >= limit_compressed {
                                return Some((compressed_to_tick(found_compressed, spacing), td.3));
                            }
                        }
                    }
                }
            }
        }

        // 3. Use L2 word bitmap to skip to the adjacent non-empty L1 word
        if let Some(target_word) = find_adjacent_chunk_bitmap_word(e, bm_word_pos, true) {
            let target_word_bits = u256_to_array(&get_chunk_bitmap_word(e, target_word));
            if let Some(found_bit) = find_prev_set_bit(&target_word_bits, 255) {
                let found_chunk_pos = (target_word << 8) + found_bit as i32;
                if let Some(chunk) = cc.get_chunk(e, found_chunk_pos) {
                    for s in (0..TICKS_PER_CHUNK as u32).rev() {
                        let td = chunk.get(s).unwrap();
                        if td.2 > 0 {
                            let found_compressed = found_chunk_pos * TICKS_PER_CHUNK + s as i32;
                            if found_compressed >= limit_compressed {
                                return Some((compressed_to_tick(found_compressed, spacing), td.3));
                            } else {
                                return None;
                            }
                        }
                    }
                }
            }
        }

        None
    } else {
        // --- Scanning upward ---
        let start_compressed = compressed.saturating_add(1);
        let (chunk_pos, slot) = chunk_address(start_compressed);

        // 1. Check current chunk: scan slots [slot..TICKS_PER_CHUNK) upward
        if let Some(chunk) = cc.get_chunk(e, chunk_pos) {
            for s in slot..TICKS_PER_CHUNK as u32 {
                let td = chunk.get(s).unwrap();
                if td.2 > 0 {
                    let found_compressed = chunk_pos * TICKS_PER_CHUNK + s as i32;
                    if found_compressed <= limit_compressed {
                        return Some((compressed_to_tick(found_compressed, spacing), td.3));
                    }
                }
            }
        }

        // 2. Use chunk bitmap to find next chunk with initialized ticks
        let (bm_word_pos, bm_bit_pos) = chunk_bitmap_position(chunk_pos);
        let word = u256_to_array(&get_chunk_bitmap_word(e, bm_word_pos));

        if bm_bit_pos < 255 {
            if let Some(found_bit) = find_next_set_bit(&word, bm_bit_pos + 1) {
                let found_chunk_pos = (bm_word_pos << 8) + found_bit as i32;
                if let Some(chunk) = cc.get_chunk(e, found_chunk_pos) {
                    for s in 0..TICKS_PER_CHUNK as u32 {
                        let td = chunk.get(s).unwrap();
                        if td.2 > 0 {
                            let found_compressed = found_chunk_pos * TICKS_PER_CHUNK + s as i32;
                            if found_compressed <= limit_compressed {
                                return Some((compressed_to_tick(found_compressed, spacing), td.3));
                            }
                        }
                    }
                }
            }
        }

        // 3. Use L2 word bitmap to skip to the adjacent non-empty L1 word
        if let Some(target_word) = find_adjacent_chunk_bitmap_word(e, bm_word_pos, false) {
            let target_word_bits = u256_to_array(&get_chunk_bitmap_word(e, target_word));
            if let Some(found_bit) = find_next_set_bit(&target_word_bits, 0) {
                let found_chunk_pos = (target_word << 8) + found_bit as i32;
                if let Some(chunk) = cc.get_chunk(e, found_chunk_pos) {
                    for s in 0..TICKS_PER_CHUNK as u32 {
                        let td = chunk.get(s).unwrap();
                        if td.2 > 0 {
                            let found_compressed = found_chunk_pos * TICKS_PER_CHUNK + s as i32;
                            if found_compressed <= limit_compressed {
                                return Some((compressed_to_tick(found_compressed, spacing), td.3));
                            } else {
                                return None;
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

// Cumulative boundary count at step `i`: triangular number (i+1)*(i+2)/2.
// Step 0: 1, Step 1: 3, Step 2: 6, ..., Step 19: 210.
fn step_target(step: u32) -> i32 {
    (((step + 1) * (step + 2)) / 2) as i32
}

// Compute amounts between two sqrt prices at given liquidity.
// Returns (amount_in, amount_out).
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
            amount0_delta(e, sqrt_b, sqrt_a, liquidity, true),
            amount1_delta(e, sqrt_b, sqrt_a, liquidity, false),
        )
    } else {
        // Selling token1 for token0: price goes up (sqrt_a < sqrt_b)
        (
            amount1_delta(e, sqrt_a, sqrt_b, liquidity, true),
            amount0_delta(e, sqrt_a, sqrt_b, liquidity, false),
        )
    }
}

fn compute_full_range_reserves(e: &Env, spacing: i32, full_range_liquidity: u128) -> (u128, u128) {
    if full_range_liquidity == 0 {
        return (0, 0);
    }

    let (tick_lower, tick_upper) = match full_range_ticks_for_spacing(spacing) {
        Some(ticks) => ticks,
        None => return (0, 0),
    };

    let sqrt_lower = sqrt_ratio_at_tick(e, tick_lower);
    let sqrt_upper = sqrt_ratio_at_tick(e, tick_upper);

    let slot = get_slot0(e);
    if slot.sqrt_price_x96 <= sqrt_lower {
        (
            amount0_delta(e, &sqrt_lower, &sqrt_upper, full_range_liquidity, false),
            0,
        )
    } else if slot.sqrt_price_x96 < sqrt_upper {
        (
            amount0_delta(
                e,
                &slot.sqrt_price_x96,
                &sqrt_upper,
                full_range_liquidity,
                false,
            ),
            amount1_delta(
                e,
                &sqrt_lower,
                &slot.sqrt_price_x96,
                full_range_liquidity,
                false,
            ),
        )
    } else {
        (
            0,
            amount1_delta(e, &sqrt_lower, &sqrt_upper, full_range_liquidity, false),
        )
    }
}

// Compute the liquidity_net adjustment needed to exclude full-range position
// contributions at a given tick. Full-range positions add +L at lower tick and
// -L at upper tick (encoded as liquidity_net += L for lower, liquidity_net -= L
// for upper via the is_upper convention). To exclude their contribution we must
// subtract the same delta they added.
fn full_range_liquidity_net_adjustment(
    tick: i32,
    spacing: i32,
    full_range_lower: i32,
    full_range_upper: i32,
    full_range_liquidity: u128,
) -> i128 {
    if full_range_liquidity == 0 || spacing <= 0 {
        return 0;
    }
    let compressed = compress_tick(tick, spacing);
    let fr_lower_compressed = compress_tick(full_range_lower, spacing);
    let fr_upper_compressed = compress_tick(full_range_upper, spacing);
    let fl = full_range_liquidity as i128;

    if compressed == fr_lower_compressed {
        // Full-range deposit added +L to liquidity_net here → subtract it
        -fl
    } else if compressed == fr_upper_compressed {
        // Full-range deposit subtracted L from liquidity_net here → add it back
        fl
    } else {
        0
    }
}

fn collect_exact_direction_steps(
    e: &Env,
    zero_for_one: bool,
    steps: u32,
    spacing: i32,
    base_liquidity: u128,
    full_range_liquidity: u128,
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
    let mut liquidity = base_liquidity;
    let mut exhausted = false;
    let mut cc = ChunkCache::new(e);

    // Canonical full-range tick boundaries for adjusting liquidity_net on crossing.
    let (fr_lower, fr_upper) = full_range_ticks_for_spacing(spacing).unwrap_or((0, 0));

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

        let sqrt_step_target = sqrt_ratio_at_tick(e, target_tick);

        if sqrt_step_target == sqrt_cursor {
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

            let maybe_init = find_initialized_tick(
                e,
                cursor_compressed,
                limit_compressed,
                spacing,
                zero_for_one,
                &mut cc,
            );

            let init_in_range = match maybe_init {
                Some((tick, _)) => {
                    if zero_for_one {
                        tick >= target_tick
                    } else {
                        tick <= target_tick
                    }
                }
                None => false,
            };

            if init_in_range {
                let (init_tick, raw_liquidity_net) = maybe_init.unwrap();
                let sqrt_init = sqrt_ratio_at_tick(e, init_tick);

                // Compute amounts from cursor to initialized tick
                let (amt_in, amt_out) =
                    compute_amounts(e, &sqrt_cursor, &sqrt_init, liquidity, zero_for_one);
                step_in = step_in.saturating_add(amt_in);
                step_out = step_out.saturating_add(amt_out);

                // Exclude full-range position contribution from liquidity_net
                // so the step collector tracks only non-full-range liquidity.
                let adj = full_range_liquidity_net_adjustment(
                    init_tick,
                    spacing,
                    fr_lower,
                    fr_upper,
                    full_range_liquidity,
                );
                let liquidity_net = raw_liquidity_net.saturating_add(adj);

                // Cross the tick: apply adjusted liquidity delta.
                // Must happen BEFORE the saturation early-exit so that subsequent
                // steps use the correct liquidity value.
                liquidity = apply_liquidity_net(liquidity, liquidity_net, zero_for_one);
                sqrt_cursor = sqrt_init;
                cursor_compressed = compress_tick(init_tick, spacing);

                // After crossing, move cursor past this tick for next bitmap scan
                if zero_for_one {
                    cursor_compressed -= 1;
                }

                // If amounts saturated, further tick-walking won't improve the
                // estimate — break early to save gas.
                if step_in == u128::MAX || step_out == u128::MAX {
                    break;
                }
                // For !zero_for_one, find_initialized_tick searches from compressed+1,
                // so no adjustment needed.
            } else {
                // No more initialized ticks before target; compute remaining segment
                break;
            }
        }

        // Compute amounts from cursor to the step target (constant liquidity).
        // Skip if amounts already saturated (further computation won't help).
        if sqrt_cursor != sqrt_step_target
            && liquidity > 0
            && step_in != u128::MAX
            && step_out != u128::MAX
        {
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
    let reserve0 = get_reserve0(e);
    let reserve1 = get_reserve1(e);
    let spacing = get_tick_spacing(e);
    let exact_steps = exact_tick_steps_for_spacing(spacing);
    let full_range_liquidity = get_full_range_liquidity(e);
    let active_liquidity = get_liquidity(e);
    let non_full_range_active_liquidity = active_liquidity.saturating_sub(full_range_liquidity);
    let (full_range_reserve0, full_range_reserve1) =
        compute_full_range_reserves(e, spacing, full_range_liquidity);
    let spacing_u128 = if spacing > 0 { spacing as u128 } else { 0 };

    let mut reserves = Vec::from_array(
        e,
        [reserve0, reserve1, full_range_reserve0, full_range_reserve1],
    );
    let steps_0_to_1 = collect_exact_direction_steps(
        e,
        true,
        exact_steps,
        spacing,
        non_full_range_active_liquidity,
        full_range_liquidity,
    );
    for value in steps_0_to_1.iter() {
        reserves.push_back(value);
    }

    let steps_1_to_0 = collect_exact_direction_steps(
        e,
        false,
        exact_steps,
        spacing,
        non_full_range_active_liquidity,
        full_range_liquidity,
    );
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
                exact_steps as u128,
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

#[cfg(test)]
mod tests {
    use super::{
        exact_tick_steps_for_spacing, full_range_liquidity_net_adjustment,
        full_range_ticks_for_spacing,
    };

    #[test]
    fn test_exact_tick_steps_for_spacing_bounds() {
        assert_eq!(exact_tick_steps_for_spacing(0), 0);
        assert_eq!(exact_tick_steps_for_spacing(-1), 0);
        assert_eq!(exact_tick_steps_for_spacing(1), 20);
        assert_eq!(exact_tick_steps_for_spacing(10), 20);
        assert_eq!(exact_tick_steps_for_spacing(60), 14);
        assert_eq!(exact_tick_steps_for_spacing(200), 8);
    }

    #[test]
    fn test_full_range_ticks_for_spacing() {
        assert_eq!(full_range_ticks_for_spacing(0), None);
        assert_eq!(full_range_ticks_for_spacing(-1), None);
        assert_eq!(full_range_ticks_for_spacing(1), Some((-887_272, 887_272)));
        assert_eq!(full_range_ticks_for_spacing(10), Some((-887_270, 887_270)));
        assert_eq!(full_range_ticks_for_spacing(60), Some((-887_220, 887_220)));
    }

    #[test]
    fn test_full_range_liquidity_net_adjustment() {
        let spacing = 20;
        let (fr_lower, fr_upper) = full_range_ticks_for_spacing(spacing).unwrap();
        let fl: u128 = 1_000_000;

        // At the full-range lower tick: subtract full-range contribution
        assert_eq!(
            full_range_liquidity_net_adjustment(fr_lower, spacing, fr_lower, fr_upper, fl),
            -(fl as i128)
        );

        // At the full-range upper tick: add back full-range contribution
        assert_eq!(
            full_range_liquidity_net_adjustment(fr_upper, spacing, fr_lower, fr_upper, fl),
            fl as i128
        );

        // At an unrelated tick: no adjustment
        assert_eq!(
            full_range_liquidity_net_adjustment(0, spacing, fr_lower, fr_upper, fl),
            0
        );

        // With zero full-range liquidity: no adjustment anywhere
        assert_eq!(
            full_range_liquidity_net_adjustment(fr_lower, spacing, fr_lower, fr_upper, 0),
            0
        );

        // With zero spacing: no adjustment
        assert_eq!(
            full_range_liquidity_net_adjustment(fr_lower, 0, fr_lower, fr_upper, fl),
            0
        );
    }
}

#[cfg(test)]
mod wordbitmap_tests {
    use super::*;
    use crate::storage::get_tick;
    use crate::testutils::{create_pool_contract, create_token_contract, get_token_admin_client};
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Address, Env, Vec};

    #[test]
    fn test_plane_find_tick_across_chunk_bitmap_word_boundary_lte() {
        let env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let spacing = 10;
        let admin = Address::generate(&env);
        let router = Address::generate(&env);
        let user = Address::generate(&env);

        let token_a = create_token_contract(&env, &admin);
        let token_b = create_token_contract(&env, &admin);
        let (token0, token1) = if token_a.address < token_b.address {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        };
        let plane = pool_plane::Client::new(&env, &env.register(pool_plane::WASM, ()));
        let pool = create_pool_contract(
            &env,
            &admin,
            &router,
            &plane.address,
            &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
            30,
            spacing,
        );

        get_token_admin_client(&env, &token0.address).mint(&user, &2_000_0000000);
        get_token_admin_client(&env, &token1.address).mint(&user, &2_000_0000000);

        pool.deposit_position(
            &user,
            &-887_270,
            &887_270,
            &Vec::from_array(&env, [1_000_0000000u128, 1_000_0000000u128]),
            &0,
        );
        pool.deposit_position(
            &user,
            &-20,
            &-10,
            &Vec::from_array(&env, [0u128, 100_0000000u128]),
            &0,
        );

        env.as_contract(&pool.address, || {
            let expected = get_tick(&env, -10, spacing).liquidity_net;

            let mut cc = ChunkCache::new(&env);
            assert_eq!(
                find_initialized_tick(&env, 0, i32::MIN, spacing, true, &mut cc),
                Some((-10, expected))
            );

            let mut cc = ChunkCache::new(&env);
            assert_eq!(
                find_initialized_tick(&env, 0, -1, spacing, true, &mut cc),
                Some((-10, expected))
            );

            let mut cc = ChunkCache::new(&env);
            assert_eq!(
                find_initialized_tick(&env, 0, 0, spacing, true, &mut cc),
                None
            );
        });
    }

    #[test]
    fn test_plane_find_tick_across_chunk_bitmap_word_boundary_gte() {
        let env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let spacing = 10;
        let admin = Address::generate(&env);
        let router = Address::generate(&env);
        let user = Address::generate(&env);

        let token_a = create_token_contract(&env, &admin);
        let token_b = create_token_contract(&env, &admin);
        let (token0, token1) = if token_a.address < token_b.address {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        };
        let plane = pool_plane::Client::new(&env, &env.register(pool_plane::WASM, ()));
        let pool = create_pool_contract(
            &env,
            &admin,
            &router,
            &plane.address,
            &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
            30,
            spacing,
        );

        get_token_admin_client(&env, &token0.address).mint(&user, &2_000_0000000);
        get_token_admin_client(&env, &token1.address).mint(&user, &2_000_0000000);

        pool.deposit_position(
            &user,
            &-887_270,
            &887_270,
            &Vec::from_array(&env, [1_000_0000000u128, 1_000_0000000u128]),
            &0,
        );
        pool.deposit_position(
            &user,
            &0,
            &20,
            &Vec::from_array(&env, [100_0000000u128, 100_0000000u128]),
            &0,
        );

        env.as_contract(&pool.address, || {
            let expected = get_tick(&env, 0, spacing).liquidity_net;

            let mut cc = ChunkCache::new(&env);
            assert_eq!(
                find_initialized_tick(&env, -17, i32::MAX, spacing, false, &mut cc),
                Some((0, expected))
            );

            let mut cc = ChunkCache::new(&env);
            assert_eq!(
                find_initialized_tick(&env, -17, 0, spacing, false, &mut cc),
                Some((0, expected))
            );

            let mut cc = ChunkCache::new(&env);
            assert_eq!(
                find_initialized_tick(&env, -17, -1, spacing, false, &mut cc),
                None
            );
        });
    }
}
