//! Bulk encoding and decoding of many values.
//!
//! All functions here produce and consume the canonical vlen byte
//! stream — output is byte-for-byte identical to encoding each value
//! individually, and any mix of the bulk and per-value APIs
//! interoperates. The specialized functions for `u32`, `u64`, `i32`,
//! and `i64` add SWAR fast paths that process runs of equal-length
//! encodings several values at a time; they are portable safe Rust,
//! active on every architecture. Prefer them whenever the data has
//! runs of similarly-sized values (the signed variants shine on
//! delta-encoded streams). Windows that are not runs encode
//! branch-free, so the specialized encoders also lead on interleaved
//! sizes; there the generic decoders are about 1.3x faster, because
//! they skip the run detection.

use crate::decode::Decode;
use crate::encode::Encode;
use crate::error::Result;

/// Encodes a slice of values into `buf`, returning the total encoded
/// length.
///
/// The buffer only needs room for the actual encoded stream; encoding
/// stops with [`Error::BufferTooSmall`](crate::Error::BufferTooSmall)
/// if it runs out of space. Bytes past the returned length may be
/// overwritten with scratch data, here and in the specialized bulk
/// encoders.
pub fn bulk_encode<T: Encode>(buf: &mut [u8], values: &[T]) -> Result<usize> {
	let mut offset = 0;
	for &value in values {
		offset += value.encode(&mut buf[offset..])?;
	}
	Ok(offset)
}

/// Decodes exactly `out.len()` values from `buf`, returning the number
/// of bytes consumed.
///
/// Fails if the buffer holds fewer values than `out` expects or if the
/// stream is invalid; `out` may be partially overwritten in that case.
/// To decode a stream of unknown length, use [`decode_iter`].
pub fn bulk_decode<T: Decode>(buf: &[u8], out: &mut [T]) -> Result<usize> {
	let mut offset = 0;
	for slot in out.iter_mut() {
		let (value, len) = T::decode(&buf[offset..])?;
		*slot = value;
		offset += len;
	}
	Ok(offset)
}

/// Every one-byte encoding has its top bit clear.
const ONE_BYTE_RUN: u64 = 0x8080_8080_8080_8080;
/// Two-byte encodings start `10xxxxxx`: these masks test the top two
/// bits of the four even-position bytes in a window at once.
const TWO_BYTE_MASK: u64 = 0x00C0_00C0_00C0_00C0;
const TWO_BYTE_WANT: u64 = 0x0080_0080_0080_0080;
/// Four-byte encodings start `1110xxxx`: top four bits of bytes 0
/// and 4.
const FOUR_BYTE_MASK: u64 = 0x0000_00F0_0000_00F0;
const FOUR_BYTE_WANT: u64 = 0x0000_00E0_0000_00E0;
/// Three-byte encodings start `110xxxxx`: top three bits of bytes 0,
/// 3, and 6.
#[cfg(all(feature = "simd", target_arch = "x86_64", target_feature = "ssse3"))]
const THREE_BYTE_MASK: u64 = 0x00E0_0000_E000_00E0;
#[cfg(all(feature = "simd", target_arch = "x86_64", target_feature = "ssse3"))]
const THREE_BYTE_WANT: u64 = 0x00C0_0000_C000_00C0;

/// Reassembles eight two-byte encodings (sixteen interleaved bytes)
/// into eight 16-bit value lanes. The caller has already verified the
/// prefix bits of all eight first bytes.
#[inline(always)]
fn two_byte_lanes(bytes: &[u8; 16], lanes: &mut [u16; 8]) {
	#[cfg(all(
		feature = "simd",
		any(
			target_arch = "aarch64",
			target_arch = "x86_64",
			all(target_arch = "wasm32", target_feature = "simd128")
		)
	))]
	{
		crate::kernels::two_byte_lanes(bytes, lanes);
	}
	#[cfg(not(all(
		feature = "simd",
		any(
			target_arch = "aarch64",
			target_arch = "x86_64",
			all(target_arch = "wasm32", target_feature = "simd128")
		)
	)))]
	{
		let mut half = 0;
		while half < 2 {
			let word = u64::from_le_bytes(
				bytes[half * 8..half * 8 + 8].try_into().unwrap(),
			);
			let lo = word & 0x003F_003F_003F_003F;
			let hi = (word >> 8) & 0x00FF_00FF_00FF_00FF;
			let packed = (hi << 6) | lo;
			lanes[half * 4] = (packed & 0xFFFF) as u16;
			lanes[half * 4 + 1] = ((packed >> 16) & 0xFFFF) as u16;
			lanes[half * 4 + 2] = ((packed >> 32) & 0xFFFF) as u16;
			lanes[half * 4 + 3] = (packed >> 48) as u16;
			half += 1;
		}
	}
}

/// Reassembles four four-byte encodings (sixteen bytes) into four
/// 32-bit value lanes. The caller has already verified the prefix
/// nibbles of all four first bytes.
#[inline(always)]
fn four_byte_lanes(bytes: &[u8; 16], lanes: &mut [u32; 4]) {
	#[cfg(all(
		feature = "simd",
		any(
			target_arch = "aarch64",
			target_arch = "x86_64",
			all(target_arch = "wasm32", target_feature = "simd128")
		)
	))]
	{
		crate::kernels::four_byte_lanes(bytes, lanes);
	}
	#[cfg(not(all(
		feature = "simd",
		any(
			target_arch = "aarch64",
			target_arch = "x86_64",
			all(target_arch = "wasm32", target_feature = "simd128")
		)
	)))]
	{
		let mut i = 0;
		while i < 4 {
			let lane =
				u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
			lanes[i] = ((lane >> 8) << 4) | (lane & 0x0F);
			i += 1;
		}
	}
}

/// Generates the window fast paths for one unsigned width.
///
/// Both directions detect runs of equal-length encodings, whose
/// boundaries are known in advance and therefore need no per-value
/// branching: one- and two-byte runs move through SWAR lanes, and
/// three- to five-byte runs use class-known constructions — a single
/// full-width store or masked load per value with no length
/// computation at all. Widths with longer encodings pass `$try_wide`
/// to encode the remaining binary length-prefix classes the same way.
/// Windows without a run return `None` and the caller falls back to
/// the branchy scalar codec, which branch prediction serves best.
macro_rules! window_run_fns {
	($try_enc:ident, $try_dec:ident, $ut:ident $(, $try_wide:ident)?) => {
		/// Attempts the run fast paths on one eight-value window.
		/// Returns the new byte offset after encoding all eight, or
		/// `None` when the window holds no run (or the output lacks
		/// scratch room) and the caller must encode it one value at
		/// a time.
		#[inline(always)]
		fn $try_enc(
			buf: &mut [u8],
			offset: usize,
			chunk: &[$ut; 8],
		) -> Option<usize> {
			// Verifies that all eight values sit in the class
			// `[lo, lo + span)`: values below `lo` wrap to huge, values
			// at or beyond the class stay at `span` or more. Each lane
			// is compared on its own; or-reducing the lanes first is
			// only exact when `span` is a power of two, and for the
			// wider classes it rejects nearly every window of
			// uniformly spread values.
			#[inline(always)]
			fn in_class(chunk: &[$ut; 8], lo: $ut, span: $ut) -> bool {
				chunk
					.iter()
					.fold(true, |acc, &v| acc & (v.wrapping_sub(lo) < span))
			}

			// The two hottest classes get dedicated cheap gates;
			// wider classes share one dispatch on the encoded size
			// of the window's first and last values.
			let first = chunk[0];
			let last = chunk[7];
			// The one-byte span is a power of two, so a single
			// or-reduction is exact here.
			if (first | last) < 0x80
				&& chunk.iter().fold(0, |acc, &v| acc | v) < 0x80
			{
				// Eight one-byte values become one packed word.
				let dst = buf.get_mut(offset..offset + 8)?;
				let mut word = 0u64;
				for (j, &v) in chunk.iter().enumerate() {
					word |= (v as u64) << (8 * j);
				}
				dst.copy_from_slice(&word.to_le_bytes());
				return Some(offset + 8);
			}
			if first.wrapping_sub(0x80).max(last.wrapping_sub(0x80)) < 0x3F80
				&& in_class(chunk, 0x80, 0x3F80)
			{
				// Eight two-byte values: each 16-bit lane's
				// little-endian bytes are exactly the encoding, so
				// the lane array serializes as one contiguous block.
				let dst = buf.get_mut(offset..offset + 16)?;
				let mut lanes = [0u16; 8];
				for (lane, &v) in lanes.iter_mut().zip(chunk) {
					*lane =
						0x80 | ((v & 0x3F) as u16) | (((v >> 6) as u16) << 8);
				}
				for (pair, &lane) in dst.chunks_exact_mut(2).zip(&lanes) {
					pair.copy_from_slice(&lane.to_le_bytes());
				}
				return Some(offset + 16);
			}
			if first >= 0x4000 && first.encoded_size() == last.encoded_size() {
				match first.encoded_size() {
					3 if in_class(chunk, 0x4000, 0x1F_C000) => {
						// One full-width store per value at a
						// three-byte stride; the next store
						// overwrites the scratch bytes.
						let dst = buf.get_mut(offset..offset + 3 * 8 + 5)?;
						let mut o = 0;
						for &v in chunk {
							let word = (((v >> 5) as u64) << 8)
								| (0xC0 | ((v & 0x1F) as u64));
							dst[o..o + 8].copy_from_slice(&word.to_le_bytes());
							o += 3;
						}
						return Some(offset + 3 * 8);
					},
					4 if in_class(chunk, 0x20_0000, 0xFE0_0000) => {
						// Each 32-bit lane's little-endian bytes are
						// exactly the four-byte encoding; no scratch
						// bytes needed.
						let dst = buf.get_mut(offset..offset + 32)?;
						let mut lanes = [0u32; 8];
						for (lane, &v) in lanes.iter_mut().zip(chunk) {
							*lane = 0xE0
								| ((v & 0x0F) as u32)
								| (((v >> 4) as u32) << 8);
						}
						for (quad, &lane) in dst.chunks_exact_mut(4).zip(&lanes)
						{
							quad.copy_from_slice(&lane.to_le_bytes());
						}
						return Some(offset + 32);
					},
					5 if in_class(chunk, 0x1000_0000, 0xF000_0000) => {
						let dst = buf.get_mut(offset..offset + 5 * 8 + 3)?;
						let mut o = 0;
						for &v in chunk {
							let word = ((v as u64) << 8) | 0xF3;
							dst[o..o + 8].copy_from_slice(&word.to_le_bytes());
							o += 5;
						}
						return Some(offset + 5 * 8);
					},
					$(len @ 6..=9 => return $try_wide(buf, offset, chunk, len),)?
					_ => {},
				}
			}
			None
		}

		/// Attempts the run fast paths at one stream position.
		/// Returns how many values were written into `slots` and the
		/// new byte offset, or `None` when no run starts here (or
		/// fewer than eight bytes remain) and the caller must decode
		/// one value at a time. Three-byte runs have a run path only
		/// with the SSSE3 kernel: uniform runs predict perfectly, and
		/// their SWAR lane math costs more than the branchy scalar
		/// path.
		#[inline(always)]
		fn $try_dec(
			buf: &[u8],
			offset: usize,
			slots: &mut [$ut; 8],
		) -> Option<(usize, usize)> {
			let chunk = buf.get(offset..offset + 8)?;
			let word = u64::from_le_bytes(chunk.try_into().unwrap());
			// Dispatch on the first encoding's size class; each
			// class runs at most one run test.
			let b0 = (word & 0xFF) as u8;
			if b0 < 0x80 {
				if word & ONE_BYTE_RUN == 0 {
					// Eight one-byte encodings.
					#[cfg(all(
						feature = "simd",
						any(
							target_arch = "aarch64",
							target_arch = "x86_64",
							all(
								target_arch = "wasm32",
								target_feature = "simd128"
							)
						)
					))]
					{
						let mut lanes = [0u16; 8];
						crate::kernels::one_byte_lanes(
							chunk.try_into().unwrap(),
							&mut lanes,
						);
						for (slot, &lane) in slots.iter_mut().zip(&lanes) {
							*slot = lane as $ut;
						}
					}
					#[cfg(not(all(
						feature = "simd",
						any(
							target_arch = "aarch64",
							target_arch = "x86_64",
							all(
								target_arch = "wasm32",
								target_feature = "simd128"
							)
						)
					)))]
					{
						for (slot, &b) in slots.iter_mut().zip(chunk) {
							*slot = b as $ut;
						}
					}
					return Some((8, offset + 8));
				}
			} else if b0 < 0xC0 {
				if word & TWO_BYTE_MASK == TWO_BYTE_WANT {
					// Try a sixteen-byte window first: eight two-byte
					// encodings in one step.
					if let Some(wide) = buf.get(offset..offset + 16) {
						let word2 =
							u64::from_le_bytes(wide[8..16].try_into().unwrap());
						if word2 & TWO_BYTE_MASK == TWO_BYTE_WANT {
							let mut lanes = [0u16; 8];
							two_byte_lanes(
								wide.try_into().unwrap(),
								&mut lanes,
							);
							for (slot, &lane) in slots.iter_mut().zip(&lanes) {
								*slot = lane as $ut;
							}
							return Some((8, offset + 16));
						}
					}
					// Four two-byte encodings: reassemble all four
					// values inside 16-bit lanes at once.
					let lo = word & 0x003F_003F_003F_003F;
					let hi = (word >> 8) & 0x00FF_00FF_00FF_00FF;
					let packed = (hi << 6) | lo;
					slots[0] = (packed & 0xFFFF) as $ut;
					slots[1] = ((packed >> 16) & 0xFFFF) as $ut;
					slots[2] = ((packed >> 32) & 0xFFFF) as $ut;
					slots[3] = (packed >> 48) as $ut;
					return Some((4, offset + 8));
				}
			} else if (0xE0..0xF0).contains(&b0) {
				if word & FOUR_BYTE_MASK == FOUR_BYTE_WANT {
					// Try a sixteen-byte window first: four four-byte
					// encodings in one step.
					if let Some(wide) = buf.get(offset..offset + 16) {
						let word2 =
							u64::from_le_bytes(wide[8..16].try_into().unwrap());
						if word2 & FOUR_BYTE_MASK == FOUR_BYTE_WANT {
							let mut lanes = [0u32; 4];
							four_byte_lanes(
								wide.try_into().unwrap(),
								&mut lanes,
							);
							for (slot, &lane) in slots.iter_mut().zip(&lanes) {
								*slot = lane as $ut;
							}
							return Some((4, offset + 16));
						}
					}
					// Two four-byte encodings in two 32-bit lanes:
					// value bits sit above the prefix nibble.
					let lane0 = word & 0xFFFF_FFFF;
					let lane1 = word >> 32;
					slots[0] = (((lane0 >> 8) << 4) | (lane0 & 0x0F)) as $ut;
					slots[1] = (((lane1 >> 8) << 4) | (lane1 & 0x0F)) as $ut;
					return Some((2, offset + 8));
				}
			} else if b0 >= 0xF0 {
				// Binary length prefix: decode pairs of equal-length
				// encodings with plain masked loads, no length
				// arithmetic per value.
				const WIDTH: usize = core::mem::size_of::<$ut>();
				let len = ((b0 & 0x0F) as usize) + 2;
				if len <= WIDTH + 1 {
					let pair = buf.get(offset..offset + 2 * len)?;
					if pair[len] == b0 && offset + len + 1 + WIDTH <= buf.len()
					{
						let payload = len - 1;
						let mask = if payload >= WIDTH {
							$ut::MAX
						} else {
							$ut::MAX >> ((WIDTH - payload) * 8)
						};
						let lo = $ut::from_le_bytes(
							buf[offset + 1..offset + 1 + WIDTH]
								.try_into()
								.unwrap(),
						);
						let hi = $ut::from_le_bytes(
							buf[offset + len + 1..offset + len + 1 + WIDTH]
								.try_into()
								.unwrap(),
						);
						slots[0] = lo & mask;
						slots[1] = hi & mask;
						return Some((2, offset + 2 * len));
					}
				}
			}
			// Eight three-byte encodings, gathered by one byte shuffle
			// per sixteen bytes once the first three prefixes in this
			// word rule out most mixed windows. This sits after the
			// dispatch so builds without the kernel keep its exact
			// shape, which the hotter classes are sensitive to.
			#[cfg(all(
				feature = "simd",
				target_arch = "x86_64",
				target_feature = "ssse3"
			))]
			if word & THREE_BYTE_MASK == THREE_BYTE_WANT {
				if let Some(window) = buf.get(offset..offset + 24) {
					let mut lanes = [0u32; 8];
					if crate::kernels::three_byte_lanes(
						window.try_into().unwrap(),
						&mut lanes,
					) {
						for (slot, &lane) in slots.iter_mut().zip(&lanes) {
							*slot = lane as $ut;
						}
						return Some((8, offset + 24));
					}
				}
			}
			None
		}
	};
}

window_run_fns!(try_encode_run_u32, try_decode_run_u32, u32);
window_run_fns!(
	try_encode_run_u64,
	try_decode_run_u64,
	u64,
	try_encode_wide_run_u64
);

/// Encodes a window of `u64` values that all fall in one binary
/// length-prefix class of six to nine bytes, or returns `None`.
#[inline(always)]
fn try_encode_wide_run_u64(
	buf: &mut [u8],
	offset: usize,
	chunk: &[u64; 8],
	len: usize,
) -> Option<usize> {
	match len {
		6 => encode_wide_run::<6>(buf, offset, chunk),
		7 => encode_wide_run::<7>(buf, offset, chunk),
		8 => encode_wide_run::<8>(buf, offset, chunk),
		9 => encode_wide_run::<9>(buf, offset, chunk),
		_ => None,
	}
}

/// One `LEN`-byte class: a prefix byte and a full eight-byte payload
/// store per value at a constant `LEN`-byte stride, so each store
/// overwrites the previous value's scratch bytes.
#[inline(always)]
fn encode_wide_run<const LEN: usize>(
	buf: &mut [u8],
	offset: usize,
	chunk: &[u64; 8],
) -> Option<usize> {
	// The class is [2^(8 * (LEN - 2)), 2^(8 * (LEN - 1))).
	let lo = 1u64 << (8 * (LEN - 2));
	let width = (u64::MAX >> (8 * (9 - LEN))) - lo;
	if !chunk
		.iter()
		.fold(true, |acc, &v| acc & (v.wrapping_sub(lo) <= width))
	{
		return None;
	}
	let dst = buf.get_mut(offset..offset + 7 * LEN + 9)?;
	for (i, &v) in chunk.iter().enumerate() {
		let o = i * LEN;
		dst[o] = 0xF0 | (LEN - 2) as u8;
		dst[o + 1..o + 9].copy_from_slice(&v.to_le_bytes());
	}
	Some(offset + 8 * LEN)
}

/// How one encoded size lays out a value, as the little-endian word
/// `prefix | (v & low) | ((v >> shift) << 8)`: the first byte holds the
/// prefix bits and the value's low bits, and every later byte is the
/// next eight value bits. Binary length-prefix forms put all value bits
/// after the prefix byte (`low = 0`, `shift = 0`).
#[derive(Clone, Copy)]
struct Layout {
	prefix: u64,
	low: u64,
	shift: u32,
	len: u8,
}

/// Each encoded size's layout, indexed by a value's leading zeros
/// within `width` bits, for a type of at most nine encoded bytes.
const fn layouts<const N: usize>(width: u32) -> [Layout; N] {
	let mut table = [Layout {
		prefix: 0,
		low: 0,
		shift: 0,
		len: 0,
	}; N];
	let mut lz = 0;
	while lz < N {
		let bits = width - lz as u32;
		table[lz] = if bits <= 28 {
			// Prefix varint: len - 1 leading ones and a zero above
			// 8 - len value bits, then 8 value bits per byte.
			let len = if bits <= 7 { 1 } else { bits.div_ceil(7) };
			let low_bits = 8 - len;
			Layout {
				prefix: !(0xFF >> (len - 1)) & 0xFF,
				low: (1 << low_bits) - 1,
				shift: low_bits,
				len: len as u8,
			}
		} else {
			let len = bits.div_ceil(8) + 1;
			Layout {
				prefix: 0xF0 | (len as u64 - 2),
				low: 0,
				shift: 0,
				len: len as u8,
			}
		};
		lz += 1;
	}
	table
}

const U32_LAYOUTS: [Layout; 33] = layouts(32);
const U64_LAYOUTS: [Layout; 65] = layouts(64);

/// Encodes a window of eight values of mixed sizes without branching on
/// any of them: each value's layout comes from a table, its word is
/// stored whole at the running offset (the next store overwrites the
/// scratch bytes), and the offset advances by its length. Mixed sizes
/// would mispredict a per-value branch; this only waits on additions.
/// Returns `None` when the output lacks room for the full-width stores.
#[inline(always)]
fn encode_mixed_window_u32(
	buf: &mut [u8],
	offset: usize,
	chunk: &[u32; 8],
) -> Option<usize> {
	let dst = buf.get_mut(offset..offset + 7 * 5 + 8)?;
	let mut o = 0;
	for &v in chunk {
		let layout = U32_LAYOUTS[v.leading_zeros() as usize];
		let v = v as u64;
		let word =
			layout.prefix | (v & layout.low) | ((v >> layout.shift) << 8);
		dst[o..o + 8].copy_from_slice(&word.to_le_bytes());
		o += layout.len as usize;
	}
	Some(offset + o)
}

/// The `u64` form of [`encode_mixed_window_u32`]; a nine-byte encoding
/// also stores the value's top byte after the word.
#[inline(always)]
fn encode_mixed_window_u64(
	buf: &mut [u8],
	offset: usize,
	chunk: &[u64; 8],
) -> Option<usize> {
	let dst = buf.get_mut(offset..offset + 7 * 9 + 9)?;
	let mut o = 0;
	for &v in chunk {
		let layout = U64_LAYOUTS[v.leading_zeros() as usize];
		let word =
			layout.prefix | (v & layout.low) | ((v >> layout.shift) << 8);
		dst[o..o + 8].copy_from_slice(&word.to_le_bytes());
		dst[o + 8] = (v >> 56) as u8;
		o += layout.len as usize;
	}
	Some(offset + o)
}

/// Generates the specialized bulk codec for one unsigned width on top
/// of its window run functions.
macro_rules! bulk_unsigned {
	(
		$(#[$enc_docs:meta])* $enc_name:ident,
		$(#[$dec_docs:meta])* $dec_name:ident,
		$ut:ident, $try_enc:ident, $try_dec:ident, $mixed_enc:ident
	) => {
		$(#[$enc_docs])*
		pub fn $enc_name(buf: &mut [u8], values: &[$ut]) -> Result<usize> {
			let mut offset = 0;
			let mut i = 0;
			while i < values.len() {
				if let Some(chunk) = values.get(i..i + 8) {
					let chunk: &[$ut; 8] = chunk.try_into().unwrap();
					if let Some(new_offset) = $try_enc(buf, offset, chunk)
					{
						offset = new_offset;
						i += 8;
						continue;
					}
					// The window mixes size classes: encode it without
					// branching on sizes, or one value at a time where
					// the output is too short for full-width stores.
					if let Some(new_offset) = $mixed_enc(buf, offset, chunk)
					{
						offset = new_offset;
					} else {
						for &value in chunk {
							offset += value.encode(&mut buf[offset..])?;
						}
					}
					i += 8;
					continue;
				}
				// Tail shorter than a window.
				offset += values[i].encode(&mut buf[offset..])?;
				i += 1;
			}
			Ok(offset)
		}

		$(#[$dec_docs])*
		pub fn $dec_name(buf: &[u8], out: &mut [$ut]) -> Result<usize> {
			let mut offset = 0;
			let mut i = 0;
			while i < out.len() {
				if let Some(slots) = out.get_mut(i..i + 8) {
					let slots: &mut [$ut; 8] =
						slots.try_into().unwrap();
					if let Some((n, new_offset)) =
						$try_dec(buf, offset, slots)
					{
						offset = new_offset;
						i += n;
						continue;
					}
					// No run at this position: decode the next eight
					// values one at a time so the failed checks are
					// amortized across eight values.
					for slot in slots {
						let (value, len) = <$ut>::decode(&buf[offset..])?;
						*slot = value;
						offset += len;
					}
					i += 8;
					continue;
				}
				// Tail shorter than a window.
				let (value, len) = <$ut>::decode(&buf[offset..])?;
				out[i] = value;
				offset += len;
				i += 1;
			}
			Ok(offset)
		}
	};
}

bulk_unsigned! {
	/// Encodes a slice of `u32` values into `buf`, returning the total
	/// encoded length.
	///
	/// Produces exactly the same bytes as [`bulk_encode`], with fast
	/// paths for runs of equal-length encodings.
	bulk_encode_u32,
	/// Decodes exactly `out.len()` `u32` values from `buf`, returning
	/// the number of bytes consumed.
	///
	/// Accepts exactly the streams [`bulk_decode`] accepts, with fast
	/// paths for runs of equal-length encodings.
	bulk_decode_u32,
	u32, try_encode_run_u32, try_decode_run_u32, encode_mixed_window_u32
}

bulk_unsigned! {
	/// Encodes a slice of `u64` values into `buf`, returning the total
	/// encoded length.
	///
	/// Produces exactly the same bytes as [`bulk_encode`], with fast
	/// paths for runs of equal-length encodings.
	bulk_encode_u64,
	/// Decodes exactly `out.len()` `u64` values from `buf`, returning
	/// the number of bytes consumed.
	///
	/// Accepts exactly the streams [`bulk_decode`] accepts, with fast
	/// paths for runs of equal-length encodings.
	bulk_decode_u64,
	u64, try_encode_run_u64, try_decode_run_u64, encode_mixed_window_u64
}

/// Generates the specialized bulk codec for a signed width: zigzag
/// each window into its unsigned representation, reuse the unsigned
/// run machinery, and map back on the way out. Delta-encoded streams
/// are the sweet spot — small magnitudes land in the one- and
/// two-byte run classes.
macro_rules! bulk_signed {
	(
		$(#[$enc_docs:meta])* $enc_name:ident,
		$(#[$dec_docs:meta])* $dec_name:ident,
		$it:ident, $ut:ident, $try_enc:ident, $try_dec:ident, $mixed_enc:ident
	) => {
		$(#[$enc_docs])*
		pub fn $enc_name(buf: &mut [u8], values: &[$it]) -> Result<usize> {
			#[inline(always)]
			fn zigzag(v: $it) -> $ut {
				((v >> (<$it>::BITS - 1)) as $ut) ^ ((v << 1) as $ut)
			}

			let mut offset = 0;
			let mut i = 0;
			while i < values.len() {
				if let Some(chunk) = values.get(i..i + 8) {
					let mut mapped = [0; 8];
					for (z, &v) in mapped.iter_mut().zip(chunk) {
						*z = zigzag(v);
					}
					// A window can only be a run if its first and
					// last values encode at the same length.
					if mapped[0].encoded_size() == mapped[7].encoded_size() {
						if let Some(new_offset) =
							$try_enc(buf, offset, &mapped)
						{
							offset = new_offset;
							i += 8;
							continue;
						}
					}
					if let Some(new_offset) = $mixed_enc(buf, offset, &mapped)
					{
						offset = new_offset;
					} else {
						for &value in chunk {
							offset += value.encode(&mut buf[offset..])?;
						}
					}
					i += 8;
					continue;
				}
				offset += values[i].encode(&mut buf[offset..])?;
				i += 1;
			}
			Ok(offset)
		}

		$(#[$dec_docs])*
		pub fn $dec_name(buf: &[u8], out: &mut [$it]) -> Result<usize> {
			let mut offset = 0;
			let mut i = 0;
			while i < out.len() {
				if i + 8 <= out.len() {
					let mut zigzag: [$ut; 8] = [0; 8];
					if let Some((n, new_offset)) =
						$try_dec(buf, offset, &mut zigzag)
					{
						for (slot, &z) in
							out[i..i + n].iter_mut().zip(&zigzag[..n])
						{
							*slot = ((z >> 1) as $it)
								^ (-((z & 1) as $it));
						}
						offset = new_offset;
						i += n;
						continue;
					}
					for slot in out[i..i + 8].iter_mut() {
						let (value, len) = <$it>::decode(&buf[offset..])?;
						*slot = value;
						offset += len;
					}
					i += 8;
					continue;
				}
				let (value, len) = <$it>::decode(&buf[offset..])?;
				out[i] = value;
				offset += len;
				i += 1;
			}
			Ok(offset)
		}
	};
}

bulk_signed! {
	/// Encodes a slice of `i32` values into `buf`, returning the total
	/// encoded length.
	///
	/// Produces exactly the same bytes as [`bulk_encode`], with fast
	/// paths for runs of equal-length encodings; small magnitudes
	/// (delta streams) hit the fastest paths.
	bulk_encode_i32,
	/// Decodes exactly `out.len()` `i32` values from `buf`, returning
	/// the number of bytes consumed.
	///
	/// Accepts exactly the streams [`bulk_decode`] accepts, with fast
	/// paths for runs of equal-length encodings.
	bulk_decode_i32,
	i32, u32, try_encode_run_u32, try_decode_run_u32, encode_mixed_window_u32
}

bulk_signed! {
	/// Encodes a slice of `i64` values into `buf`, returning the total
	/// encoded length.
	///
	/// Produces exactly the same bytes as [`bulk_encode`], with fast
	/// paths for runs of equal-length encodings; small magnitudes
	/// (delta streams) hit the fastest paths.
	bulk_encode_i64,
	/// Decodes exactly `out.len()` `i64` values from `buf`, returning
	/// the number of bytes consumed.
	///
	/// Accepts exactly the streams [`bulk_decode`] accepts, with fast
	/// paths for runs of equal-length encodings.
	bulk_decode_i64,
	i64, u64, try_encode_run_u64, try_decode_run_u64, encode_mixed_window_u64
}

/// Returns an iterator that decodes consecutive values from `buf`.
///
/// The iterator yields `Ok(value)` for each decoded value, ends after
/// the final complete value, and yields a single `Err` then stops if
/// the stream is invalid or truncated.
#[must_use]
pub fn decode_iter<T: Decode>(buf: &[u8]) -> DecodeIter<'_, T> {
	DecodeIter {
		buf,
		offset: 0,
		failed: false,
		_marker: core::marker::PhantomData,
	}
}

/// Iterator over the values in an encoded stream. See [`decode_iter`].
#[derive(Debug, Clone)]
pub struct DecodeIter<'a, T> {
	buf: &'a [u8],
	offset: usize,
	failed: bool,
	_marker: core::marker::PhantomData<fn() -> T>,
}

impl<'a, T> DecodeIter<'a, T> {
	/// The byte offset of the next value in the underlying buffer.
	#[must_use]
	pub fn offset(&self) -> usize {
		self.offset
	}
}

impl<'a, T: Decode> Iterator for DecodeIter<'a, T> {
	type Item = Result<T>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.failed || self.offset >= self.buf.len() {
			return None;
		}
		match T::decode(&self.buf[self.offset..]) {
			Ok((value, len)) => {
				self.offset += len;
				Some(Ok(value))
			},
			Err(err) => {
				self.failed = true;
				Some(Err(err))
			},
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let remaining = self.buf.len() - self.offset;
		if self.failed || remaining == 0 {
			(0, Some(0))
		} else {
			// An unvalidated tail guarantees only one item: its first
			// prefix may produce the iterator's terminal error.
			(1, Some(remaining))
		}
	}
}

impl<'a, T: Decode> core::iter::FusedIterator for DecodeIter<'a, T> {}

/// Generates a run-accelerated decoding iterator for one unsigned
/// width: an internal window of up to eight values is refilled through
/// the bulk run fast paths, and `next()` serves from it with an index
/// bump. Falls back to the checked scalar decoder between runs.
macro_rules! run_iter {
	(
		$(#[$fn_docs:meta])* $fn_name:ident,
		$(#[$ty_docs:meta])* $ty_name:ident,
		$ut:ident, $try_dec:ident
	) => {
		$(#[$fn_docs])*
		#[must_use]
		pub fn $fn_name(buf: &[u8]) -> $ty_name<'_> {
			$ty_name {
				buf,
				offset: 0,
				pending: [0; 8],
				head: 0,
				len: 0,
				failed: false,
			}
		}

		$(#[$ty_docs])*
		#[derive(Debug, Clone)]
		pub struct $ty_name<'a> {
			buf: &'a [u8],
			offset: usize,
			pending: [$ut; 8],
			head: u8,
			len: u8,
			failed: bool,
		}

		impl<'a> Iterator for $ty_name<'a> {
			type Item = Result<$ut>;

			#[inline]
			fn next(&mut self) -> Option<Self::Item> {
				if self.head < self.len {
					let value = self.pending[self.head as usize];
					self.head += 1;
					return Some(Ok(value));
				}
				if self.failed || self.offset >= self.buf.len() {
					return None;
				}
				if let Some((n, new_offset)) =
					$try_dec(self.buf, self.offset, &mut self.pending)
				{
					self.offset = new_offset;
					self.head = 1;
					self.len = n as u8;
					return Some(Ok(self.pending[0]));
				}
				match <$ut>::decode(&self.buf[self.offset..]) {
					Ok((value, len)) => {
						self.offset += len;
						Some(Ok(value))
					},
					Err(err) => {
						self.failed = true;
						Some(Err(err))
					},
				}
			}

			fn size_hint(&self) -> (usize, Option<usize>) {
				let buffered = (self.len - self.head) as usize;
				if self.failed {
					return (buffered, Some(buffered));
				}
				let remaining = self.buf.len() - self.offset;
				let tail = usize::from(remaining != 0);
				(
					buffered + tail,
					Some(buffered + remaining),
				)
			}
		}

		impl<'a> core::iter::FusedIterator for $ty_name<'a> {}
	};
}

run_iter! {
	/// Returns a run-accelerated iterator decoding consecutive `u32`
	/// values from `buf`.
	///
	/// Behaves like [`decode_iter`], but refills an internal window
	/// through the same run fast paths as [`bulk_decode_u32`], so runs
	/// of similarly-sized values decode several at a time. Unlike
	/// [`DecodeIter`] it buffers ahead, so it exposes no byte offset.
	decode_iter_u32,
	/// Run-accelerated iterator over `u32` values. See
	/// [`decode_iter_u32`].
	DecodeIterU32,
	u32, try_decode_run_u32
}

run_iter! {
	/// Returns a run-accelerated iterator decoding consecutive `u64`
	/// values from `buf`.
	///
	/// Behaves like [`decode_iter`], but refills an internal window
	/// through the same run fast paths as [`bulk_decode_u64`], so runs
	/// of similarly-sized values decode several at a time. Unlike
	/// [`DecodeIter`] it buffers ahead, so it exposes no byte offset.
	decode_iter_u64,
	/// Run-accelerated iterator over `u64` values. See
	/// [`decode_iter_u64`].
	DecodeIterU64,
	u64, try_decode_run_u64
}

/// Generates the signed run-accelerated iterator: refills through the
/// unsigned run machinery and zigzag-maps into the pending window.
macro_rules! run_iter_signed {
	(
		$(#[$fn_docs:meta])* $fn_name:ident,
		$(#[$ty_docs:meta])* $ty_name:ident,
		$it:ident, $ut:ident, $try_dec:ident
	) => {
		$(#[$fn_docs])*
		#[must_use]
		pub fn $fn_name(buf: &[u8]) -> $ty_name<'_> {
			$ty_name {
				buf,
				offset: 0,
				pending: [0; 8],
				head: 0,
				len: 0,
				failed: false,
			}
		}

		$(#[$ty_docs])*
		#[derive(Debug, Clone)]
		pub struct $ty_name<'a> {
			buf: &'a [u8],
			offset: usize,
			pending: [$it; 8],
			head: u8,
			len: u8,
			failed: bool,
		}

		impl<'a> Iterator for $ty_name<'a> {
			type Item = Result<$it>;

			#[inline]
			fn next(&mut self) -> Option<Self::Item> {
				if self.head < self.len {
					let value = self.pending[self.head as usize];
					self.head += 1;
					return Some(Ok(value));
				}
				if self.failed || self.offset >= self.buf.len() {
					return None;
				}
				let mut raw: [$ut; 8] = [0; 8];
				if let Some((n, new_offset)) =
					$try_dec(self.buf, self.offset, &mut raw)
				{
					for (slot, &z) in
						self.pending[..n].iter_mut().zip(&raw[..n])
					{
						*slot = ((z >> 1) as $it) ^ (-((z & 1) as $it));
					}
					self.offset = new_offset;
					self.head = 1;
					self.len = n as u8;
					return Some(Ok(self.pending[0]));
				}
				match <$it>::decode(&self.buf[self.offset..]) {
					Ok((value, len)) => {
						self.offset += len;
						Some(Ok(value))
					},
					Err(err) => {
						self.failed = true;
						Some(Err(err))
					},
				}
			}

			fn size_hint(&self) -> (usize, Option<usize>) {
				let buffered = (self.len - self.head) as usize;
				if self.failed {
					return (buffered, Some(buffered));
				}
				let remaining = self.buf.len() - self.offset;
				let tail = usize::from(remaining != 0);
				(
					buffered + tail,
					Some(buffered + remaining),
				)
			}
		}

		impl<'a> core::iter::FusedIterator for $ty_name<'a> {}
	};
}

run_iter_signed! {
	/// Returns a run-accelerated iterator decoding consecutive `i32`
	/// values from `buf`.
	///
	/// Behaves like [`decode_iter`], with the same run fast paths as
	/// [`bulk_decode_i32`]; delta-encoded streams are the sweet spot.
	decode_iter_i32,
	/// Run-accelerated iterator over `i32` values. See
	/// [`decode_iter_i32`].
	DecodeIterI32,
	i32, u32, try_decode_run_u32
}

run_iter_signed! {
	/// Returns a run-accelerated iterator decoding consecutive `i64`
	/// values from `buf`.
	///
	/// Behaves like [`decode_iter`], with the same run fast paths as
	/// [`bulk_decode_i64`]; delta-encoded streams are the sweet spot.
	decode_iter_i64,
	/// Run-accelerated iterator over `i64` values. See
	/// [`decode_iter_i64`].
	DecodeIterI64,
	i64, u64, try_decode_run_u64
}

/// Generates the run-accelerated `Decode::decode_to_vec` for one type:
/// whole runs are appended from the window fast paths, and a position
/// without a run decodes up to eight values one at a time so the failed
/// check is amortized, as in the bulk decoders. `$map` turns each
/// window lane into a value.
#[cfg(feature = "alloc")]
macro_rules! decode_to_vec_fn {
	($name:ident, $t:ident, $ut:ident, $try_dec:ident, $map:expr) => {
		pub(crate) fn $name(buf: &[u8]) -> Result<alloc::vec::Vec<$t>> {
			let map: fn($ut) -> $t = $map;
			// Every encoding fits in MAX_ENCODED_SIZE bytes, so this
			// many values at least are coming.
			let mut out = alloc::vec::Vec::with_capacity(
				buf.len().div_ceil(<$t as Decode>::MAX_ENCODED_SIZE),
			);
			let mut offset = 0;
			while offset < buf.len() {
				let mut lanes: [$ut; 8] = [0; 8];
				if let Some((n, next)) = $try_dec(buf, offset, &mut lanes) {
					out.extend(lanes[..n].iter().map(|&lane| map(lane)));
					offset = next;
					continue;
				}
				for _ in 0..8 {
					if offset >= buf.len() {
						break;
					}
					let (value, len) = <$t>::decode(&buf[offset..])?;
					out.push(value);
					offset += len;
				}
			}
			Ok(out)
		}
	};
}

#[cfg(feature = "alloc")]
decode_to_vec_fn!(decode_to_vec_u32, u32, u32, try_decode_run_u32, |v| v);
#[cfg(feature = "alloc")]
decode_to_vec_fn!(decode_to_vec_u64, u64, u64, try_decode_run_u64, |v| v);
#[cfg(feature = "alloc")]
decode_to_vec_fn!(decode_to_vec_i32, i32, u32, try_decode_run_u32, |z| {
	((z >> 1) as i32) ^ -((z & 1) as i32)
});
#[cfg(feature = "alloc")]
decode_to_vec_fn!(decode_to_vec_i64, i64, u64, try_decode_run_u64, |z| {
	((z >> 1) as i64) ^ -((z & 1) as i64)
});
