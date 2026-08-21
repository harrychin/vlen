//! # vlen: High-performance variable-length numeric encoding
//!
//! `vlen` is an enhanced version of the original `vu128` variable-length
//! numeric encoding. Numeric types up to 128 bits are supported (integers
//! and floating-point), with smaller values being encoded using fewer
//! bytes. Every integer width shares one wire format, so a value encoded
//! as one type decodes as any wider type.
//!
//! The compression matches the widely used [VLQ] and [LEB128]
//! encodings for values below `2^28` (and caps at 9 bytes for `u64`,
//! where LEB128 needs up to 10), and it decodes faster on modern
//! pipelined architectures because the encoded length is announced by
//! the first byte instead of continuation bits spread across the value.
//!
//! [VLQ]: https://en.wikipedia.org/wiki/Variable-length_quantity
//! [LEB128]: https://en.wikipedia.org/wiki/LEB128
//!
//! ## Quick Start
//!
//! ```rust
//! use vlen::{Decode, Encode};
//!
//! let mut buf = [0u8; 5];
//! let value = 12345u32;
//!
//! let len = value.encode(&mut buf)?;
//! assert_eq!(len, value.encoded_size());
//!
//! let (decoded, decoded_len) = u32::decode(&buf[..len])?;
//! assert_eq!(decoded, value);
//! assert_eq!(decoded_len, len);
//! # Ok::<(), vlen::Error>(())
//! ```
//!
//! ## Two API layers
//!
//! - The [`Encode`] and [`Decode`] traits (and the free [`encode()`],
//!   [`decode()`], and [`bulk_encode`]/[`bulk_decode`] functions) work on
//!   ordinary slices, validate their input, and return typed
//!   [`Error`]s. Use these for untrusted or exactly-sized data.
//! - The array-based functions ([`encode_u32`], [`decode_u32`], and
//!   friends) are the infallible fast core. They are all `const fn`,
//!   so they also work in compile-time contexts:
//!
//! ```rust
//! const LEN: usize = {
//!     let mut buf = [0u8; 5];
//!     vlen::encode_u32(&mut buf, 12345)
//! };
//! assert_eq!(LEN, 2);
//! ```

#![cfg_attr(not(test), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod bulk;
mod cursor;
pub mod decode;
pub mod encode;
mod error;
#[cfg(all(
	feature = "simd",
	any(
		target_arch = "aarch64",
		target_arch = "x86_64",
		all(target_arch = "wasm32", target_feature = "simd128")
	)
))]
#[allow(unsafe_code)]
mod kernels;
#[cfg(feature = "serde")]
pub mod serde;

pub use cursor::{Reader, Writer};
pub use error::{Error, Result};

pub use decode::{
	Decode, decode, decode_f32, decode_f64, decode_i8, decode_i16, decode_i32,
	decode_i64, decode_i128, decode_u8, decode_u16, decode_u32, decode_u64,
	decode_u128,
};

pub use encode::{
	Encode, encode, encode_f32, encode_f64, encode_i8, encode_i16, encode_i32,
	encode_i64, encode_i128, encode_u8, encode_u16, encode_u32, encode_u64,
	encode_u128, encoded_len, encoded_size, encoded_size_u8, encoded_size_u16,
	encoded_size_u32, encoded_size_u64, encoded_size_u128,
};

pub use bulk::{
	DecodeIter, DecodeIterI32, DecodeIterI64, DecodeIterU32, DecodeIterU64,
	bulk_decode, bulk_decode_i32, bulk_decode_i64, bulk_decode_u32,
	bulk_decode_u64, bulk_encode, bulk_encode_i32, bulk_encode_i64,
	bulk_encode_u32, bulk_encode_u64, decode_iter, decode_iter_i32,
	decode_iter_i64, decode_iter_u32, decode_iter_u64,
};

/// Decodes a single value from a slice, discarding the length.
///
/// Trailing bytes after the first value are ignored.
#[inline]
pub fn decode_value<T: Decode>(buf: &[u8]) -> Result<T> {
	let (value, _) = T::decode(buf)?;
	Ok(value)
}

/// Encodes a value into a newly allocated buffer.
#[cfg(feature = "alloc")]
#[must_use]
pub fn encode_to_vec<T: Encode>(value: T) -> alloc::vec::Vec<u8> {
	let mut buf = alloc::vec![0u8; value.encoded_size()];
	let len = value
		.encode(&mut buf)
		.expect("buffer sized by encoded_size");
	debug_assert_eq!(len, buf.len());
	buf
}

/// Appends the encoding of `value` to a byte vector.
#[cfg(feature = "alloc")]
pub fn encode_append<T: Encode>(buf: &mut alloc::vec::Vec<u8>, value: T) {
	let mut tmp = [0u8; 17];
	let len = value
		.encode(&mut tmp)
		.expect("seventeen bytes fit any encoding");
	buf.extend_from_slice(&tmp[..len]);
}

/// Appends the encodings of all `values` to a byte vector.
#[cfg(feature = "alloc")]
pub fn bulk_encode_append<T: Encode>(
	buf: &mut alloc::vec::Vec<u8>,
	values: &[T],
) {
	let total: usize = values.iter().map(|v| v.encoded_size()).sum();
	let start = buf.len();
	buf.resize(start + total, 0);
	let len = bulk_encode(&mut buf[start..], values)
		.expect("buffer sized by encoded_size");
	debug_assert_eq!(len, total);
}

/// Encodes a slice of values into a newly allocated buffer.
#[cfg(feature = "alloc")]
#[must_use]
pub fn bulk_encode_to_vec<T: Encode>(values: &[T]) -> alloc::vec::Vec<u8> {
	let total = values.iter().map(|v| v.encoded_size()).sum();
	let mut buf = alloc::vec![0u8; total];
	let len =
		bulk_encode(&mut buf, values).expect("buffer sized by encoded_size");
	debug_assert_eq!(len, total);
	buf
}

/// Decodes every value in a slice into a newly allocated vector.
///
/// The buffer must contain a whole number of valid encodings.
#[cfg(feature = "alloc")]
pub fn bulk_decode_values<T: Decode>(buf: &[u8]) -> Result<alloc::vec::Vec<T>> {
	decode_iter(buf).collect()
}
