//! Generic SIMD implementation for architectures without SIMD support

use super::SimdImpl;
use crate::decode::decode_u32;
use crate::encode::encode_u32;

/// Generic SIMD implementation for architectures without SIMD support
pub struct GenericSimd;

impl SimdImpl for GenericSimd {
	#[inline]
	unsafe fn bulk_encode_u32(buf: &mut [u8], values: &[u32]) -> usize {
		let mut offset = 0;
		for &value in values {
			unsafe {
				let buf_ptr = buf.as_mut_ptr().add(offset) as *mut [u8; 5];
				offset += encode_u32(&mut *buf_ptr, value);
			}
		}
		offset
	}

	#[inline]
	unsafe fn bulk_decode_u32(buf: &[u8], values: &mut [u32]) -> usize {
		let mut offset = 0;
		let mut i = 0;
		while i < values.len() && offset < buf.len() {
			let mut temp_buf = [0u8; 5];
			let copy_len = core::cmp::min(5, buf.len() - offset);
			temp_buf[..copy_len]
				.copy_from_slice(&buf[offset..offset + copy_len]);
			let (value, len) = decode_u32(&temp_buf);
			values[i] = value;
			offset += len;
			i += 1;
		}
		offset
	}
}
