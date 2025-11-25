//! Const-compatible encoding functions for vlen

use crate::helpers::const_copy_slice;

/// Encodes a `u16` into a buffer, returning the encoded length.
#[inline]
#[must_use]
pub const fn encode_u16(buf: &mut [u8; 3], value: u16) -> usize {
	match value {
		_ if value < 0x80 => {
			buf[0] = value as u8;
			1
		},
		_ if value < 0x4000 => {
			buf[0] = 0x80 | ((value & 0x3F) as u8);
			buf[1] = (value >> 6) as u8;
			2
		},
		_ => {
			buf[0] = 0xDE;
			let bytes = value.to_le_bytes();
			buf[1] = bytes[0];
			buf[2] = bytes[1];
			3
		},
	}
}

/// Encodes a `u32` into a buffer, returning the encoded length.
#[inline]
#[must_use]
pub const fn encode_u32(buf: &mut [u8; 5], value: u32) -> usize {
	match value {
		_ if value < 0x4000 => {
            let mut sub_buf = [0u8; 3];
            let len = encode_u16(&mut sub_buf, value as u16);
            const_copy_slice(&sub_buf, buf, 0, len);
            len
		},
		_ if value < 0x200000 => {
			buf[0] = 0xC0 | ((value & 0x1F) as u8);
			buf[1] = (value >> 5) as u8;
			buf[2] = (value >> 13) as u8;
			3
		},
		_ if value < 0x10000000 => {
			buf[0] = 0xE0 | ((value & 0x0F) as u8);
            let bytes = value.to_le_bytes();
            const_copy_slice(&bytes, buf, 1, 4);
			4
		},
		_ => {
            let bytes = value.to_le_bytes();
            const_copy_slice(&bytes, buf, 1, 4);
			buf[0] = 0xF3;
			5
		},
	}
}

/// Encodes a `u64` into a buffer, returning the encoded length.
#[inline]
#[must_use]
pub const fn encode_u64(buf: &mut [u8; 9], value: u64) -> usize {
    if value <= u32::MAX as u64 {
        let mut sub_buf = [0u8; 5];
        let len = encode_u32(&mut sub_buf, value as u32);
        const_copy_slice(&sub_buf, buf, 0, len);
        len
    } else {
        let bytes = value.to_le_bytes();
        const_copy_slice(&bytes, buf, 1, 8);
        
        // Calculate length prefix
        const LEN_MASK: u8 = 0b111;
        let len = ((value.leading_zeros() >> 3) as u8) ^ LEN_MASK;
        buf[0] = 0xF0 | len;
        (len + 2) as usize
    }
}

/// Encodes a `u128` into a buffer, returning the encoded length.
#[inline]
#[must_use]
pub const fn encode_u128(buf: &mut [u8; 17], value: u128) -> usize {
    if value <= u64::MAX as u128 {
        let mut sub_buf = [0u8; 9];
        let len = encode_u64(&mut sub_buf, value as u64);
        const_copy_slice(&sub_buf, buf, 0, len);
        len
    } else {
        let bytes = value.to_le_bytes();
        const_copy_slice(&bytes, buf, 1, 16);
        
        // Calculate length prefix
        const LEN_MASK: u8 = 0b1111;
        let len = ((value.leading_zeros() >> 3) as u8) ^ LEN_MASK;
        buf[0] = 0xF0 | len;
        (len + 2) as usize
    }
}

// Signed integers

/// Encodes an `i16` into a buffer, returning the encoded length.
#[inline]
#[must_use]
pub const fn encode_i16(buf: &mut [u8; 3], value: i16) -> usize {
    const ZIGZAG_SHIFT: u8 = 15;
    let zigzag = ((value >> ZIGZAG_SHIFT) as u16) ^ ((value << 1) as u16);
    encode_u16(buf, zigzag)
}

/// Encodes an `i32` into a buffer, returning the encoded length.
#[inline]
#[must_use]
pub const fn encode_i32(buf: &mut [u8; 5], value: i32) -> usize {
    const ZIGZAG_SHIFT: u8 = 31;
    let zigzag = ((value >> ZIGZAG_SHIFT) as u32) ^ ((value << 1) as u32);
    encode_u32(buf, zigzag)
}

/// Encodes an `i64` into a buffer, returning the encoded length.
#[inline]
#[must_use]
pub const fn encode_i64(buf: &mut [u8; 9], value: i64) -> usize {
    const ZIGZAG_SHIFT: u8 = 63;
    let zigzag = ((value >> ZIGZAG_SHIFT) as u64) ^ ((value << 1) as u64);
    encode_u64(buf, zigzag)
}

/// Encodes an `i128` into a buffer, returning the encoded length.
#[inline]
#[must_use]
pub const fn encode_i128(buf: &mut [u8; 17], value: i128) -> usize {
    const ZIGZAG_SHIFT: u8 = 127;
    let zigzag = ((value >> ZIGZAG_SHIFT) as u128) ^ ((value << 1) as u128);
    encode_u128(buf, zigzag)
}
