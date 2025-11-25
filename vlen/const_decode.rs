//! Const-compatible decoding functions for vlen

use crate::helpers::const_read_array;
use konst::cmp::min;

/// Decodes a `u16` from a buffer, returning the value and encoded length.
#[inline]
#[must_use]
pub const fn decode_u16(buf: &[u8; 3]) -> (u16, usize) {
	let buf0 = buf[0] as u16;
	match buf0 {
		_ if buf0 < 0x80 => (buf0, 1),
		_ if buf0 < 0xC0 => {
			let low = (buf0 as u8) & 0x3F;
			let value = ((buf[1] as u16) << 6) | (low as u16);
			(value, 2)
		},
		_ if buf0 == 0xDE => {
			let value = ((buf[2] as u16) << 8) | (buf[1] as u16);
			(value, 3)
		},
		_ => {
			// decode_binary_length_prefix logic
			let len = buf[0] & 0x0F;
			let payload_bytes = (len + 1) as usize;

			let effective_bytes = min!(payload_bytes, 2);
			let mask = u16::MAX >> ((2 - effective_bytes) * 8);

			let bytes = const_read_array::<2>(buf, 1);
			let value = u16::from_le_bytes(bytes);
			(value & mask, (len + 2) as usize)
		},
	}
}

/// Decodes a `u32` from a buffer, returning the value and encoded length.
#[inline]
#[must_use]
pub const fn decode_u32(buf: &[u8; 5]) -> (u32, usize) {
	let buf0 = buf[0] as u32;
	match buf0 {
		_ if buf0 >= 0xF0 => {
			// decode_binary_length_prefix logic
			let len = buf[0] & 0x0F;
			let payload_bytes = (len + 1) as usize;

			let effective_bytes = min!(payload_bytes, 4);
			let mask = u32::MAX >> ((4 - effective_bytes) * 8);

			let bytes = const_read_array::<4>(buf, 1);
			let value = u32::from_le_bytes(bytes);
			(value & mask, (len + 2) as usize)
		},
		_ if buf0 < 0xC0 => {
			let sub_buf = const_read_array::<3>(buf, 0);
			let (value, len) = decode_u16(&sub_buf);
			(value as u32, len)
		},
		_ if buf0 < 0xE0 => {
			let low = buf0 & 0x1F;
			let value = ((buf[2] as u32) << 13) | ((buf[1] as u32) << 5) | low;
			(value, 3)
		},
		_ => {
			let value = ((buf[3] as u32) << 20)
				| ((buf[2] as u32) << 12)
				| ((buf[1] as u32) << 4)
				| (buf0 & 0x0F);
			(value, 4)
		},
	}
}

/// Decodes a `u64` from a buffer, returning the value and encoded length.
#[inline]
#[must_use]
pub const fn decode_u64(buf: &[u8; 9]) -> (u64, usize) {
	if buf[0] >= 0xF0 {
		// decode_binary_length_prefix logic
		let len = buf[0] & 0x0F;
		let payload_bytes = (len + 1) as usize;

		let effective_bytes = min!(payload_bytes, 8);
		let mask = u64::MAX >> ((8 - effective_bytes) * 8);

		let bytes = const_read_array::<8>(buf, 1);
		let value = u64::from_le_bytes(bytes);
		(value & mask, (len + 2) as usize)
	} else {
		let sub_buf = const_read_array::<5>(buf, 0);
		let (value, len) = decode_u32(&sub_buf);
		(value as u64, len)
	}
}

/// Decodes a `u128` from a buffer, returning the value and encoded length.
#[inline]
#[must_use]
pub const fn decode_u128(buf: &[u8; 17]) -> (u128, usize) {
	if buf[0] >= 0xF0 {
		// decode_binary_length_prefix logic
		let len = buf[0] & 0x0F;
		let payload_bytes = (len + 1) as usize;

		let effective_bytes = min!(payload_bytes, 16);
		let mask = u128::MAX >> ((16 - effective_bytes) * 8);

		let bytes = const_read_array::<16>(buf, 1);
		let value = u128::from_le_bytes(bytes);
		(value & mask, (len + 2) as usize)
	} else {
		let sub_buf = const_read_array::<9>(buf, 0);
		let (value, len) = decode_u64(&sub_buf);
		(value as u128, len)
	}
}

// Signed integers

/// Decodes an `i16` from a buffer, returning the value and encoded length.
#[inline]
#[must_use]
pub const fn decode_i16(buf: &[u8; 3]) -> (i16, usize) {
	const ZIGZAG_SHIFT: u8 = 1;
	let (zigzag, len) = decode_u16(buf);
	let value = ((zigzag >> ZIGZAG_SHIFT) as i16) ^ (-((zigzag & 1) as i16));
	(value, len)
}

/// Decodes an `i32` from a buffer, returning the value and encoded length.
#[inline]
#[must_use]
pub const fn decode_i32(buf: &[u8; 5]) -> (i32, usize) {
	const ZIGZAG_SHIFT: u8 = 1;
	let (zigzag, len) = decode_u32(buf);
	let value = ((zigzag >> ZIGZAG_SHIFT) as i32) ^ (-((zigzag & 1) as i32));
	(value, len)
}

/// Decodes an `i64` from a buffer, returning the value and encoded length.
#[inline]
#[must_use]
pub const fn decode_i64(buf: &[u8; 9]) -> (i64, usize) {
	const ZIGZAG_SHIFT: u8 = 1;
	let (zigzag, len) = decode_u64(buf);
	let value = ((zigzag >> ZIGZAG_SHIFT) as i64) ^ (-((zigzag & 1) as i64));
	(value, len)
}

/// Decodes an `i128` from a buffer, returning the value and encoded length.
#[inline]
#[must_use]
pub const fn decode_i128(buf: &[u8; 17]) -> (i128, usize) {
	const ZIGZAG_SHIFT: u8 = 1;
	let (zigzag, len) = decode_u128(buf);
	let value = ((zigzag >> ZIGZAG_SHIFT) as i128) ^ (-((zigzag & 1) as i128));
	(value, len)
}
