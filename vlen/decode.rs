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

use crate::encode::{Encode, encoded_len};
use crate::error::{Error, Result, StrictError, StrictResult};

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

/// Decodes a `u8` from a buffer, returning the value and encoded length.
///
/// Two-byte encodings can carry values up to `2^14 - 1`; anything
/// above `u8::MAX` is truncated. Use [`Decode`] to reject such input.
#[inline]
#[must_use]
pub const fn decode_u8(buf: &[u8; 2]) -> (u8, usize) {
	let b0 = buf[0];
	if b0 < 0x80 {
		(b0, 1)
	} else if b0 < 0xC0 {
		((((buf[1] as u16) << 6) | ((b0 & 0x3F) as u16)) as u8, 2)
	} else {
		decode_u8_high_prefix(buf)
	}
}

/// Keeps the uncommon binary-prefix form and invalid-prefix fallback out of
/// the array decoder's prefix-varint hot path.
#[cold]
#[inline(never)]
const fn decode_u8_high_prefix(buf: &[u8; 2]) -> (u8, usize) {
	if buf[0] == 0xF0 {
		(buf[1], 2)
	} else {
		// Invalid for u8; preserve the array decoder's bounded result.
		((((buf[1] as u16) << 6) | ((buf[0] & 0x3F) as u16)) as u8, 2)
	}
}

/// Decodes an `i8` from a buffer, returning the value and encoded length.
#[inline]
#[must_use]
pub const fn decode_i8(buf: &[u8; 2]) -> (i8, usize) {
	let (zigzag, len) = decode_u8(buf);
	(((zigzag >> 1) as i8) ^ (-((zigzag & 1) as i8)), len)
}

/// Decodes a `u16` from a buffer, returning the value and encoded length.
///
/// Three-byte encodings can carry values up to `2^21 - 1`; anything
/// above `u16::MAX` is truncated. Use [`Decode`] to reject such input.
#[inline]
#[must_use]
pub const fn decode_u16(buf: &[u8; 3]) -> (u16, usize) {
	let b0 = buf[0];
	if b0 < 0x80 {
		return (b0 as u16, 1);
	}
	if b0 < 0xC0 {
		return (((buf[1] as u16) << 6) | ((b0 & 0x3F) as u16), 2);
	}
	if b0 >= 0xE0 {
		return decode_u16_high_prefix(buf);
	}
	let wide =
		((buf[2] as u32) << 13) | ((buf[1] as u32) << 5) | ((b0 & 0x1F) as u32);
	(wide as u16, 3)
}

/// Keeps binary-prefix forms and invalid-prefix fallback out of the array
/// decoder's prefix-varint hot path.
#[cold]
#[inline(never)]
const fn decode_u16_high_prefix(buf: &[u8; 3]) -> (u16, usize) {
	match buf[0] {
		0xF0 => (buf[1] as u16, 2),
		0xF1 => (u16::from_le_bytes([buf[1], buf[2]]), 3),
		b0 => {
			// Invalid for u16; preserve the array decoder's bounded result.
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

/// Decodes exactly one value occupying the entire slice.
///
/// Unlike [`decode`], this rejects trailing bytes. It retains the normal
/// decoder's permissive handling of over-long encodings; use [`decode_strict`]
/// when the input must also be canonical.
#[inline]
pub fn decode_exact<T: Decode>(buf: &[u8]) -> StrictResult<T> {
	let (value, consumed) = T::decode(buf)?;
	if consumed == buf.len() {
		Ok(value)
	} else {
		Err(StrictError::TrailingBytes {
			consumed,
			available: buf.len(),
		})
	}
}

/// Decodes one value and requires its canonical byte representation.
///
/// As with [`decode`], trailing bytes after the first value are ignored and
/// the consumed length is returned. Use [`decode_strict`] to require the value
/// to occupy the entire slice as well.
#[inline]
pub fn decode_canonical<T: Decode + Encode>(
	buf: &[u8],
) -> StrictResult<(T, usize)> {
	let (value, encoded_len) = T::decode(buf)?;
	if value.is_canonical_encoding(&buf[..encoded_len]) {
		Ok((value, encoded_len))
	} else {
		Err(StrictError::NonCanonical {
			encoded_len,
			canonical_len: value.encoded_size(),
		})
	}
}

/// Decodes one canonical value occupying the entire slice.
///
/// Canonicality is checked before trailing bytes, so an over-long first value
/// reports [`StrictError::NonCanonical`] even if more data follows it.
#[inline]
pub fn decode_strict<T: Decode + Encode>(buf: &[u8]) -> StrictResult<T> {
	let (value, consumed) = decode_canonical(buf)?;
	if consumed == buf.len() {
		Ok(value)
	} else {
		Err(StrictError::TrailingBytes {
			consumed,
			available: buf.len(),
		})
	}
}

/// Types that can be decoded using vlen.
///
/// # Implementation contract
///
/// [`MAX_ENCODED_SIZE`](Decode::MAX_ENCODED_SIZE) must be nonzero. A
/// successful decode must consume at least one byte, no more than the supplied
/// slice, and no more than `MAX_ENCODED_SIZE`. Sequential and bulk helpers rely
/// on these guarantees to make forward progress and keep their offsets in
/// bounds.
pub trait Decode: Sized {
	/// The nonzero maximum possible encoded size for this type.
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

/// Generates the out-of-line path for slices shorter than the type's
/// maximum encoding: validate the prefix, copy into a zero-padded
/// full-size array, decode. Cold — the hot path never comes here.
macro_rules! decode_short_fn {
	($name:ident, $t:ty, $size:expr, $decode_fn:ident) => {
		#[cold]
		#[inline(never)]
		fn $name(buf: &[u8]) -> Result<($t, usize)> {
			let Some(&b0) = buf.first() else {
				return Err(Error::BufferTooSmall {
					needed: 1,
					available: 0,
				});
			};
			let needed = encoded_len(b0);
			if needed > $size {
				return Err(Error::InvalidPrefix { prefix: b0 });
			}
			if buf.len() >= needed {
				let mut tmp = [0u8; $size];
				// Byte loop rather than copy_from_slice: a variable-
				// length copy links compiler_builtins memcpy, which
				// dwarfs this crate in minimal embedded builds.
				for (dst, &src) in tmp.iter_mut().zip(buf) {
					*dst = src;
				}
				Ok($decode_fn(&tmp))
			} else {
				Err(Error::BufferTooSmall {
					needed,
					available: buf.len(),
				})
			}
		}
	};
}

/// Implements [`Decode`]. When the slice holds a full-size window,
/// the only invalid input is a binary length prefix announcing more
/// bytes than the type can use — a single compare against
/// `$max_prefix` (omitted for u128, whose every prefix is valid).
macro_rules! impl_decode {
	($t:ty, $size:expr, $decode_fn:ident, $short_fn:ident
		$(, $max_prefix:literal)?) => {
		decode_short_fn!($short_fn, $t, $size, $decode_fn);

		impl Decode for $t {
			const MAX_ENCODED_SIZE: usize = $size;

			// inline(always): decode loops live or die by this being
			// merged into the caller's loop body.
			#[inline(always)]
			fn decode(buf: &[u8]) -> Result<(Self, usize)> {
				if let Some(arr) = buf.first_chunk::<$size>() {
					$(
						if arr[0] > $max_prefix {
							return Err(Error::InvalidPrefix {
								prefix: arr[0],
							});
						}
					)?
					Ok($decode_fn(arr))
				} else {
					$short_fn(buf)
				}
			}
		}
	};
}

impl_decode!(u32, 5, decode_u32, decode_u32_short, 0xF3);
impl_decode!(u64, 9, decode_u64, decode_u64_short, 0xF7);
impl_decode!(u128, 17, decode_u128, decode_u128_short);

decode_short_fn!(decode_u16_short, u32, 3, decode_u16_wide);

impl Decode for u16 {
	const MAX_ENCODED_SIZE: usize = 3;

	#[inline(always)]
	fn decode(buf: &[u8]) -> Result<(Self, usize)> {
		// Decode through the u32 grammar so that three-byte encodings
		// carrying values above u16::MAX are rejected, not truncated.
		let (value, len) = if let Some(arr) = buf.first_chunk::<3>() {
			if arr[0] >= 0xE0 {
				return decode_u16_high_checked(arr);
			}
			decode_u16_wide(arr)
		} else {
			decode_u16_short(buf)?
		};
		if value > u16::MAX as u32 {
			return Err(Error::Overflow);
		}
		Ok((value as u16, len))
	}
}

decode_short_fn!(decode_u8_short, u16, 2, decode_u8_wide);

impl Decode for u8 {
	const MAX_ENCODED_SIZE: usize = 2;

	#[inline(always)]
	fn decode(buf: &[u8]) -> Result<(Self, usize)> {
		// Decode through the u16 grammar so that two-byte encodings
		// carrying values above u8::MAX are rejected, not truncated.
		let (value, len) = if let Some(arr) = buf.first_chunk::<2>() {
			if arr[0] >= 0xC0 {
				return decode_u8_high_checked(arr);
			}
			decode_u8_wide(arr)
		} else {
			decode_u8_short(buf)?
		};
		if value > u8::MAX as u16 {
			return Err(Error::Overflow);
		}
		Ok((value as u8, len))
	}
}

#[cold]
#[inline(never)]
fn decode_u8_high_checked(buf: &[u8; 2]) -> Result<(u8, usize)> {
	if buf[0] == 0xF0 {
		Ok((buf[1], 2))
	} else {
		Err(Error::InvalidPrefix { prefix: buf[0] })
	}
}

#[cold]
#[inline(never)]
fn decode_u16_high_checked(buf: &[u8; 3]) -> Result<(u16, usize)> {
	match buf[0] {
		0xF0 => Ok((buf[1] as u16, 2)),
		0xF1 => Ok((u16::from_le_bytes([buf[1], buf[2]]), 3)),
		prefix => Err(Error::InvalidPrefix { prefix }),
	}
}

/// Decodes a u8-sized buffer through the u16 grammar, preserving
/// two-byte values above `u8::MAX` for range checking. The caller has
/// already rejected prefixes longer than two bytes.
#[inline]
const fn decode_u8_wide(buf: &[u8; 2]) -> (u16, usize) {
	let b0 = buf[0];
	if b0 < 0x80 {
		(b0 as u16, 1)
	} else if b0 < 0xC0 {
		(((buf[1] as u16) << 6) | ((b0 & 0x3F) as u16), 2)
	} else {
		// The caller only permits the two-byte binary prefix, 0xF0.
		(buf[1] as u16, 2)
	}
}

/// `usize` decodes through the `u64` grammar (the wire format is
/// platform-independent); values that do not fit the platform's
/// pointer width fail with [`Error::Overflow`].
impl Decode for usize {
	const MAX_ENCODED_SIZE: usize = 9;

	#[inline(always)]
	fn decode(buf: &[u8]) -> Result<(Self, usize)> {
		let (value, len) = u64::decode(buf)?;
		match usize::try_from(value) {
			Ok(value) => Ok((value, len)),
			Err(_) => Err(Error::Overflow),
		}
	}
}

/// `isize` decodes through the `i64` grammar (the wire format is
/// platform-independent); values that do not fit the platform's
/// pointer width fail with [`Error::Overflow`].
impl Decode for isize {
	const MAX_ENCODED_SIZE: usize = 9;

	#[inline(always)]
	fn decode(buf: &[u8]) -> Result<(Self, usize)> {
		let (value, len) = i64::decode(buf)?;
		match isize::try_from(value) {
			Ok(value) => Ok((value, len)),
			Err(_) => Err(Error::Overflow),
		}
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
		_ if b0 < 0xE0 => {
			let value = ((buf[2] as u32) << 13)
				| ((buf[1] as u32) << 5)
				| ((b0 & 0x1F) as u32);
			(value, 3)
		},
		_ => {
			// The caller only permits 0xF0 and 0xF1 here.
			let payload = (b0 & 0x0F) as usize + 1;
			let value = (buf[1] as u32) | ((buf[2] as u32) << 8);
			let mask = if payload == 1 { 0xFF } else { 0xFFFF };
			(value & mask, payload + 1)
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

impl_decode_signed!(i8, u8, 2);
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
