//! # vlen: High-performance variable-length numeric encoding
//!
//! `vlen` is an enhanced version of the original `vu128` variable-length numeric encoding, featuring SIMD optimizations, improved performance, and enhanced functionality. Numeric types up to 128 bits are supported (integers and floating-point), with smaller values being encoded using fewer bytes.
//!
//! The compression ratio of `vlen` equals or exceeds the widely used [VLQ] and [LEB128] encodings, and is significantly faster on modern pipelined architectures thanks to SIMD optimizations and algorithmic improvements.
//!
//! [VLQ]: https://en.wikipedia.org/wiki/Variable-length_quantity
//! [LEB128]: https://en.wikipedia.org/wiki/LEB128
//!
//! ## Quick Start
//!
//! ```rust
//! use vlen::{encode, decode, encoded_size};
//!
//! // Simple encoding and decoding
//! let mut buf = [0u8; 17];
//! let value = 12345u32;
//!
//! let encoded_len = encode(&mut buf, value).unwrap();
//! let (decoded_value, decoded_len) = decode::<u32>(&buf).unwrap();
//!
//! assert_eq!(value, decoded_value);
//! assert_eq!(encoded_len, decoded_len);
//!
//! // Calculate size without encoding
//! let size = encoded_size(value).unwrap();
//! assert_eq!(size, encoded_len);
//! ```
//!

#![cfg_attr(not(test), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod decode;
pub mod encode;
mod helpers;
#[cfg(feature = "serde")]
pub mod serde;
#[cfg(feature = "simd")]
pub mod simd;

// Export specific functions from decode module
pub use decode::{
	bulk_decode,
	decode,
	decode_f32,
	decode_f64,
	decode_i128,
	decode_i16,
	decode_i32,
	decode_i64,
	decode_u128,
	decode_u16,
	decode_u32,
	decode_u64,
	Decode,
};

// Export specific functions from encode module
pub use encode::{
	bulk_encode,
	encode,
	encode_f32,
	encode_f64,
	encode_i128,
	encode_i16,
	encode_i32,
	encode_i64,
	encode_u128,
	encode_u16,
	encode_u32,
	encode_u64,
	encoded_len,
	encoded_size,
	encoded_size_u128,
	encoded_size_u16,
	encoded_size_u32,
	encoded_size_u64,
	Encode,
};

// Export SIMD-specific functions with unique names to avoid conflicts
#[cfg(feature = "simd")]
pub use simd::{bulk_decode_u32_safe, bulk_encode_u32_safe};

// Re-export the unsafe SIMD functions with unique names
#[cfg(all(
	feature = "simd",
	any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub use simd::{bulk_decode_u32, bulk_encode_u32};

/// Convenience function to encode a value into a newly allocated buffer.
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
#[cfg(feature = "alloc")]
pub fn encode_to_vec<T>(value: T) -> Result<alloc::vec::Vec<u8>, &'static str>
where
	T: encode::Encode + Copy,
{
	let max_size = T::MAX_ENCODED_SIZE;
	let mut buf = alloc::vec![0u8; max_size];
	let encoded_len = T::encode(&mut buf, value)?;
	buf.truncate(encoded_len);
	Ok(buf)
}

/// Convenience function to decode a value from a slice.
pub fn decode_value<T>(buf: &[u8]) -> Result<T, &'static str>
where
	T: decode::Decode,
{
	let (value, _) = T::decode(buf)?;
	Ok(value)
}

/// Convenience function to encode multiple values into a newly allocated buffer.
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
#[cfg(feature = "alloc")]
pub fn bulk_encode_to_vec<T>(
	values: &[T],
) -> Result<alloc::vec::Vec<u8>, &'static str>
where
	T: encode::Encode + Copy,
{
	let max_size_per_value = T::MAX_ENCODED_SIZE;
	let mut buf = alloc::vec![0u8; values.len() * max_size_per_value];
	let encoded_len = bulk_encode(&mut buf, values)?;
	buf.truncate(encoded_len);
	Ok(buf)
}

/// Convenience function to decode multiple values from a slice.
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
#[cfg(feature = "alloc")]
pub fn bulk_decode_values<T>(
	buf: &[u8],
) -> Result<alloc::vec::Vec<T>, &'static str>
where
	T: decode::Decode,
{
	// Estimate capacity: assume average encoding is half of max size
	let estimated_count = buf.len() / (T::MAX_ENCODED_SIZE / 2).max(1);
	let mut values = alloc::vec::Vec::with_capacity(estimated_count);
	let mut offset = 0;

	while offset < buf.len() {
		let (value, len) = T::decode(&buf[offset..])?;
		values.push(value);
		offset += len;
	}
	Ok(values)
}
