//! SIMD-accelerated bulk encoding for vlen

use crate::decode::decode_u32;
use crate::encode::encode_u32;

/// Trait that all SIMD implementations must implement
/// This ensures consistency across different architectures
pub trait SimdImpl {
	/// Bulk encode u32 values using SIMD optimizations
	///
	/// # Safety
	///
	/// - The buffer must be large enough to hold all encoded values
	/// - The buffer size should be at least `values.len() * 5` bytes
	/// - The caller must ensure the buffer is valid for the duration of the operation
	unsafe fn bulk_encode_u32(buf: &mut [u8], values: &[u32]) -> usize;

	/// Bulk decode u32 values using SIMD optimizations
	///
	/// # Safety
	///
	/// - The buffer must contain valid encoded data
	/// - The values array must be large enough to hold all decoded values
	/// - The caller must ensure the buffer is valid for the duration of the operation
	unsafe fn bulk_decode_u32(buf: &[u8], values: &mut [u32]) -> usize;
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#[inline]
fn handle_remaining_encode(
	buf: &mut [u8],
	values: &[u32],
	mut offset: usize,
	i: usize,
) -> usize {
	for &value in values[i..].iter() {
		unsafe {
			let buf_ptr = buf.as_mut_ptr().add(offset) as *mut [u8; 5];
			offset += encode_u32(&mut *buf_ptr, value);
		}
	}
	offset
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#[inline]
fn handle_remaining_decode(
	buf: &[u8],
	values: &mut [u32],
	mut offset: usize,
	mut i: usize,
) -> usize {
	while i < values.len() && offset < buf.len() {
		let mut temp_buf = [0u8; 5];
		let copy_len = core::cmp::min(5, buf.len() - offset);
		temp_buf[..copy_len].copy_from_slice(&buf[offset..offset + copy_len]);
		let (value, len) = decode_u32(&temp_buf);
		values[i] = value;
		offset += len;
		i += 1;
	}
	offset
}

// Architecture-specific modules
#[cfg(target_arch = "x86_64")]
mod x86_64_simd;

#[cfg(target_arch = "aarch64")]
mod aarch64_simd;

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
mod generic_simd;

// Re-export the appropriate implementation
#[cfg(target_arch = "x86_64")]
pub use x86_64_simd::X86_64Simd as CurrentSimd;

#[cfg(target_arch = "aarch64")]
pub use aarch64_simd::Aarch64Simd as CurrentSimd;

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub use generic_simd::GenericSimd as CurrentSimd;

/// Bulk encoding function for u32 values using SIMD optimizations.
///
/// # Safety
///
/// - The buffer must be large enough to hold all encoded values
/// - The buffer size should be at least `values.len() * 5` bytes
/// - The caller must ensure the buffer is valid for the duration of the operation
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#[inline]
pub unsafe fn bulk_encode_u32(buf: &mut [u8], values: &[u32]) -> usize {
	CurrentSimd::bulk_encode_u32(buf, values)
}

/// Bulk decoding function for u32 values using SIMD optimizations.
///
/// # Safety
///
/// - The buffer must contain valid encoded data
/// - The values array must be large enough to hold all decoded values
/// - The caller must ensure the buffer is valid for the duration of the operation
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
#[inline]
pub unsafe fn bulk_decode_u32(buf: &[u8], values: &mut [u32]) -> usize {
	CurrentSimd::bulk_decode_u32(buf, values)
}

/// Bulk encoding function for u32 values using generic implementation.
///
/// # Safety
///
/// - The buffer must be large enough to hold all encoded values
/// - The buffer size should be at least `values.len() * 5` bytes
/// - The caller must ensure the buffer is valid for the duration of the operation
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
#[inline]
pub unsafe fn bulk_encode_u32(buf: &mut [u8], values: &[u32]) -> usize {
	CurrentSimd::bulk_encode_u32(buf, values)
}

/// Bulk decoding function for u32 values using generic implementation.
///
/// # Safety
///
/// - The buffer must contain valid encoded data
/// - The values array must be large enough to hold all decoded values
/// - The caller must ensure the buffer is valid for the duration of the operation
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
#[inline]
pub unsafe fn bulk_decode_u32(buf: &[u8], values: &mut [u32]) -> usize {
	CurrentSimd::bulk_decode_u32(buf, values)
}

/// Generic bulk encoding function that works with any integer type.
#[inline]
pub fn bulk_encode<T>(
	buf: &mut [u8],
	values: &[T],
) -> Result<usize, &'static str>
where
	T: crate::encode::Encode + Copy,
{
	let mut offset = 0;
	for &value in values {
		if offset >= buf.len() {
			return Err("buffer too small for bulk encoding");
		}
		let len = T::encode(&mut buf[offset..], value)?;
		offset += len;
	}
	Ok(offset)
}

/// Generic bulk decoding function that works with any integer type.
#[inline]
pub fn bulk_decode<T>(
	buf: &[u8],
	values: &mut [T],
) -> Result<usize, &'static str>
where
	T: crate::decode::Decode,
{
	let mut offset = 0;
	let mut i = 0;
	while i < values.len() && offset < buf.len() {
		let (value, len) = T::decode(&buf[offset..])?;
		values[i] = value;
		offset += len;
		i += 1;
	}
	Ok(offset)
}

/// Safe wrapper for bulk encoding u32 values.
#[inline]
pub fn bulk_encode_u32_safe(
	buf: &mut [u8],
	values: &[u32],
) -> Result<usize, &'static str> {
	if buf.len() < values.len() * 5 {
		return Err("buffer too small for bulk encoding");
	}
	Ok(unsafe { bulk_encode_u32(buf, values) })
}

/// Safe wrapper for bulk decoding u32 values.
#[inline]
pub fn bulk_decode_u32_safe(
	buf: &[u8],
	values: &mut [u32],
) -> Result<usize, &'static str> {
	if buf.is_empty() {
		return Ok(0);
	}
	Ok(unsafe { bulk_decode_u32(buf, values) })
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_basic_encoding() {
		let mut buf = [0u8; 20];
		let values = [1u32, 2, 3, 4];
		let len = unsafe { bulk_encode_u32(&mut buf, &values) };
		assert!(len > 0);
	}

	#[test]
	fn test_encode_decode_roundtrip() {
		let mut buf = [0u8; 20];
		let values = [1u32, 1000, 1000000, 1000000000];
		let encoded_len = unsafe { bulk_encode_u32(&mut buf, &values) };
		let mut decoded_values = [0u32; 4];
		let _decoded_len = unsafe {
			bulk_decode_u32(&buf[..encoded_len], &mut decoded_values)
		};
		assert_eq!(values, decoded_values);
	}

	#[test]
	fn test_4byte_5byte_encoding() {
		let mut buf = [0u8; 20];
		let values = [0x10000000u32, 0x20000000, 0x30000000, 0x40000000];
		let len = unsafe { bulk_encode_u32(&mut buf, &values) };
		assert!(len > 0);
	}

	#[test]
	fn test_mixed_encoding_sizes() {
		let mut buf = [0u8; 20];
		let values = [1u32, 1000, 1000000, 1000000000];
		let len = unsafe { bulk_encode_u32(&mut buf, &values) };
		assert!(len > 0);
	}

	#[test]
	fn test_decode_2byte_values() {
		let mut buf = [0u8; 20];
		let values = [1000u32, 2000, 3000, 4000];
		let encoded_len = unsafe { bulk_encode_u32(&mut buf, &values) };
		let mut decoded_values = [0u32; 4];
		let _decoded_len = unsafe {
			bulk_decode_u32(&buf[..encoded_len], &mut decoded_values)
		};
		assert_eq!(values, decoded_values);
	}

	#[test]
	fn test_decode_3byte_values() {
		let mut buf = [0u8; 12];
		let values = [100000u32, 200000, 300000, 400000];
		let encoded_len = unsafe { bulk_encode_u32(&mut buf, &values) };
		let mut decoded_values = [0u32; 4];
		let _decoded_len = unsafe {
			bulk_decode_u32(&buf[..encoded_len], &mut decoded_values)
		};
		assert_eq!(values, decoded_values);
	}

	#[test]
	fn test_decode_mixed_sizes_in_sequence() {
		let mut buf = [0u8; 20];
		let values = [1u32, 1000, 100000, 10000000];
		let encoded_len = unsafe { bulk_encode_u32(&mut buf, &values) };
		let mut decoded_values = [0u32; 4];
		let _decoded_len = unsafe {
			bulk_decode_u32(&buf[..encoded_len], &mut decoded_values)
		};
		assert_eq!(values, decoded_values);
	}
}
