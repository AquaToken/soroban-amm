// Uniswap V3 tick bounds: tick_at_sqrt_ratio(MIN_SQRT_RATIO) and tick_at_sqrt_ratio(MAX_SQRT_RATIO).
// Price range: [1.0001^-887272, 1.0001^887272] ≈ [2.35e-39, 4.25e+38].
pub const MIN_TICK: i32 = -887_272;
pub const MAX_TICK: i32 = 887_272;
// Fee precision: fee=30 means 30/10_000 = 0.3%.
pub const FEE_DENOMINATOR: u128 = 10_000;
// Max positions per user account (prevents storage bloat from griefing).
pub const MAX_USER_POSITIONS: u32 = 20;
// Number of ticks per chunk. Each chunk is stored as one Vec<TickData> entry.
// Chunk addressing: chunk_pos = compressed_tick.div_euclid(TICKS_PER_CHUNK),
//                   slot      = compressed_tick.rem_euclid(TICKS_PER_CHUNK).
pub const TICKS_PER_CHUNK: i32 = 16;
