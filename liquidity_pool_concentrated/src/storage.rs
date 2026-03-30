use crate::constants::TICKS_PER_CHUNK;
use crate::types::{
    PositionData, PositionRange, ProtocolFees, Slot0, TickData, TickInfo, UserState,
};
use paste::paste;
use soroban_sdk::{contracttype, panic_with_error, Address, BytesN, Env, Vec};
use utils::bump::{bump_instance, bump_persistent};
use utils::storage_errors::StorageError;
use utils::{
    generate_instance_storage_getter, generate_instance_storage_getter_and_setter,
    generate_instance_storage_getter_and_setter_with_default,
    generate_instance_storage_getter_with_default, generate_instance_storage_setter,
};

// Storage layout. Instance storage for pool-wide config (accessed every tx),
// persistent storage for per-user and per-tick data (accessed selectively).
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    // ── Instance: pool config ──
    Router,
    Token0,
    Token1,
    Fee,                 // u32 — basis points (e.g. 30 = 0.3%)
    TickSpacing,         // i32
    ProtocolFeeFraction, // u32 — protocol's share of fees, per FEE_DENOMINATOR
    Plane,

    // ── Instance: pool state ──
    Slot0,                // Slot0 { sqrt_price_x96, tick }
    Liquidity,            // u128 — active liquidity at current tick
    FeeGrowthGlobal0X128, // U256
    FeeGrowthGlobal1X128, // U256
    ProtocolFees,         // ProtocolFees { token0, token1 }
    Reserve0,             // u128 — tracked LP reserve (excludes protocol fees)
    Reserve1,             // u128

    // ── Instance: tick bounds (anti-griefing) ──
    MinInitTick, // i32 — lowest initialized tick
    MaxInitTick, // i32 — highest initialized tick

    // ── Instance: rewards ──
    TotalRawLiquidity,      // u128 — sum of all users' raw liquidity
    TotalWeightedLiquidity, // u128 — sum of all users' weighted liquidity
    FullRangeLiquidity,     // u128 — total liquidity in full-range positions

    // ── Instance: kill switches ──
    IsKilledDeposit,
    IsKilledSwap,
    ClaimKilled,

    // ── Instance: upgrade staging ──
    TokenFutureWasm, // BytesN<32>
    GaugeFutureWasm, // BytesN<32>

    // ── Persistent: tick bitmap (3-level) ──
    //   Level 0: TickChunk — Vec<16 × TickData>, keyed by chunk_pos
    //   Level 1: ChunkBitmap — U256, 1 bit per chunk, keyed by chunk_pos >> 8
    //   Level 2: WordBitmap — U256, 1 bit per ChunkBitmap word, keyed by (chunk_pos >> 8) >> 8
    TickChunk(i32),
    ChunkBitmap(i32),
    WordBitmap(i32),

    // ── Persistent: per-user ──
    Position(Address, i32, i32), // PositionData — keyed by (owner, tick_lower, tick_upper)
    User(Address),               // UserState — positions + raw/weighted liquidity
}

generate_instance_storage_getter_and_setter!(router, DataKey::Router, Address);
generate_instance_storage_getter_and_setter!(plane, DataKey::Plane, Address);

pub fn has_plane(e: &Env) -> bool {
    bump_instance(e);
    e.storage().instance().has(&DataKey::Plane)
}
generate_instance_storage_getter_and_setter!(token0, DataKey::Token0, Address);
generate_instance_storage_getter_and_setter!(token1, DataKey::Token1, Address);
generate_instance_storage_getter_and_setter!(fee, DataKey::Fee, u32);
generate_instance_storage_getter_and_setter!(tick_spacing, DataKey::TickSpacing, i32);
generate_instance_storage_getter_and_setter!(slot0, DataKey::Slot0, Slot0);
generate_instance_storage_getter_and_setter_with_default!(liquidity, DataKey::Liquidity, u128, 0);
generate_instance_storage_getter_and_setter!(
    fee_growth_global_0_x128,
    DataKey::FeeGrowthGlobal0X128,
    soroban_sdk::U256
);
generate_instance_storage_getter_and_setter!(
    fee_growth_global_1_x128,
    DataKey::FeeGrowthGlobal1X128,
    soroban_sdk::U256
);
generate_instance_storage_getter_and_setter_with_default!(
    protocol_fees,
    DataKey::ProtocolFees,
    ProtocolFees,
    ProtocolFees {
        token0: 0,
        token1: 0,
    }
);
generate_instance_storage_getter_and_setter_with_default!(
    protocol_fee_fraction,
    DataKey::ProtocolFeeFraction,
    u32,
    5_000
);
generate_instance_storage_getter_and_setter_with_default!(
    is_killed_deposit,
    DataKey::IsKilledDeposit,
    bool,
    false
);
generate_instance_storage_getter_and_setter_with_default!(
    is_killed_swap,
    DataKey::IsKilledSwap,
    bool,
    false
);
generate_instance_storage_getter_and_setter!(
    token_future_wasm,
    DataKey::TokenFutureWasm,
    BytesN<32>
);
generate_instance_storage_getter_and_setter!(
    gauge_future_wasm,
    DataKey::GaugeFutureWasm,
    BytesN<32>
);
generate_instance_storage_getter_and_setter_with_default!(
    total_raw_liquidity,
    DataKey::TotalRawLiquidity,
    u128,
    0
);
generate_instance_storage_getter_and_setter_with_default!(
    total_weighted_liquidity,
    DataKey::TotalWeightedLiquidity,
    u128,
    0
);
generate_instance_storage_getter_and_setter_with_default!(
    full_range_liquidity,
    DataKey::FullRangeLiquidity,
    u128,
    0
);
generate_instance_storage_getter_and_setter_with_default!(
    claim_killed,
    DataKey::ClaimKilled,
    bool,
    false
);
generate_instance_storage_getter_and_setter_with_default!(reserve0, DataKey::Reserve0, u128, 0);
generate_instance_storage_getter_and_setter_with_default!(reserve1, DataKey::Reserve1, u128, 0);

// Tick bounds: initialized tick range. Default to inverted bounds (empty pool).
generate_instance_storage_getter_and_setter_with_default!(
    min_init_tick,
    DataKey::MinInitTick,
    i32,
    crate::constants::MAX_TICK
);
generate_instance_storage_getter_and_setter_with_default!(
    max_init_tick,
    DataKey::MaxInitTick,
    i32,
    crate::constants::MIN_TICK
);

// ── Position accessors (persistent storage) ──
// Keyed by (owner, tick_lower, tick_upper). A user can have up to MAX_USER_POSITIONS
// distinct ranges. Returns None if position doesn't exist.
pub fn get_position(
    e: &Env,
    owner: &Address,
    tick_lower: i32,
    tick_upper: i32,
) -> Option<PositionData> {
    let key = DataKey::Position(owner.clone(), tick_lower, tick_upper);
    let v = e.storage().persistent().get(&key);
    if v.is_some() {
        bump_persistent(e, &key);
    }
    v
}

pub fn set_position(
    e: &Env,
    owner: &Address,
    tick_lower: i32,
    tick_upper: i32,
    value: &PositionData,
) {
    let key = DataKey::Position(owner.clone(), tick_lower, tick_upper);
    e.storage().persistent().set(&key, value);
    bump_persistent(e, &key);
}

pub fn remove_position(e: &Env, owner: &Address, tick_lower: i32, tick_upper: i32) {
    e.storage()
        .persistent()
        .remove(&DataKey::Position(owner.clone(), tick_lower, tick_upper));
}

// ── Chunk addressing ──

// Compress a tick value by dividing by spacing (floor division).
fn compress_tick_storage(tick: i32, spacing: i32) -> i32 {
    let mut compressed = tick / spacing;
    if tick < 0 && tick % spacing != 0 {
        compressed -= 1;
    }
    compressed
}

// Compute (chunk_pos, slot) from a compressed tick.
// Uses Euclidean division so negative compressed ticks map correctly.
pub fn chunk_address(compressed_tick: i32) -> (i32, u32) {
    let chunk_pos = compressed_tick.div_euclid(TICKS_PER_CHUNK);
    let slot = compressed_tick.rem_euclid(TICKS_PER_CHUNK) as u32;
    (chunk_pos, slot)
}

// ── Tick chunk accessors (persistent storage) ──
// Each chunk holds exactly TICKS_PER_CHUNK TickData entries, pre-allocated at creation.

pub fn get_tick_chunk(e: &Env, chunk_pos: i32) -> Option<Vec<TickData>> {
    let key = DataKey::TickChunk(chunk_pos);
    let v = e.storage().persistent().get(&key);
    if v.is_some() {
        bump_persistent(e, &key);
    }
    v
}

pub fn set_tick_chunk(e: &Env, chunk_pos: i32, chunk: &Vec<TickData>) {
    let key = DataKey::TickChunk(chunk_pos);
    e.storage().persistent().set(&key, chunk);
    bump_persistent(e, &key);
}

// Allocate a zeroed chunk: Vec of TICKS_PER_CHUNK TickData entries.
pub fn new_empty_chunk(e: &Env) -> Vec<TickData> {
    let zero = soroban_sdk::U256::from_u32(e, 0);
    let mut chunk = Vec::new(e);
    for _ in 0..TICKS_PER_CHUNK {
        chunk.push_back(TickData(zero.clone(), zero.clone(), 0, 0));
    }
    chunk
}

// Get or create chunk for a given chunk_pos.
pub fn get_or_create_tick_chunk(e: &Env, chunk_pos: i32) -> Vec<TickData> {
    get_tick_chunk(e, chunk_pos).unwrap_or_else(|| new_empty_chunk(e))
}

// Convenience: read a single tick's TickInfo from chunk storage.
pub fn get_tick(e: &Env, tick: i32, spacing: i32) -> TickInfo {
    let compressed = compress_tick_storage(tick, spacing);
    let (chunk_pos, slot) = chunk_address(compressed);
    match get_tick_chunk(e, chunk_pos) {
        Some(chunk) => TickInfo::from(chunk.get(slot).unwrap()),
        None => TickInfo {
            fee_growth_outside_0_x128: soroban_sdk::U256::from_u32(e, 0),
            fee_growth_outside_1_x128: soroban_sdk::U256::from_u32(e, 0),
            liquidity_gross: 0,
            liquidity_net: 0,
        },
    }
}

// ── Chunk bitmap accessors (persistent storage) ──
// 256-bit words for efficient chunk scanning. Each bit marks a chunk with initialized ticks.
// word_pos = chunk_pos >> 8. bit_pos = chunk_pos & 255.
pub fn get_chunk_bitmap_word(e: &Env, word_pos: i32) -> soroban_sdk::U256 {
    let key = DataKey::ChunkBitmap(word_pos);
    match e.storage().persistent().get(&key) {
        Some(word) => {
            bump_persistent(e, &key);
            word
        }
        None => soroban_sdk::U256::from_u32(e, 0),
    }
}

pub fn set_chunk_bitmap_word(e: &Env, word_pos: i32, word: &soroban_sdk::U256) {
    let key = DataKey::ChunkBitmap(word_pos);
    e.storage().persistent().set(&key, word);
    bump_persistent(e, &key);
}

// ── Level-2 word bitmap accessors (persistent storage) ──
// 256-bit words for efficient ChunkBitmap word scanning.
// Each bit marks a ChunkBitmap word that has any chunk bit set.
// l2_word_pos = chunk_bitmap_word_pos >> 8. bit_pos = chunk_bitmap_word_pos & 255.
pub fn get_word_bitmap(e: &Env, l2_word_pos: i32) -> soroban_sdk::U256 {
    let key = DataKey::WordBitmap(l2_word_pos);
    match e.storage().persistent().get(&key) {
        Some(word) => {
            bump_persistent(e, &key);
            word
        }
        None => soroban_sdk::U256::from_u32(e, 0),
    }
}

pub fn set_word_bitmap(e: &Env, l2_word_pos: i32, word: &soroban_sdk::U256) {
    let key = DataKey::WordBitmap(l2_word_pos);
    e.storage().persistent().set(&key, word);
    bump_persistent(e, &key);
}

// ── Chunk cache (write-back with explicit flush) ──
// Avoids repeated XDR deserialization of Vec<TickData> within one operation.
// Read-through: first get loads from storage and caches.
// Write-back: set_chunk updates only the cache; flush() persists all dirty chunks to storage.
// Caller must call flush() before the cache is dropped to persist writes.
pub struct ChunkCache {
    cache: soroban_sdk::Map<i32, Vec<TickData>>,
    dirty: soroban_sdk::Map<i32, bool>,
}

impl ChunkCache {
    pub fn new(e: &Env) -> Self {
        Self {
            cache: soroban_sdk::Map::new(e),
            dirty: soroban_sdk::Map::new(e),
        }
    }

    // Read-through: returns cached chunk or loads from storage (caching the result).
    pub fn get_chunk(&mut self, e: &Env, chunk_pos: i32) -> Option<Vec<TickData>> {
        if let Some(cached) = self.cache.get(chunk_pos) {
            return Some(cached);
        }
        let chunk = get_tick_chunk(e, chunk_pos);
        if let Some(ref c) = chunk {
            self.cache.set(chunk_pos, c.clone());
        }
        chunk
    }

    // Read-through with lazy allocation: returns cached/stored chunk, or creates an empty one.
    pub fn get_or_create_chunk(&mut self, e: &Env, chunk_pos: i32) -> Vec<TickData> {
        if let Some(cached) = self.cache.get(chunk_pos) {
            return cached;
        }
        let chunk = get_tick_chunk(e, chunk_pos).unwrap_or_else(|| new_empty_chunk(e));
        self.cache.set(chunk_pos, chunk.clone());
        chunk
    }

    // Write-back: updates chunk in cache and marks it dirty. Does NOT write to storage.
    pub fn set_chunk(&mut self, chunk_pos: i32, chunk: &Vec<TickData>) {
        self.cache.set(chunk_pos, chunk.clone());
        self.dirty.set(chunk_pos, true);
    }

    // Read a single tick from cached chunk.
    pub fn get_tick(&mut self, e: &Env, tick: i32, spacing: i32) -> TickInfo {
        let compressed = compress_tick_storage(tick, spacing);
        let (chunk_pos, slot) = chunk_address(compressed);
        match self.get_chunk(e, chunk_pos) {
            Some(chunk) => TickInfo::from(chunk.get(slot).unwrap()),
            None => TickInfo {
                fee_growth_outside_0_x128: soroban_sdk::U256::from_u32(e, 0),
                fee_growth_outside_1_x128: soroban_sdk::U256::from_u32(e, 0),
                liquidity_gross: 0,
                liquidity_net: 0,
            },
        }
    }

    // Persist all dirty chunks to storage.
    pub fn flush(&self, e: &Env) {
        for (chunk_pos, _) in self.dirty.iter() {
            if let Some(chunk) = self.cache.get(chunk_pos) {
                set_tick_chunk(e, chunk_pos, &chunk);
            }
        }
    }
}

// ── Per-user state (single persistent storage entry) ──
// Merged positions + raw/weighted liquidity to save 2 footprint entries per user operation.
pub fn get_user_state(e: &Env, user: &Address) -> UserState {
    let key = DataKey::User(user.clone());
    match e.storage().persistent().get(&key) {
        Some(state) => {
            bump_persistent(e, &key);
            state
        }
        None => UserState {
            positions: Vec::new(e),
            raw_liquidity: 0,
            weighted_liquidity: 0,
        },
    }
}

pub fn set_user_state(e: &Env, user: &Address, state: &UserState) {
    let key = DataKey::User(user.clone());
    e.storage().persistent().set(&key, state);
    bump_persistent(e, &key);
}

// Convenience read-only accessors — delegate to get_user_state.
pub fn get_user_raw_liquidity(e: &Env, user: &Address) -> u128 {
    get_user_state(e, user).raw_liquidity
}

pub fn get_user_weighted_liquidity(e: &Env, user: &Address) -> u128 {
    get_user_state(e, user).weighted_liquidity
}
