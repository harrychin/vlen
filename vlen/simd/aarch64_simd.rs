//! aarch64 SIMD implementation using ARM NEON instructions

use super::{handle_remaining_decode, handle_remaining_encode, SimdImpl};

#[cfg(not(test))]
use core::arch::aarch64::*;
#[cfg(test)]
use std::arch::aarch64::*;

/// aarch64 SIMD implementation using ARM NEON instructions
pub struct Aarch64Simd;

impl SimdImpl for Aarch64Simd {
	#[inline]
	unsafe fn bulk_encode_u32(buf: &mut [u8], values: &[u32]) -> usize {
		let mut offset = 0;
		let mut i = 0;

		while i + 3 < values.len() {
			let values_vec = vsetq_lane_u32(
				values[i + 3],
				vsetq_lane_u32(
					values[i + 2],
					vsetq_lane_u32(
						values[i + 1],
						vsetq_lane_u32(values[i], vdupq_n_u32(0), 0),
						1,
					),
					2,
				),
				3,
			);

			let max_value = values[i..i + 4].iter().max().unwrap();
			let bytes_needed = if *max_value < 0x4000 {
				encode_2byte(buf, offset, values_vec)
			} else if *max_value < 0x200000 {
				encode_3byte(buf, offset, values_vec)
			} else if *max_value < 0x10000000 {
				encode_4byte(buf, offset, values_vec)
			} else {
				encode_5byte(buf, offset, values_vec)
			};

			offset += bytes_needed;
			i += 4;
		}

		handle_remaining_encode(buf, values, offset, i)
	}

	#[inline]
	unsafe fn bulk_decode_u32(buf: &[u8], values: &mut [u32]) -> usize {
		let mut offset = 0;
		let mut i = 0;

		while i + 3 < values.len() && offset + 20 <= buf.len() {
			let first_byte = buf[offset];
			let bytes_needed = if first_byte < 0xC0 {
				decode_2byte(buf, offset, values, i)
			} else if first_byte < 0xE0 {
				decode_3byte(buf, offset, values, i)
			} else if first_byte < 0xF0 {
				decode_4byte(buf, offset, values, i)
			} else {
				decode_5byte(buf, offset, values, i)
			};

			offset = bytes_needed;
			i += 4;
		}

		handle_remaining_decode(buf, values, offset, i)
	}
}

#[inline]
unsafe fn encode_2byte(
	buf: &mut [u8],
	offset: usize,
	values: uint32x4_t,
) -> usize {
	let v0 = vgetq_lane_u32(values, 0);
	let v1 = vgetq_lane_u32(values, 1);
	let v2 = vgetq_lane_u32(values, 2);
	let v3 = vgetq_lane_u32(values, 3);

	let mut combined = [0u8; 8];
	combined[0] = 0x80 | ((v0 & 0x3F) as u8);
	combined[1] = (v0 >> 6) as u8;
	combined[2] = 0x80 | ((v1 & 0x3F) as u8);
	combined[3] = (v1 >> 6) as u8;
	combined[4] = 0x80 | ((v2 & 0x3F) as u8);
	combined[5] = (v2 >> 6) as u8;
	combined[6] = 0x80 | ((v3 & 0x3F) as u8);
	combined[7] = (v3 >> 6) as u8;

	vst1q_u8(buf.as_mut_ptr().add(offset), vld1q_u8(combined.as_ptr()));
	8
}

#[inline]
unsafe fn encode_3byte(
	buf: &mut [u8],
	offset: usize,
	values: uint32x4_t,
) -> usize {
	let v0 = vgetq_lane_u32(values, 0);
	let v1 = vgetq_lane_u32(values, 1);
	let v2 = vgetq_lane_u32(values, 2);
	let v3 = vgetq_lane_u32(values, 3);

	let mut combined = [0u8; 12];
	combined[0] = 0xC0 | ((v0 & 0x1F) as u8);
	combined[1] = (v0 >> 5) as u8;
	combined[2] = (v0 >> 13) as u8;
	combined[3] = 0xC0 | ((v1 & 0x1F) as u8);
	combined[4] = (v1 >> 5) as u8;
	combined[5] = (v1 >> 13) as u8;
	combined[6] = 0xC0 | ((v2 & 0x1F) as u8);
	combined[7] = (v2 >> 5) as u8;
	combined[8] = (v2 >> 13) as u8;
	combined[9] = 0xC0 | ((v3 & 0x1F) as u8);
	combined[10] = (v3 >> 5) as u8;
	combined[11] = (v3 >> 13) as u8;

	vst1q_u8(buf.as_mut_ptr().add(offset), vld1q_u8(combined.as_ptr()));
	vst1_u8(
		buf.as_mut_ptr().add(offset + 8),
		vld1_u8(combined.as_ptr().add(8)),
	);
	12
}

#[inline]
unsafe fn encode_4byte(
	buf: &mut [u8],
	offset: usize,
	values: uint32x4_t,
) -> usize {
	let v0 = vgetq_lane_u32(values, 0);
	let v1 = vgetq_lane_u32(values, 1);
	let v2 = vgetq_lane_u32(values, 2);
	let v3 = vgetq_lane_u32(values, 3);

	let mut combined = [0u8; 16];
	combined[0] = 0xE0 | ((v0 & 0x0F) as u8);
	combined[1] = (v0 >> 4) as u8;
	combined[2] = (v0 >> 12) as u8;
	combined[3] = (v0 >> 20) as u8;
	combined[4] = 0xE0 | ((v1 & 0x0F) as u8);
	combined[5] = (v1 >> 4) as u8;
	combined[6] = (v1 >> 12) as u8;
	combined[7] = (v1 >> 20) as u8;
	combined[8] = 0xE0 | ((v2 & 0x0F) as u8);
	combined[9] = (v2 >> 4) as u8;
	combined[10] = (v2 >> 12) as u8;
	combined[11] = (v2 >> 20) as u8;
	combined[12] = 0xE0 | ((v3 & 0x0F) as u8);
	combined[13] = (v3 >> 4) as u8;
	combined[14] = (v3 >> 12) as u8;
	combined[15] = (v3 >> 20) as u8;

	vst1q_u8(buf.as_mut_ptr().add(offset), vld1q_u8(combined.as_ptr()));
	vst1q_u8(
		buf.as_mut_ptr().add(offset + 8),
		vld1q_u8(combined.as_ptr().add(8)),
	);
	16
}

#[inline]
unsafe fn encode_5byte(
	buf: &mut [u8],
	offset: usize,
	values: uint32x4_t,
) -> usize {
	let v0 = vgetq_lane_u32(values, 0);
	let v1 = vgetq_lane_u32(values, 1);
	let v2 = vgetq_lane_u32(values, 2);
	let v3 = vgetq_lane_u32(values, 3);

	let mut combined = [0u8; 20];
	combined[0] = 0xF3;
	combined[1] = (v0 & 0xFF) as u8;
	combined[2] = (v0 >> 8) as u8;
	combined[3] = (v0 >> 16) as u8;
	combined[4] = (v0 >> 24) as u8;
	combined[5] = 0xF3;
	combined[6] = (v1 & 0xFF) as u8;
	combined[7] = (v1 >> 8) as u8;
	combined[8] = (v1 >> 16) as u8;
	combined[9] = (v1 >> 24) as u8;
	combined[10] = 0xF3;
	combined[11] = (v2 & 0xFF) as u8;
	combined[12] = (v2 >> 8) as u8;
	combined[13] = (v2 >> 16) as u8;
	combined[14] = (v2 >> 24) as u8;
	combined[15] = 0xF3;
	combined[16] = (v3 & 0xFF) as u8;
	combined[17] = (v3 >> 8) as u8;
	combined[18] = (v3 >> 16) as u8;
	combined[19] = (v3 >> 24) as u8;

	vst1q_u8(buf.as_mut_ptr().add(offset), vld1q_u8(combined.as_ptr()));
	vst1q_u8(
		buf.as_mut_ptr().add(offset + 8),
		vld1q_u8(combined.as_ptr().add(8)),
	);
	vst1_u32(
		buf.as_mut_ptr().add(offset + 16) as *mut u32,
		vld1_u32(combined.as_ptr().add(16) as *const u32),
	);
	20
}

#[inline]
unsafe fn decode_2byte(
	buf: &[u8],
	offset: usize,
	values: &mut [u32],
	i: usize,
) -> usize {
	let data = vld1q_u8(buf.as_ptr().add(offset));

	let low_bits = vandq_u8(data, vdupq_n_u8(0x3F));
	let high_bits = vshlq_n_u32(vreinterpretq_u32_u8(vshrq_n_u8(data, 1)), 6);
	let combined = vorrq_u32(vreinterpretq_u32_u8(low_bits), high_bits);

	vst1q_u32(values.as_mut_ptr().add(i), combined);

	offset + 8
}

#[inline]
unsafe fn decode_3byte(
	buf: &[u8],
	offset: usize,
	values: &mut [u32],
	i: usize,
) -> usize {
	let data_low = vld1q_u8(buf.as_ptr().add(offset));

	let low_bits = vandq_u8(data_low, vdupq_n_u8(0x1F));
	let mid_bits =
		vshlq_n_u32(vreinterpretq_u32_u8(vshrq_n_u8(data_low, 1)), 5);
	let high_bits =
		vshlq_n_u32(vreinterpretq_u32_u8(vshrq_n_u8(data_low, 2)), 13);

	let combined = vorrq_u32(
		vorrq_u32(vreinterpretq_u32_u8(low_bits), mid_bits),
		high_bits,
	);

	vst1q_u32(values.as_mut_ptr().add(i), combined);

	offset + 12
}

#[inline]
unsafe fn decode_4byte(
	buf: &[u8],
	offset: usize,
	values: &mut [u32],
	i: usize,
) -> usize {
	let data = vld1q_u8(buf.as_ptr().add(offset));

	let low_bits = vandq_u8(data, vdupq_n_u8(0x0F));
	let mid1_bits = vshlq_n_u32(vreinterpretq_u32_u8(vshrq_n_u8(data, 1)), 4);
	let mid2_bits = vshlq_n_u32(vreinterpretq_u32_u8(vshrq_n_u8(data, 2)), 12);
	let high_bits = vshlq_n_u32(vreinterpretq_u32_u8(vshrq_n_u8(data, 3)), 20);

	let combined = vorrq_u32(
		vorrq_u32(
			vorrq_u32(vreinterpretq_u32_u8(low_bits), mid1_bits),
			mid2_bits,
		),
		high_bits,
	);

	vst1q_u32(values.as_mut_ptr().add(i), combined);

	offset + 16
}

#[inline]
unsafe fn decode_5byte(
	buf: &[u8],
	offset: usize,
	values: &mut [u32],
	i: usize,
) -> usize {
	let mut temp_buf = [0u8; 20];
	let copy_len = core::cmp::min(20, buf.len() - offset);
	temp_buf[..copy_len].copy_from_slice(&buf[offset..offset + copy_len]);

	let mut out = [0u32; 4];
	for (j, item) in out.iter_mut().enumerate() {
		if i + j >= values.len() {
			break;
		}
		let data_offset = j * 5 + 1;
		*item = u32::from_le_bytes([
			temp_buf[data_offset],
			temp_buf[data_offset + 1],
			temp_buf[data_offset + 2],
			temp_buf[data_offset + 3],
		]);
	}
	vst1q_u32(values.as_mut_ptr().add(i), vld1q_u32(out.as_ptr()));
	offset + 20
}
