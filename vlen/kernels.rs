//! Native SIMD kernels for the bulk run fast paths (`simd` feature).
//!
//! These are drop-in replacements for the portable SWAR lane
//! producers in `bulk.rs`, and the only unsafe code in the crate. The
//! unsafe surface is deliberately tiny and sound by construction:
//!
//! - Every intrinsic used is part of the target's *baseline* feature
//!   set — NEON on aarch64, SSE2 on x86_64 — so no runtime detection
//!   or `#[target_feature]` preconditions apply.
//! - Loads and stores take pointers derived from array references,
//!   which guarantee validity for exactly the accessed widths.
//!
//! Correctness is pinned by the same equivalence suites that cover
//! the portable paths: the full test suite runs with and without this
//! feature, on both architectures, in CI.

#[cfg(target_arch = "aarch64")]
pub(crate) fn two_byte_lanes(bytes: &[u8; 16], lanes: &mut [u16; 8]) {
	use core::arch::aarch64::*;
	// SAFETY: NEON is a baseline feature of every aarch64 target, and
	// the load/store pointers come from array references valid for
	// exactly sixteen bytes each.
	unsafe {
		let v = vld1q_u8(bytes.as_ptr());
		// De-interleave first bytes (evens) from payload bytes (odds).
		let evens = vget_low_u8(vuzp1q_u8(v, v));
		let odds = vget_low_u8(vuzp2q_u8(v, v));
		// value = payload << 6 | (first & 0x3F), widened to 16 bits.
		let low = vmovl_u8(vand_u8(evens, vdup_n_u8(0x3F)));
		let value = vorrq_u16(vshll_n_u8::<6>(odds), low);
		vst1q_u16(lanes.as_mut_ptr(), value);
	}
}

#[cfg(target_arch = "aarch64")]
pub(crate) fn four_byte_lanes(bytes: &[u8; 16], lanes: &mut [u32; 4]) {
	use core::arch::aarch64::*;
	// SAFETY: as above.
	unsafe {
		let v = vreinterpretq_u32_u8(vld1q_u8(bytes.as_ptr()));
		// value = (lane >> 8) << 4 | (lane & 0x0F), per 32-bit lane.
		let value = vorrq_u32(
			vshlq_n_u32::<4>(vshrq_n_u32::<8>(v)),
			vandq_u32(v, vdupq_n_u32(0x0F)),
		);
		vst1q_u32(lanes.as_mut_ptr(), value);
	}
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn two_byte_lanes(bytes: &[u8; 16], lanes: &mut [u16; 8]) {
	use core::arch::x86_64::*;
	// SAFETY: every intrinsic here is SSE2, a baseline feature of the
	// x86_64 target; the unaligned load/store pointers come from
	// array references valid for exactly sixteen bytes each.
	unsafe {
		let v = _mm_loadu_si128(bytes.as_ptr().cast());
		// Each 16-bit lane holds first | payload << 8 (little-endian);
		// value = payload << 6 | (first & 0x3F).
		let payload = _mm_srli_epi16::<8>(v);
		let low = _mm_and_si128(v, _mm_set1_epi16(0x003F));
		let value = _mm_or_si128(_mm_slli_epi16::<6>(payload), low);
		_mm_storeu_si128(lanes.as_mut_ptr().cast(), value);
	}
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn four_byte_lanes(bytes: &[u8; 16], lanes: &mut [u32; 4]) {
	use core::arch::x86_64::*;
	// SAFETY: as above.
	unsafe {
		let v = _mm_loadu_si128(bytes.as_ptr().cast());
		// value = (lane >> 8) << 4 | (lane & 0x0F), per 32-bit lane.
		let value = _mm_or_si128(
			_mm_slli_epi32::<4>(_mm_srli_epi32::<8>(v)),
			_mm_and_si128(v, _mm_set1_epi32(0x0F)),
		);
		_mm_storeu_si128(lanes.as_mut_ptr().cast(), value);
	}
}
