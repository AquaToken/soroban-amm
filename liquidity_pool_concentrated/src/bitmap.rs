use crate::constants::{MAX_TICK, MIN_TICK};
use soroban_sdk::{Bytes, Env, U256};

pub(crate) fn u256_to_array(v: &U256) -> [u8; 32] {
    let bytes = v.to_be_bytes();
    let mut out = [0u8; 32];
    bytes.copy_into_slice(&mut out);
    out
}

pub(crate) fn u256_from_array(e: &Env, bytes: &[u8; 32]) -> U256 {
    U256::from_be_bytes(e, &Bytes::from_array(e, bytes))
}

pub(crate) fn set_bit(word: &mut [u8; 32], bit_pos: u32, value: bool) {
    if bit_pos >= 256 {
        return;
    }

    let byte_idx = 31usize - (bit_pos / 8) as usize;
    let bit_idx = (bit_pos % 8) as u8;
    let mask = 1u8 << bit_idx;
    if value {
        word[byte_idx] |= mask;
    } else {
        word[byte_idx] &= !mask;
    }
}

pub(crate) fn find_prev_set_bit(word: &[u8; 32], from_bit: u32) -> Option<u32> {
    let from_bit = from_bit.min(255);
    // Big-endian: byte 0 = bits 255..248, byte 31 = bits 7..0
    let start_byte = (255 - from_bit) / 8;
    let start_bit_in_byte = from_bit % 8;

    // Check the first (partial) byte — mask off bits above from_bit
    let mask = ((1u16 << (start_bit_in_byte + 1)) - 1) as u8;
    let masked = word[start_byte as usize] & mask;
    if masked != 0 {
        let top_bit = 7 - masked.leading_zeros();
        return Some((31 - start_byte) * 8 + top_bit);
    }

    // Scan remaining bytes downward (higher byte index = lower bits)
    for byte_idx in (start_byte + 1)..32 {
        if word[byte_idx as usize] != 0 {
            let top_bit = 7 - word[byte_idx as usize].leading_zeros();
            return Some((31 - byte_idx) * 8 + top_bit);
        }
    }

    None
}

pub(crate) fn find_next_set_bit(word: &[u8; 32], from_bit: u32) -> Option<u32> {
    let from_bit = from_bit.min(255);
    let start_byte = (255 - from_bit) / 8;
    let start_bit_in_byte = from_bit % 8;

    // Check the first (partial) byte — mask off bits below from_bit
    let mask = !((1u8 << start_bit_in_byte).wrapping_sub(1));
    let masked = word[start_byte as usize] & mask;
    if masked != 0 {
        let low_bit = masked.trailing_zeros();
        return Some((31 - start_byte) * 8 + low_bit);
    }

    // Scan remaining bytes upward (lower byte index = higher bits)
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

// Chunk bitmap addressing: 1 bit per chunk.
pub(crate) fn chunk_bitmap_position(chunk_pos: i32) -> (i32, u32) {
    let word_pos = chunk_pos >> 8;
    let bit_pos = (chunk_pos & 255) as u32;
    (word_pos, bit_pos)
}

pub(crate) fn compress_tick(tick: i32, spacing: i32) -> i32 {
    let mut compressed = tick / spacing;
    if tick < 0 && tick % spacing != 0 {
        compressed -= 1;
    }
    compressed
}

pub(crate) fn clamp_tick(tick: i32) -> i32 {
    tick.max(MIN_TICK).min(MAX_TICK)
}

pub(crate) fn compressed_to_tick(compressed: i32, spacing: i32) -> i32 {
    clamp_tick(compressed.saturating_mul(spacing))
}
