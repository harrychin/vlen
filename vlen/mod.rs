//! # vlen: High-performance variable-length numeric encoding
//!
//! `vlen` is an enhanced version of the original `vu128` variable-length
//! numeric encoding. Numeric types up to 128 bits are supported (integers
//! and floating-point), with smaller values being encoded using fewer
//! bytes. Every integer width shares one wire format, so a value encoded
//! as one type decodes as any wider type.
//!
//! The compression ratio of `vlen` equals or exceeds the widely used
//! [VLQ] and [LEB128] encodings, and it decodes faster on modern
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
pub mod decode;
pub mod encode;
mod error;
#[cfg(feature = "serde")]
pub mod serde;

pub use error::{Error, Result};

pub use decode::{
	Decode, decode, decode_f32, decode_f64, decode_i16, decode_i32, decode_i64,
	decode_i128, decode_u16, decode_u32, decode_u64, decode_u128,
};

pub use encode::{
	Encode, encode, encode_f32, encode_f64, encode_i16, encode_i32, encode_i64,
	encode_i128, encode_u16, encode_u32, encode_u64, encode_u128, encoded_len,
	encoded_size, encoded_size_u16, encoded_size_u32, encoded_size_u64,
	encoded_size_u128,
};

pub use bulk::{
	DecodeIter, bulk_decode, bulk_decode_u32, bulk_encode, bulk_encode_u32,
	decode_iter,
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
