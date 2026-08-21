//! Encoding functions for vlen.
//!
//! The array-based functions in this module are the fast, infallible
//! core of the codec: their array parameter types guarantee enough room
//! for any value of the type, so they cannot fail. They may write to
//! bytes of the array beyond the returned length; only the first
//! `returned length` bytes are part of the encoding. All of them are
//! `const fn`, so they can also be evaluated at compile time.
//!
//! For encoding into arbitrary slices with error handling, use the
//! [`Encode`] trait or the free [`encode`](crate::encode()) function.

use crate::error::{Error, Result};

/// Returns the total encoded length announced by a `vlen` prefix byte.
#[must_use]
pub const fn encoded_len(b: u8) -> usize {
	match b {
		_ if b < 0x80 => 1,
		_ if b < 0xC0 => 2,
		_ if b < 0xE0 => 3,
		_ if b < 0xF0 => 4,
		_ => ((b & 0x0F) + 2) as usize,
	}
}

/// Calculates the encoded size of a `u16` value without encoding it.
#[inline]
#[must_use]
pub const fn encoded_size_u16(value: u16) -> usize {
	match value {
		_ if value < 0x80 => 1,
		_ if value < 0x4000 => 2,
		_ => 3,
	}
}

/// Calculates the encoded size of a `u32` value without encoding it.
#[inline]
#[must_use]
pub const fn encoded_size_u32(value: u32) -> usize {
	match value {
		_ if value < 0x80 => 1,
		_ if value < 0x4000 => 2,
		_ if value < 0x200000 => 3,
		_ if value < 0x10000000 => 4,
		_ => 5,
	}
}

/// Calculates the encoded size of a `u64` value without encoding it.
#[inline]
#[must_use]
pub const fn encoded_size_u64(value: u64) -> usize {
	if value <= u32::MAX as u64 {
		encoded_size_u32(value as u32)
	} else {
		let len = ((value.leading_zeros() >> 3) as u8) ^ 0b111;
		(len + 2) as usize
	}
}

/// Calculates the encoded size of a `u128` value without encoding it.
#[inline]
#[must_use]
pub const fn encoded_size_u128(value: u128) -> usize {
	if value <= u64::MAX as u128 {
		encoded_size_u64(value as u64)
	} else {
		let len = ((value.leading_zeros() >> 3) as u8) ^ 0b1111;
		(len + 2) as usize
	}
}

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
			buf[0] = 0xC0 | ((value & 0x1F) as u8);
			buf[1] = (value >> 5) as u8;
			buf[2] = (value >> 13) as u8;
			3
		},
	}
}

/// Generates the encoder for a wide unsigned type. Values up to
/// `2^28` use the shared prefix-varint forms; larger values use the
/// binary length prefix, whose payload is written at full width (the
/// bytes past the returned length are scratch).
macro_rules! encode_unsigned {
	($(#[$docs:meta])* $name:ident, $ut:ident, $size:expr, $len_mask:expr) => {
		$(#[$docs])*
		#[inline]
		#[must_use]
		pub const fn $name(buf: &mut [u8; $size], value: $ut) -> usize {
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
				_ if value < 0x200000 => {
					buf[0] = 0xC0 | ((value & 0x1F) as u8);
					buf[1] = (value >> 5) as u8;
					buf[2] = (value >> 13) as u8;
					3
				},
				_ if value < 0x10000000 => {
					buf[0] = 0xE0 | ((value & 0x0F) as u8);
					buf[1] = (value >> 4) as u8;
					buf[2] = (value >> 12) as u8;
					buf[3] = (value >> 20) as u8;
					4
				},
				_ => {
					let bytes = value.to_le_bytes();
					let mut i = 0;
					while i < $size - 1 {
						buf[i + 1] = bytes[i];
						i += 1;
					}
					let len = ((value.leading_zeros() >> 3) as u8) ^ $len_mask;
					buf[0] = 0xF0 | len;
					(len + 2) as usize
				},
			}
		}
	};
}

encode_unsigned! {
	/// Encodes a `u32` into a buffer, returning the encoded length.
	encode_u32, u32, 5, 0b11
}

encode_unsigned! {
	/// Encodes a `u64` into a buffer, returning the encoded length.
	encode_u64, u64, 9, 0b111
}

encode_unsigned! {
	/// Encodes a `u128` into a buffer, returning the encoded length.
	encode_u128, u128, 17, 0b1111
}

/// Generates the zigzag encoder for a signed type.
macro_rules! encode_signed {
	($(#[$docs:meta])* $name:ident, $it:ident, $ut:ident, $encode_fn:ident, $size:expr) => {
		$(#[$docs])*
		#[inline]
		#[must_use]
		pub const fn $name(buf: &mut [u8; $size], value: $it) -> usize {
			$encode_fn(buf, zigzag!($it, $ut, value))
		}
	};
}

/// Maps a signed value to its zigzag unsigned representation.
macro_rules! zigzag {
	($it:ident, $ut:ident, $value:expr) => {
		(($value >> ($ut::BITS - 1)) as $ut) ^ (($value << 1) as $ut)
	};
}

encode_signed! {
	/// Encodes an `i16` into a buffer, returning the encoded length.
	encode_i16, i16, u16, encode_u16, 3
}

encode_signed! {
	/// Encodes an `i32` into a buffer, returning the encoded length.
	encode_i32, i32, u32, encode_u32, 5
}

encode_signed! {
	/// Encodes an `i64` into a buffer, returning the encoded length.
	encode_i64, i64, u64, encode_u64, 9
}

encode_signed! {
	/// Encodes an `i128` into a buffer, returning the encoded length.
	encode_i128, i128, u128, encode_u128, 17
}

/// Generates the reverse-endian encoder for a floating-point type.
macro_rules! encode_float {
	($(#[$docs:meta])* $name:ident, $ft:ident, $encode_fn:ident, $size:expr) => {
		$(#[$docs])*
		#[inline]
		#[must_use]
		pub const fn $name(buf: &mut [u8; $size], value: $ft) -> usize {
			$encode_fn(buf, value.to_bits().swap_bytes())
		}
	};
}

encode_float! {
	/// Encodes an `f32` into a buffer, returning the encoded length.
	encode_f32, f32, encode_u32, 5
}

encode_float! {
	/// Encodes an `f64` into a buffer, returning the encoded length.
	encode_f64, f64, encode_u64, 9
}

/// Encodes a value into a slice, returning the encoded length.
///
/// Unlike the array-based functions, the buffer only needs room for the
/// value's actual encoded size, not the type's maximum.
#[inline]
pub fn encode<T: Encode>(buf: &mut [u8], value: T) -> Result<usize> {
	value.encode(buf)
}

/// Calculates the encoded size of a value without encoding it.
#[inline]
#[must_use]
pub fn encoded_size<T: Encode>(value: T) -> usize {
	value.encoded_size()
}

/// Types that can be encoded using vlen.
pub trait Encode: Copy {
	/// The maximum possible encoded size for this type.
	const MAX_ENCODED_SIZE: usize;

	/// Calculates the encoded size of the value without encoding it.
	#[must_use]
	fn encoded_size(self) -> usize;

	/// Encodes the value into the slice, returning the encoded length.
	///
	/// The slice only needs room for the value's actual encoded size.
	/// Fails with [`Error::BufferTooSmall`] otherwise.
	fn encode(self, buf: &mut [u8]) -> Result<usize>;
}

/// Implements [`Encode`] on top of an array-based encoder plus a size
/// expression evaluated with the value bound to `$v`.
macro_rules! impl_encode {
	($t:ty, $size:expr, $encode_fn:ident, $v:ident => $size_expr:expr) => {
		impl Encode for $t {
			const MAX_ENCODED_SIZE: usize = $size;

			#[inline]
			fn encoded_size(self) -> usize {
				let $v = self;
				$size_expr
			}

			#[inline]
			fn encode(self, buf: &mut [u8]) -> Result<usize> {
				if let Some(arr) = buf.first_chunk_mut::<$size>() {
					return Ok($encode_fn(arr, self));
				}
				let mut tmp = [0u8; $size];
				let len = $encode_fn(&mut tmp, self);
				match buf.get_mut(..len) {
					Some(dst) => {
						dst.copy_from_slice(&tmp[..len]);
						Ok(len)
					},
					None => Err(Error::BufferTooSmall {
						needed: len,
						available: buf.len(),
					}),
				}
			}
		}
	};
}

impl_encode!(u16, 3, encode_u16, v => encoded_size_u16(v));
impl_encode!(u32, 5, encode_u32, v => encoded_size_u32(v));
impl_encode!(u64, 9, encode_u64, v => encoded_size_u64(v));
impl_encode!(u128, 17, encode_u128, v => encoded_size_u128(v));

impl_encode!(i16, 3, encode_i16, v => encoded_size_u16(zigzag!(i16, u16, v)));
impl_encode!(i32, 5, encode_i32, v => encoded_size_u32(zigzag!(i32, u32, v)));
impl_encode!(i64, 9, encode_i64, v => encoded_size_u64(zigzag!(i64, u64, v)));
impl_encode!(
	i128, 17, encode_i128,
	v => encoded_size_u128(zigzag!(i128, u128, v))
);

impl_encode!(f32, 5, encode_f32, v => {
	encoded_size_u32(v.to_bits().swap_bytes())
});
impl_encode!(f64, 9, encode_f64, v => {
	encoded_size_u64(v.to_bits().swap_bytes())
});
