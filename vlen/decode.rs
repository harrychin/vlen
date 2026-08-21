//! Decoding functions for vlen.
//!
//! The array-based functions in this module are the fast core of the
//! codec: their array parameter types guarantee enough bytes for any
//! encoding of the type, so they cannot fail. They trust their input —
//! on bytes that are not a valid encoding for the type the returned
//! value is unspecified (though the call is always memory-safe and the
//! returned length never exceeds the array size). All of them are
//! `const fn`, so they can also be evaluated at compile time.
//!
//! For decoding untrusted or exactly-sized input, use the [`Decode`]
//! trait or the free [`decode`](crate::decode()) function: those validate
//! prefixes, buffer lengths, and value ranges.

use crate::encode::encoded_len;
use crate::error::{Error, Result};

/// Reads `N` bytes from `buf` starting at `offset` (const-compatible).
#[inline]
const fn read_array<const N: usize>(buf: &[u8], offset: usize) -> [u8; N] {
	let mut arr = [0u8; N];
	let mut i = 0;
	while i < N {
		arr[i] = buf[offset + i];
		i += 1;
	}
	arr
}

/// Decodes a `u16` from a buffer, returning the value and encoded length.
///
/// Three-byte encodings can carry values up to `2^21 - 1`; anything
/// above `u16::MAX` is truncated. Use [`Decode`] to reject such input.
#[inline]
#[must_use]
pub const fn decode_u16(buf: &[u8; 3]) -> (u16, usize) {
	let b0 = buf[0];
	match b0 {
		_ if b0 < 0x80 => (b0 as u16, 1),
		_ if b0 < 0xC0 => (((buf[1] as u16) << 6) | ((b0 & 0x3F) as u16), 2),
		_ => {
			let wide = ((buf[2] as u32) << 13)
				| ((buf[1] as u32) << 5)
				| ((b0 & 0x1F) as u32);
			(wide as u16, 3)
		},
	}
}

/// Generates the decoder for a wide unsigned type. The binary
/// length-prefix branch reads the payload at full width and masks it
/// down to the announced length; announced lengths beyond the type's
/// maximum are clamped to the array size.
macro_rules! decode_unsigned {
	($(#[$docs:meta])* $name:ident, $ut:ident, $size:expr) => {
		$(#[$docs])*
		#[inline]
		#[must_use]
		pub const fn $name(buf: &[u8; $size]) -> ($ut, usize) {
			const WIDTH: usize = core::mem::size_of::<$ut>();
			let b0 = buf[0];
			match b0 {
				_ if b0 < 0x80 => (b0 as $ut, 1),
				_ if b0 < 0xC0 => {
					(((buf[1] as $ut) << 6) | ((b0 & 0x3F) as $ut), 2)
				},
				_ if b0 < 0xE0 => {
					let value = ((buf[2] as $ut) << 13)
						| ((buf[1] as $ut) << 5)
						| ((b0 & 0x1F) as $ut);
					(value, 3)
				},
				_ if b0 < 0xF0 => {
					let value = ((buf[3] as $ut) << 20)
						| ((buf[2] as $ut) << 12)
						| ((buf[1] as $ut) << 4)
						| ((b0 & 0x0F) as $ut);
					(value, 4)
				},
				_ => {
					let payload = ((b0 & 0x0F) as usize) + 1;
					let mask = if payload >= WIDTH {
						$ut::MAX
					} else {
						$ut::MAX >> ((WIDTH - payload) * 8)
					};
					let raw =
						$ut::from_le_bytes(read_array::<WIDTH>(buf, 1));
					let total = payload + 1;
					(raw & mask, if total > $size { $size } else { total })
				},
			}
		}
	};
}

decode_unsigned! {
	/// Decodes a `u32` from a buffer, returning the value and encoded length.
	decode_u32, u32, 5
}

decode_unsigned! {
	/// Decodes a `u64` from a buffer, returning the value and encoded length.
	decode_u64, u64, 9
}

decode_unsigned! {
	/// Decodes a `u128` from a buffer, returning the value and encoded length.
	decode_u128, u128, 17
}

/// Maps a zigzag unsigned representation back to its signed value.
macro_rules! unzigzag {
	($it:ident, $zigzag:expr) => {
		((($zigzag >> 1) as $it) ^ (-(($zigzag & 1) as $it)))
	};
}

/// Generates the zigzag decoder for a signed type.
macro_rules! decode_signed {
	($(#[$docs:meta])* $name:ident, $it:ident, $decode_fn:ident, $size:expr) => {
		$(#[$docs])*
		#[inline]
		#[must_use]
		pub const fn $name(buf: &[u8; $size]) -> ($it, usize) {
			let (zigzag, len) = $decode_fn(buf);
			(unzigzag!($it, zigzag), len)
		}
	};
}

decode_signed! {
	/// Decodes an `i16` from a buffer, returning the value and encoded length.
	decode_i16, i16, decode_u16, 3
}

decode_signed! {
	/// Decodes an `i32` from a buffer, returning the value and encoded length.
	decode_i32, i32, decode_u32, 5
}

decode_signed! {
	/// Decodes an `i64` from a buffer, returning the value and encoded length.
	decode_i64, i64, decode_u64, 9
}

decode_signed! {
	/// Decodes an `i128` from a buffer, returning the value and encoded length.
	decode_i128, i128, decode_u128, 17
}

/// Generates the reverse-endian decoder for a floating-point type.
macro_rules! decode_float {
	($(#[$docs:meta])* $name:ident, $ft:ident, $decode_fn:ident, $size:expr) => {
		$(#[$docs])*
		#[inline]
		#[must_use]
		pub const fn $name(buf: &[u8; $size]) -> ($ft, usize) {
			let (swapped, len) = $decode_fn(buf);
			($ft::from_bits(swapped.swap_bytes()), len)
		}
	};
}

decode_float! {
	/// Decodes an `f32` from a buffer, returning the value and encoded length.
	decode_f32, f32, decode_u32, 5
}

decode_float! {
	/// Decodes an `f64` from a buffer, returning the value and encoded length.
	decode_f64, f64, decode_u64, 9
}

/// Decodes a value from a slice, returning the value and encoded length.
///
/// Unlike the array-based functions, this validates the input: the
/// slice only needs to hold the value's actual encoding, and invalid
/// prefixes or out-of-range values are rejected.
#[inline]
pub fn decode<T: Decode>(buf: &[u8]) -> Result<(T, usize)> {
	T::decode(buf)
}

/// Types that can be decoded using vlen.
pub trait Decode: Sized {
	/// The maximum possible encoded size for this type.
	const MAX_ENCODED_SIZE: usize;

	/// Decodes a value from the slice, returning it with its encoded
	/// length.
	///
	/// The slice only needs to hold the value's actual encoding.
	/// Fails with [`Error::BufferTooSmall`] on truncated input,
	/// [`Error::InvalidPrefix`] if the first byte announces an encoding
	/// longer than this type can produce, and [`Error::Overflow`] if
	/// the encoded value exceeds the type's range.
	fn decode(buf: &[u8]) -> Result<(Self, usize)>;
}

/// Validates the prefix and buffer length, then hands a full-size
/// array to `$decode_fn`. Zero-copy when the slice already holds
/// `MAX_ENCODED_SIZE` bytes.
macro_rules! checked_decode {
	($buf:ident, $size:expr, $decode_fn:ident) => {{
		let Some(&b0) = $buf.first() else {
			return Err(Error::BufferTooSmall {
				needed: 1,
				available: 0,
			});
		};
		let needed = encoded_len(b0);
		if needed > $size {
			return Err(Error::InvalidPrefix { prefix: b0 });
		}
		if let Some(arr) = $buf.first_chunk::<$size>() {
			Ok($decode_fn(arr))
		} else if $buf.len() >= needed {
			let mut tmp = [0u8; $size];
			tmp[..$buf.len()].copy_from_slice($buf);
			Ok($decode_fn(&tmp))
		} else {
			Err(Error::BufferTooSmall {
				needed,
				available: $buf.len(),
			})
		}
	}};
}

macro_rules! impl_decode {
	($t:ty, $size:expr, $decode_fn:ident) => {
		impl Decode for $t {
			const MAX_ENCODED_SIZE: usize = $size;

			#[inline]
			fn decode(buf: &[u8]) -> Result<(Self, usize)> {
				checked_decode!(buf, $size, $decode_fn)
			}
		}
	};
}

impl_decode!(u32, 5, decode_u32);
impl_decode!(u64, 9, decode_u64);
impl_decode!(u128, 17, decode_u128);

impl Decode for u16 {
	const MAX_ENCODED_SIZE: usize = 3;

	#[inline]
	fn decode(buf: &[u8]) -> Result<(Self, usize)> {
		// Decode through the u32 grammar so that three-byte encodings
		// carrying values above u16::MAX are rejected, not truncated.
		let (value, len): (u32, usize) =
			checked_decode!(buf, 3, decode_u16_wide)?;
		if value > u16::MAX as u32 {
			return Err(Error::Overflow);
		}
		Ok((value as u16, len))
	}
}

/// Decodes a u16-sized buffer through the u32 grammar, preserving
/// three-byte values above `u16::MAX` for range checking.
#[inline]
const fn decode_u16_wide(buf: &[u8; 3]) -> (u32, usize) {
	let b0 = buf[0];
	match b0 {
		_ if b0 < 0x80 => (b0 as u32, 1),
		_ if b0 < 0xC0 => (((buf[1] as u32) << 6) | ((b0 & 0x3F) as u32), 2),
		_ => {
			let value = ((buf[2] as u32) << 13)
				| ((buf[1] as u32) << 5)
				| ((b0 & 0x1F) as u32);
			(value, 3)
		},
	}
}

/// Implements [`Decode`] for a signed type on top of its unsigned
/// counterpart, inheriting all of its validation.
macro_rules! impl_decode_signed {
	($it:ident, $ut:ident, $size:expr) => {
		impl Decode for $it {
			const MAX_ENCODED_SIZE: usize = $size;

			#[inline]
			fn decode(buf: &[u8]) -> Result<(Self, usize)> {
				let (zigzag, len) = <$ut as Decode>::decode(buf)?;
				Ok((unzigzag!($it, zigzag), len))
			}
		}
	};
}

impl_decode_signed!(i16, u16, 3);
impl_decode_signed!(i32, u32, 5);
impl_decode_signed!(i64, u64, 9);
impl_decode_signed!(i128, u128, 17);

/// Implements [`Decode`] for a floating-point type on top of its
/// unsigned counterpart.
macro_rules! impl_decode_float {
	($ft:ident, $ut:ident, $size:expr) => {
		impl Decode for $ft {
			const MAX_ENCODED_SIZE: usize = $size;

			#[inline]
			fn decode(buf: &[u8]) -> Result<(Self, usize)> {
				let (swapped, len) = <$ut as Decode>::decode(buf)?;
				Ok(($ft::from_bits(swapped.swap_bytes()), len))
			}
		}
	};
}

impl_decode_float!(f32, u32, 5);
impl_decode_float!(f64, u64, 9);
