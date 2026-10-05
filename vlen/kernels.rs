//! Native SIMD kernels for the bulk run fast paths (`simd` feature).
//!
//! These are drop-in replacements for the portable SWAR lane
//! producers in `bulk.rs`, and the only unsafe code in the crate. The
//! unsafe surface is deliberately tiny and sound by construction:
//!
//! - Every intrinsic used is part of the target's *baseline* feature
//!   set — NEON on aarch64, SSE2 on x86_64 — so no runtime detection
//!   or `#[target_feature]` preconditions apply. The exception is the
//!   x86_64 SSSE3 kernel, which compiles only when the build itself
//!   enables SSSE3 (`-C target-cpu=x86-64-v2` and up, or `native`), so
//!   it needs no detection either: a binary built for it requires it.
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

/// Reassembles eight three-byte encodings (24 bytes) into eight 32-bit
/// value lanes, returning `false` (with `lanes` unspecified) unless every
/// first byte is a three-byte prefix, `110xxxxx`.
#[cfg(all(target_arch = "x86_64", target_feature = "ssse3"))]
#[inline(always)]
pub(crate) fn three_byte_lanes(bytes: &[u8; 24], lanes: &mut [u32; 8]) -> bool {
	use core::arch::x86_64::*;
	// SAFETY: SSSE3 is statically enabled (this function only compiles
	// under cfg(target_feature = "ssse3")) and every other intrinsic is
	// SSE2. The loads read bytes 0..16 and 8..24 of the 24-byte array and
	// the stores write lanes 0..4 and 4..8 of the eight-lane array.
	unsafe {
		// Spread each encoding into its own 32-bit lane, first byte
		// lowest, with a zero byte on top.
		let spread_lo =
			_mm_setr_epi8(0, 1, 2, -1, 3, 4, 5, -1, 6, 7, 8, -1, 9, 10, 11, -1);
		let spread_hi = _mm_setr_epi8(
			4, 5, 6, -1, 7, 8, 9, -1, 10, 11, 12, -1, 13, 14, 15, -1,
		);
		let a =
			_mm_shuffle_epi8(_mm_loadu_si128(bytes.as_ptr().cast()), spread_lo);
		let b = _mm_shuffle_epi8(
			_mm_loadu_si128(bytes.as_ptr().add(8).cast()),
			spread_hi,
		);
		let prefix_bits = _mm_set1_epi32(0xE0);
		let three_byte = _mm_set1_epi32(0xC0);
		let valid = _mm_and_si128(
			_mm_cmpeq_epi32(_mm_and_si128(a, prefix_bits), three_byte),
			_mm_cmpeq_epi32(_mm_and_si128(b, prefix_bits), three_byte),
		);
		if _mm_movemask_epi8(valid) != 0xFFFF {
			return false;
		}
		// value = (lane >> 8) << 5 | (lane & 0x1F), per 32-bit lane.
		let low = _mm_set1_epi32(0x1F);
		let va = _mm_or_si128(
			_mm_slli_epi32::<5>(_mm_srli_epi32::<8>(a)),
			_mm_and_si128(a, low),
		);
		let vb = _mm_or_si128(
			_mm_slli_epi32::<5>(_mm_srli_epi32::<8>(b)),
			_mm_and_si128(b, low),
		);
		_mm_storeu_si128(lanes.as_mut_ptr().cast(), va);
		_mm_storeu_si128(lanes.as_mut_ptr().add(4).cast(), vb);
	}
	true
}

#[cfg(target_arch = "aarch64")]
pub(crate) fn one_byte_lanes(bytes: &[u8; 8], lanes: &mut [u16; 8]) {
	use core::arch::aarch64::*;
	// SAFETY: NEON is a baseline feature of every aarch64 target, and
	// the load/store pointers come from array references valid for
	// exactly the accessed widths.
	unsafe {
		let v = vld1_u8(bytes.as_ptr());
		vst1q_u16(lanes.as_mut_ptr(), vmovl_u8(v));
	}
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn one_byte_lanes(bytes: &[u8; 8], lanes: &mut [u16; 8]) {
	use core::arch::x86_64::*;
	// SAFETY: every intrinsic here is SSE2, a baseline feature of the
	// x86_64 target; the pointers come from array references valid
	// for exactly the accessed widths.
	unsafe {
		let v = _mm_loadl_epi64(bytes.as_ptr().cast());
		let wide = _mm_unpacklo_epi8(v, _mm_setzero_si128());
		_mm_storeu_si128(lanes.as_mut_ptr().cast(), wide);
	}
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub(crate) fn one_byte_lanes(bytes: &[u8; 8], lanes: &mut [u16; 8]) {
	use core::arch::wasm32::*;
	// SAFETY: simd128 is statically enabled (this function only
	// compiles under cfg(target_feature = "simd128")), and the
	// pointers come from array references valid for exactly the
	// accessed widths.
	unsafe {
		let wide = u16x8_load_extend_u8x8(bytes.as_ptr());
		v128_store(lanes.as_mut_ptr().cast(), wide);
	}
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub(crate) fn two_byte_lanes(bytes: &[u8; 16], lanes: &mut [u16; 8]) {
	use core::arch::wasm32::*;
	// SAFETY: as above.
	unsafe {
		let v = v128_load(bytes.as_ptr().cast());
		let payload = u16x8_shr(v, 8);
		let low = v128_and(v, u16x8_splat(0x003F));
		let value = v128_or(u16x8_shl(payload, 6), low);
		v128_store(lanes.as_mut_ptr().cast(), value);
	}
}

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub(crate) fn four_byte_lanes(bytes: &[u8; 16], lanes: &mut [u32; 4]) {
	use core::arch::wasm32::*;
	// SAFETY: as above.
	unsafe {
		let v = v128_load(bytes.as_ptr().cast());
		let value = v128_or(
			u32x4_shl(u32x4_shr(v, 8), 4),
			v128_and(v, u32x4_splat(0x0F)),
		);
		v128_store(lanes.as_mut_ptr().cast(), value);
	}
}
