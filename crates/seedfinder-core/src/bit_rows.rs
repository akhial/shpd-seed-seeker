//! Conversion between byte-sized boolean cells and bit-parallel terrain rows.
//!
//! SSE2 handles sixteen cells at a time on x86-64 without runtime dispatch or
//! additional CPU requirements. Portable eight-cell groups handle other
//! architectures; x86-64 retains scalar vector tails.

#![allow(unsafe_code)]

/// Packs at most 64 cells, with the first cell in the least significant bit.
pub(crate) fn pack(cells: &[bool]) -> u64 {
    assert!(cells.len() <= 64);
    let mut bits = 0_u64;
    #[cfg(not(target_arch = "x86_64"))]
    let offset = 0;
    #[cfg(target_arch = "x86_64")]
    let offset = {
        use std::arch::x86_64::{_mm_loadu_si128, _mm_movemask_epi8, _mm_slli_epi16};
        let mut offset = 0;
        for chunk in cells.chunks_exact(16) {
            // SAFETY: SSE2 is baseline on x86-64. The chunk contains sixteen
            // initialized bool bytes (0 or 1); the unaligned load stays inside
            // it. Shifting each word by seven puts both bools in its sign bits.
            let mask = unsafe {
                _mm_movemask_epi8(_mm_slli_epi16::<7>(_mm_loadu_si128(chunk.as_ptr().cast())))
            };
            bits |= u64::from(mask.unsigned_abs()) << offset;
            offset += 16;
        }
        offset
    };
    #[cfg(not(target_arch = "x86_64"))]
    {
        bits |= pack_portable(&cells[offset..], offset);
    }
    #[cfg(target_arch = "x86_64")]
    for (index, &cell) in cells[offset..].iter().enumerate() {
        bits |= u64::from(cell) << (offset + index);
    }
    bits
}

/// Unpacks the low bits into at most 64 cells.
pub(crate) fn unpack(bits: u64, cells: &mut [bool]) {
    assert!(cells.len() <= 64);
    #[cfg(not(target_arch = "x86_64"))]
    let offset = 0;
    #[cfg(target_arch = "x86_64")]
    let offset = {
        use std::arch::x86_64::{
            _mm_and_si128, _mm_cmpeq_epi8, _mm_set_epi64x, _mm_set1_epi8, _mm_storeu_si128,
            _mm_unpacklo_epi64,
        };
        let mut offset = 0;
        for chunk in cells.chunks_exact_mut(16) {
            let bytes = (bits >> offset).to_le_bytes();
            // SAFETY: SSE2 is baseline on x86-64, and the unaligned store is
            // limited to the sixteen writable bytes in this chunk. Masking
            // the comparison with 1 writes only valid bool representations.
            unsafe {
                let repeated = _mm_unpacklo_epi64(
                    _mm_set1_epi8(i8::from_ne_bytes([bytes[0]])),
                    _mm_set1_epi8(i8::from_ne_bytes([bytes[1]])),
                );
                let masks = _mm_set_epi64x(
                    i64::from_ne_bytes([1, 2, 4, 8, 16, 32, 64, 128]),
                    i64::from_ne_bytes([1, 2, 4, 8, 16, 32, 64, 128]),
                );
                let values = _mm_and_si128(
                    _mm_cmpeq_epi8(_mm_and_si128(repeated, masks), masks),
                    _mm_set1_epi8(1),
                );
                _mm_storeu_si128(chunk.as_mut_ptr().cast(), values);
            }
            offset += 16;
        }
        offset
    };
    #[cfg(not(target_arch = "x86_64"))]
    unpack_portable(bits, &mut cells[offset..], offset);
    #[cfg(target_arch = "x86_64")]
    for (index, cell) in cells[offset..].iter_mut().enumerate() {
        *cell = bits & (1 << (offset + index)) != 0;
    }
}

// The slice is the remaining cells and offset is their first bit position.
// Keeping the initial offset inside the loops makes an empty tail at bit 64
// valid without ever evaluating a shift by 64.
#[cfg(any(test, not(target_arch = "x86_64")))]
fn pack_portable(cells: &[bool], mut offset: usize) -> u64 {
    debug_assert!(offset <= 64 && cells.len() <= 64 - offset);
    let mut bits = 0;
    let mut chunks = cells.chunks_exact(8);
    for chunk in &mut chunks {
        bits |= pack_eight(chunk) << offset;
        offset += 8;
    }
    for (index, &cell) in chunks.remainder().iter().enumerate() {
        bits |= u64::from(cell) << (offset + index);
    }
    bits
}

#[cfg(any(test, not(target_arch = "x86_64")))]
fn unpack_portable(bits: u64, cells: &mut [bool], mut offset: usize) {
    debug_assert!(offset <= 64 && cells.len() <= 64 - offset);
    let mut chunks = cells.chunks_exact_mut(8);
    for chunk in &mut chunks {
        chunk.copy_from_slice(&unpack_eight((bits >> offset).to_le_bytes()[0]));
        offset += 8;
    }
    for (index, cell) in chunks.into_remainder().iter_mut().enumerate() {
        *cell = bits & (1 << (offset + index)) != 0;
    }
}

// Each input byte is 0 or 1. The multiplication gathers the eight bits
// into its top byte in cell order, with no carries between byte sums.
#[cfg(any(test, not(target_arch = "x86_64")))]
fn pack_eight(cells: &[bool]) -> u64 {
    let cells: &[bool; 8] = cells.try_into().expect("exact eight-cell chunk");
    let bytes = [
        u8::from(cells[0]),
        u8::from(cells[1]),
        u8::from(cells[2]),
        u8::from(cells[3]),
        u8::from(cells[4]),
        u8::from(cells[5]),
        u8::from(cells[6]),
        u8::from(cells[7]),
    ];
    u64::from_le_bytes(bytes).wrapping_mul(0x0102_0408_1020_4080) >> 56
}

// The u8 input repeats without overflowing the u64 product. The isolated
// byte values are at most 128; adding 127 cannot carry into
// an adjacent byte. Shifting then leaves exactly the bool representation.
#[cfg(any(test, not(target_arch = "x86_64")))]
fn unpack_eight(bits: u8) -> [bool; 8] {
    let isolated = (u64::from(bits) * 0x0101_0101_0101_0101) & 0x8040_2010_0804_0201;
    let cells = ((isolated + 0x7F7F_7F7F_7F7F_7F7F) >> 7) & 0x0101_0101_0101_0101;
    cells.to_le_bytes().map(|cell| cell != 0)
}

#[cfg(test)]
mod tests {
    use super::{pack, pack_eight, pack_portable, unpack, unpack_eight, unpack_portable};

    #[test]
    fn portable_eight_cell_groups_match_every_pattern() {
        for bits in 0..=u8::MAX {
            let expected: [bool; 8] = std::array::from_fn(|index| bits & (1 << index) != 0);
            assert_eq!(unpack_eight(bits), expected);
            assert_eq!(pack_eight(&expected), u64::from(bits));
        }
    }

    #[test]
    fn portable_tails_preserve_initial_bit_offsets() {
        let patterns = [0, u64::MAX, 0xAAAA_AAAA_AAAA_AAAA, 0x5555_5555_5555_5555]
            .into_iter()
            .chain((0..64).map(|bit| 1_u64 << bit));
        for bits in patterns {
            for initial_bit in 0..=64 {
                for length in 0..=64 - initial_bit {
                    for sentinel in [false, true] {
                        let mut cells = [sentinel; 96];
                        unpack_portable(bits, &mut cells[7..7 + length], initial_bit);
                        let mut expected_bits = 0_u64;
                        for (index, &cell) in cells[7..7 + length].iter().enumerate() {
                            let bit = 1_u64 << (initial_bit + index);
                            assert_eq!(cell, bits & bit != 0);
                            expected_bits |= bits & bit;
                        }
                        assert!(cells[..7].iter().all(|&cell| cell == sentinel));
                        assert!(cells[7 + length..].iter().all(|&cell| cell == sentinel));
                        assert_eq!(
                            pack_portable(&cells[7..7 + length], initial_bit),
                            expected_bits
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn conversions_match_scalar_for_all_lengths_and_unaligned_slices() {
        let mut patterns = vec![0, u64::MAX, 0xAAAA_AAAA_AAAA_AAAA, 0x5555_5555_5555_5555];
        patterns.extend((0..64).map(|bit| 1_u64 << bit));
        let mut random = crate::rng::JavaRandom::new(123);
        patterns.extend((0..128).map(|_| u64::from_ne_bytes(random.next_i64().to_ne_bytes())));
        for bits in patterns {
            for length in 0..=64 {
                for offset in 0..16 {
                    for sentinel in [false, true] {
                        let mut cells = [sentinel; 96];
                        let mut portable = [sentinel; 96];
                        unpack(bits, &mut cells[offset..offset + length]);
                        unpack_portable(bits, &mut portable[offset..offset + length], 0);
                        assert_eq!(cells, portable);
                        for (index, &cell) in cells[offset..offset + length].iter().enumerate() {
                            assert_eq!(cell, bits & (1 << index) != 0);
                        }
                        assert!(cells[..offset].iter().all(|&cell| cell == sentinel));
                        assert!(
                            cells[offset + length..]
                                .iter()
                                .all(|&cell| cell == sentinel)
                        );
                        let mask = u64::MAX
                            .checked_shr(64 - u32::try_from(length).unwrap())
                            .unwrap_or(0);
                        assert_eq!(pack(&cells[offset..offset + length]), bits & mask);
                        assert_eq!(
                            pack_portable(&portable[offset..offset + length], 0),
                            bits & mask
                        );
                    }
                }
            }
        }
    }
}
