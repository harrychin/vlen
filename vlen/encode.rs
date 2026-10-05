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
///
/// Prefix-varint first bytes announce their length through the count
/// of leading one bits; binary-length prefixes carry it in the low
/// nibble.
#[inline]
#[must_use]
pub const fn encoded_len(b: u8) -> usize {
	if b < 0xF0 {
		b.leading_ones() as usize + 1
	} else {
		((b & 0x0F) + 2) as usize
	}
}

/// Whether `encoding` uses the canonical prefix for its total length.
///
/// Binary prefixes `0xF0..=0xF2` overlap the two- through four-byte
/// prefix-varint forms. Canonical encoders use the prefix-varint form there;
/// binary prefixes are canonical only from five bytes onward.
#[inline(always)]
fn has_canonical_prefix(encoding: &[u8]) -> bool {
	encoding.len() >= 5 || encoding.first().is_some_and(|&first| first < 0xF0)
}

/// Calculates the encoded size of a `u8` value without encoding it.
#[inline]
#[must_use]
pub const fn encoded_size_u8(value: u8) -> usize {
	if value < 0x80 { 1 } else { 2 }
}

/// Calculates the encoded size of a `u16` value without encoding it.
///
/// Branch-free: seven value bits fit per encoded byte, so the size
/// falls out of the bit width directly.
#[inline]
#[must_use]
pub const fn encoded_size_u16(value: u16) -> usize {
	(38 - (value as u32 | 1).leading_zeros() as usize) / 7
}

/// Calculates the encoded size of a `u32` value without encoding it.
#[inline]
#[must_use]
pub const fn encoded_size_u32(value: u32) -> usize {
	if value < 0x10000000 {
		// Branch-free within the prefix-varint range: seven value
		// bits fit per encoded byte.
		(38 - (value | 1).leading_zeros() as usize) / 7
	} else {
		5
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

/// Encodes a `u8` into a buffer, returning the encoded length.
#[inline]
#[must_use]
pub const fn encode_u8(buf: &mut [u8; 2], value: u8) -> usize {
	if value < 0x80 {
		buf[0] = value;
		1
	} else {
		buf[0] = 0x80 | (value & 0x3F);
		buf[1] = value >> 6;
		2
	}
}

/// Encodes an `i8` into a buffer, returning the encoded length.
#[inline]
#[must_use]
pub const fn encode_i8(buf: &mut [u8; 2], value: i8) -> usize {
	encode_u8(buf, ((value >> 7) as u8) ^ ((value << 1) as u8))
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
			// Each prefix-varint form is built as one little-endian
			// word and stored whole (trailing bytes are scratch), so
			// every arm is a single computation and a single store.
			match value {
				_ if value < 0x80 => {
					buf[0] = value as u8;
					1
				},
				_ if value < 0x4000 => {
					let word = 0x80
						| ((value & 0x3F) as u16)
						| (((value >> 6) as u16) << 8);
					let b = word.to_le_bytes();
					buf[0] = b[0];
					buf[1] = b[1];
					2
				},
				_ if value < 0x200000 => {
					let word = 0xC0
						| ((value & 0x1F) as u32)
						| ((((value >> 5) as u32) & 0xFF) << 8)
						| (((value >> 13) as u32) << 16);
					let b = word.to_le_bytes();
					buf[0] = b[0];
					buf[1] = b[1];
					buf[2] = b[2];
					buf[3] = b[3];
					3
				},
				_ if value < 0x10000000 => {
					let word = 0xE0
						| ((value & 0x0F) as u32)
						| (((value >> 4) as u32) << 8);
					let b = word.to_le_bytes();
					buf[0] = b[0];
					buf[1] = b[1];
					buf[2] = b[2];
					buf[3] = b[3];
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
///
/// When `buf` is longer than the encoding, bytes past the returned
/// length may be overwritten with scratch data (the fast path stores
/// whole words). To patch a value into a larger buffer without touching
/// what follows it, pass exactly its slot:
///
/// ```rust
/// let mut frame = [0u8; 9];
/// frame[3..].copy_from_slice(b"abcdef");
/// let len = vlen::encoded_size(20_000u32); // 3 bytes
/// vlen::encode(&mut frame[..len], 20_000u32)?;
/// assert_eq!(&frame[3..], b"abcdef");
/// # Ok::<(), vlen::Error>(())
/// ```
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

/// Encodes a value into exactly `N` bytes, padding it to an over-long
/// encoding when its canonical form is shorter.
///
/// This fills a slot reserved before its value was known, such as a
/// length prefix written after its payload. Every decoder for `T`
/// accepts the result and reports `N` bytes consumed, while
/// [`decode_canonical`](crate::decode_canonical) and
/// [`decode_strict`](crate::decode_strict) reject it unless `N` is the
/// canonical length (in which case the bytes are exactly the canonical
/// encoding). [`Writer::reserve`](crate::Writer::reserve) and
/// [`Writer::fill`](crate::Writer::fill) manage such slots in a cursor.
///
/// Fails with [`Error::BufferTooSmall`] when the canonical encoding
/// needs more than `N` bytes. `N` must be between 1 and
/// `T::MAX_ENCODED_SIZE`, which is checked at compile time: a wider
/// slot would not decode as `T`.
///
/// The padding re-expresses `value`'s canonical encoding in the vlen
/// wire grammar, which every built-in type uses. A downstream [`Encode`]
/// implementation gets correct results when it encodes through a
/// built-in type.
///
/// ```rust
/// let mut msg = [0u8; 9];
/// // A four-byte length slot, then the payload it describes.
/// msg[4..].copy_from_slice(b"hello");
/// let (slot, _) = msg.split_first_chunk_mut::<4>().unwrap();
/// vlen::encode_padded(slot, 5u32)?;
///
/// assert_eq!(vlen::decode::<u32>(&msg)?, (5, 4));
/// # Ok::<(), vlen::Error>(())
/// ```
///
/// A slot wider than the type can decode does not compile:
///
/// ```rust,compile_fail,E0080
/// let mut slot = [0u8; 6];
/// vlen::encode_padded(&mut slot, 1u32).unwrap(); // u32 decodes at most 5
/// ```
pub fn encode_padded<T: Encode, const N: usize>(
	buf: &mut [u8; N],
	value: T,
) -> Result<()> {
	const {
		assert!(
			N >= 1 && N <= T::MAX_ENCODED_SIZE && N <= 17,
			"padded width must be between 1 and the type's MAX_ENCODED_SIZE"
		);
	}
	let needed = value.encoded_size();
	if needed > N {
		return Err(Error::BufferTooSmall {
			needed,
			available: N,
		});
	}
	// Every width shares one grammar, so the canonical bytes decode to
	// the value's wire representation at the widest type.
	let mut canonical = [0u8; 17];
	let len = value.encode(&mut canonical)?;
	debug_assert_eq!(encoded_len(canonical[0]), len);
	let (wire, _) = crate::decode::decode_u128(&canonical);

	let mut out = [0u8; 17];
	if N <= 4 {
		// Prefix varint: N - 1 leading one bits, a zero, then the value
		// zero-extended to 7 * N bits.
		let low_bits = 8 - N as u32;
		let prefix = !(0xFFu32 >> (N - 1)) & 0xFF;
		let word = prefix
			| (wire as u32 & ((1 << low_bits) - 1))
			| (((wire >> low_bits) as u32) << 8);
		out[..4].copy_from_slice(&word.to_le_bytes());
	} else {
		// Binary length prefix with N - 1 zero-extended payload bytes.
		out[0] = 0xF0 | (N - 2) as u8;
		out[1..].copy_from_slice(&wire.to_le_bytes());
	}
	buf.copy_from_slice(&out[..N]);
	Ok(())
}

/// Types that can be encoded using vlen.
///
/// # Implementation contract
///
/// Downstream implementations must report an exact, nonzero
/// [`encoded_size`](Encode::encoded_size) no greater than
/// [`MAX_ENCODED_SIZE`](Encode::MAX_ENCODED_SIZE). Encoding into a slice at
/// least that long must succeed and return that same size; shorter slices must
/// return [`Error::BufferTooSmall`]. Generic allocation and bulk helpers rely
/// on these guarantees.
///
/// Only the first `returned size` bytes are part of the encoding. An
/// implementation may use later bytes in a larger destination as scratch.
/// If [`Decode`](crate::Decode) accepts multiple same-length representations
/// of one value, the implementation must also override
/// [`is_canonical_encoding`](Encode::is_canonical_encoding).
pub trait Encode: Copy {
	/// The nonzero maximum possible encoded size for this type.
	const MAX_ENCODED_SIZE: usize;

	/// Calculates the exact encoded size of the value without encoding it.
	#[must_use]
	fn encoded_size(self) -> usize;

	/// Encodes the value into the slice, returning the encoded length.
	///
	/// The slice only needs room for the value's actual encoded size.
	/// Fails with [`Error::BufferTooSmall`] otherwise. Bytes after the returned
	/// length are unspecified.
	fn encode(self, buf: &mut [u8]) -> Result<usize>;

	/// Whether `encoding`, after decoding to this value, uses canonical form.
	///
	/// The default accepts encodings whose length matches [`encoded_size`].
	/// Implementations whose [`Decode`](crate::Decode) accepts multiple
	/// same-length representations of a value must override this method so
	/// [`decode_canonical`](crate::decode_canonical) can distinguish them.
	#[must_use]
	fn is_canonical_encoding(self, encoding: &[u8]) -> bool {
		encoding.len() == self.encoded_size()
	}
}

/// Implements [`Encode`] on top of an array-based encoder plus a size
/// expression evaluated with the value bound to `$v`. The
/// shorter-than-maximum buffer case lives in a cold out-of-line
/// function so the hot path inlined into callers stays small.
macro_rules! impl_encode {
	($t:ty, $size:expr, $encode_fn:ident, $short_fn:ident,
		$v:ident => $size_expr:expr) => {
		#[cold]
		#[inline(never)]
		fn $short_fn(value: $t, buf: &mut [u8]) -> Result<usize> {
			let mut tmp = [0u8; $size];
			let len = $encode_fn(&mut tmp, value);
			match buf.get_mut(..len) {
				Some(dst) => {
					// Byte loop rather than copy_from_slice: see the
					// matching note in the decode short path.
					for (d, &s) in dst.iter_mut().zip(&tmp) {
						*d = s;
					}
					Ok(len)
				},
				None => Err(Error::BufferTooSmall {
					needed: len,
					available: buf.len(),
				}),
			}
		}

		impl Encode for $t {
			const MAX_ENCODED_SIZE: usize = $size;

			#[inline]
			fn encoded_size(self) -> usize {
				let $v = self;
				$size_expr
			}

			// inline(always): encode loops live or die by this being
			// merged into the caller's loop body.
			#[inline(always)]
			fn encode(self, buf: &mut [u8]) -> Result<usize> {
				if let Some(arr) = buf.first_chunk_mut::<$size>() {
					Ok($encode_fn(arr, self))
				} else {
					$short_fn(self, buf)
				}
			}

			#[inline]
			fn is_canonical_encoding(self, encoding: &[u8]) -> bool {
				encoding.len() == self.encoded_size()
					&& has_canonical_prefix(encoding)
			}
		}
	};
}

impl_encode!(u16, 3, encode_u16, encode_u16_short,
	v => encoded_size_u16(v));
impl_encode!(u32, 5, encode_u32, encode_u32_short,
	v => encoded_size_u32(v));
impl_encode!(u64, 9, encode_u64, encode_u64_short,
	v => encoded_size_u64(v));
impl_encode!(u128, 17, encode_u128, encode_u128_short,
	v => encoded_size_u128(v));

impl_encode!(i16, 3, encode_i16, encode_i16_short,
	v => encoded_size_u16(zigzag!(i16, u16, v)));
impl_encode!(i32, 5, encode_i32, encode_i32_short,
	v => encoded_size_u32(zigzag!(i32, u32, v)));
impl_encode!(i64, 9, encode_i64, encode_i64_short,
	v => encoded_size_u64(zigzag!(i64, u64, v)));
impl_encode!(
	i128, 17, encode_i128, encode_i128_short,
	v => encoded_size_u128(zigzag!(i128, u128, v))
);

impl_encode!(u8, 2, encode_u8, encode_u8_short,
	v => encoded_size_u8(v));
impl_encode!(i8, 2, encode_i8, encode_i8_short,
	v => encoded_size_u8(zigzag!(i8, u8, v)));

/// `usize` encodes through the `u64` grammar, so the wire format is
/// identical on every platform.
impl Encode for usize {
	const MAX_ENCODED_SIZE: usize = 9;

	#[inline]
	fn encoded_size(self) -> usize {
		encoded_size_u64(self as u64)
	}

	#[inline(always)]
	fn encode(self, buf: &mut [u8]) -> Result<usize> {
		(self as u64).encode(buf)
	}

	#[inline]
	fn is_canonical_encoding(self, encoding: &[u8]) -> bool {
		(self as u64).is_canonical_encoding(encoding)
	}
}

/// `isize` encodes through the `i64` grammar, so the wire format is
/// identical on every platform.
impl Encode for isize {
	const MAX_ENCODED_SIZE: usize = 9;

	#[inline]
	fn encoded_size(self) -> usize {
		encoded_size_u64(zigzag!(i64, u64, self as i64))
	}

	#[inline(always)]
	fn encode(self, buf: &mut [u8]) -> Result<usize> {
		(self as i64).encode(buf)
	}

	#[inline]
	fn is_canonical_encoding(self, encoding: &[u8]) -> bool {
		(self as i64).is_canonical_encoding(encoding)
	}
}

impl_encode!(f32, 5, encode_f32, encode_f32_short, v => {
	encoded_size_u32(v.to_bits().swap_bytes())
});
impl_encode!(f64, 9, encode_f64, encode_f64_short, v => {
	encoded_size_u64(v.to_bits().swap_bytes())
});
