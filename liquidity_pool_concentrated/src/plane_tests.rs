use super::{
    exact_tick_steps_for_spacing, find_initialized_tick, full_range_liquidity_net_adjustment,
    full_range_ticks_for_spacing,
};
use crate::bitmap::{
    chunk_bitmap_position, compress_tick, set_bit, u256_from_array, word_bitmap_position,
};
use crate::contract::ConcentratedLiquidityPool;
use crate::storage::{
    chunk_address, get_or_create_tick_chunk, set_chunk_bitmap_word, set_tick_chunk,
    set_word_bitmap, ChunkCache,
};
use crate::types::TickData;
use soroban_sdk::{Env, U256};

fn initialized_tick_data(e: &Env, liquidity_net: i128) -> TickData {
    TickData(
        U256::from_u32(e, 0),
        U256::from_u32(e, 0),
        1_u128,
        liquidity_net,
    )
}

fn seed_initialized_tick(e: &Env, compressed: i32, liquidity_net: i128) {
    let (chunk_pos, slot) = chunk_address(compressed);
    let mut chunk = get_or_create_tick_chunk(e, chunk_pos);
    chunk.set(slot, initialized_tick_data(e, liquidity_net));
    set_tick_chunk(e, chunk_pos, &chunk);

    let (bm_word_pos, bm_bit_pos) = chunk_bitmap_position(chunk_pos);
    let mut bm_arr = [0u8; 32];
    set_bit(&mut bm_arr, bm_bit_pos, true);
    set_chunk_bitmap_word(e, bm_word_pos, &u256_from_array(e, &bm_arr));

    let (l2_word_pos, l2_bit_pos) = word_bitmap_position(bm_word_pos);
    let mut l2_arr = [0u8; 32];
    set_bit(&mut l2_arr, l2_bit_pos, true);
    set_word_bitmap(e, l2_word_pos, &u256_from_array(e, &l2_arr));
}

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

#[test]
fn test_find_initialized_tick_downward_across_chunk_bitmap_word_boundary() {
    let e = Env::default();
    let contract_id = e.register(ConcentratedLiquidityPool {}, ());

    e.as_contract(&contract_id, || {
        seed_initialized_tick(&e, -1, 500);

        let mut cc = ChunkCache::new(&e);
        assert_eq!(
            find_initialized_tick(&e, 0, i32::MIN, 200, true, &mut cc),
            Some((-200, 500))
        );
    });
}

#[test]
fn test_find_initialized_tick_upward_across_chunk_bitmap_word_boundary() {
    let e = Env::default();
    let contract_id = e.register(ConcentratedLiquidityPool {}, ());

    e.as_contract(&contract_id, || {
        seed_initialized_tick(&e, 4_096, 750);

        let mut cc = ChunkCache::new(&e);
        assert_eq!(
            find_initialized_tick(&e, 4_080, i32::MAX, 200, false, &mut cc),
            Some((819_200, 750))
        );
    });
}

#[test]
fn test_find_initialized_tick_downward_l2_respects_limit_compressed() {
    let e = Env::default();
    let contract_id = e.register(ConcentratedLiquidityPool {}, ());

    e.as_contract(&contract_id, || {
        let compressed = compress_tick(-3_200, 200);
        assert_eq!(chunk_address(compressed), (-1, 0));
        seed_initialized_tick(&e, compressed, 300);

        let mut unbounded_cc = ChunkCache::new(&e);
        assert_eq!(
            find_initialized_tick(&e, 0, i32::MIN, 200, true, &mut unbounded_cc),
            Some((-3_200, 300))
        );

        let mut bounded_cc = ChunkCache::new(&e);
        assert_eq!(
            find_initialized_tick(&e, 0, -10, 200, true, &mut bounded_cc),
            None
        );
    });
}

#[test]
fn test_find_initialized_tick_equivalence() {
    let run_case = |current_tick: i32, init_tick: i32, lte: bool, liquidity_net: i128| {
        let e = Env::default();
        let contract_id = e.register(ConcentratedLiquidityPool {}, ());

        e.as_contract(&contract_id, || {
            let spacing = 1;
            let current_compressed = compress_tick(current_tick, spacing);
            let init_compressed = compress_tick(init_tick, spacing);
            seed_initialized_tick(&e, init_compressed, liquidity_net);

            let permissive_limit = if lte { i32::MIN } else { i32::MAX };
            let mut plane_cc = ChunkCache::new(&e);
            let plane_result = find_initialized_tick(
                &e,
                current_compressed,
                permissive_limit,
                spacing,
                lte,
                &mut plane_cc,
            );

            let mut internal_cc = ChunkCache::new(&e);
            let (internal_tick, internal_initialized) =
                ConcentratedLiquidityPool::find_initialized_tick_in_word(
                    &e,
                    current_tick,
                    spacing,
                    lte,
                    &mut internal_cc,
                );

            assert!(internal_initialized);
            assert_eq!(internal_tick, init_tick);
            assert_eq!(plane_result, Some((init_tick, liquidity_net)));
            assert_eq!(plane_result.unwrap().0, internal_tick);

            let bounded_limit = if lte {
                init_compressed + 1
            } else {
                init_compressed - 1
            };
            let mut bounded_cc = ChunkCache::new(&e);
            assert_eq!(
                find_initialized_tick(
                    &e,
                    current_compressed,
                    bounded_limit,
                    spacing,
                    lte,
                    &mut bounded_cc,
                ),
                None
            );
        });
    };

    run_case(5, 3, true, 101);
    run_case(5, 8, false, 102);
    run_case(1_600, 815, true, 201);
    run_case(815, 1_600, false, 202);
    run_case(4_160, 0, true, 301);
    run_case(0, 4_160, false, 302);
}
