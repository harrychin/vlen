//! Bulk encoding and decoding of many values.
//!
//! All functions here produce and consume the canonical vlen byte
//! stream — output is byte-for-byte identical to encoding each value
//! individually, and any mix of the bulk and per-value APIs
//! interoperates. The `u32`- and `u64`-specialized functions add SWAR
//! fast paths that process runs of one-byte encodings eight at a time
//! and runs of two-byte encodings four at a time; they are portable
//! safe Rust, active on every architecture. Prefer them when your data
//! leans toward small values; for adversarially mixed sizes the
//! generic functions are a few percent faster because they skip the
//! run detection.

use crate::decode::Decode;
use crate::encode::Encode;
use crate::error::Result;

/// Encodes a slice of values into `buf`, returning the total encoded
/// length.
///
/// The buffer only needs room for the actual encoded stream; encoding
/// stops with [`Error::BufferTooSmall`](crate::Error::BufferTooSmall)
/// if it runs out of space.
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

/// Generates the specialized bulk codec for one unsigned width, with
/// SWAR fast paths for runs of one-byte encodings (eight values per
/// step) and two-byte encodings (four values per step).
macro_rules! bulk_specialized {
	(
		$(#[$enc_docs:meta])* $enc_name:ident,
		$(#[$dec_docs:meta])* $dec_name:ident,
		$ut:ident
	) => {
		$(#[$enc_docs])*
		pub fn $enc_name(buf: &mut [u8], values: &[$ut]) -> Result<usize> {
			let mut offset = 0;
			let mut i = 0;
			while i < values.len() {
				if let Some(chunk) = values.get(i..i + 8) {
					// The first and last elements gate the run checks:
					// a window can only be a uniform run of some class
					// if both belong to that class, so windows that
					// obviously cannot match skip the reductions.
					let first = chunk[0];
					let last = chunk[7];
					if (first | last) < 0x80
						&& chunk.iter().fold(0, |acc, &v| acc | v) < 0x80
					{
						// Eight one-byte values become eight bytes.
						if let Some(dst) =
							buf.get_mut(offset..offset + 8)
						{
							for (d, &v) in dst.iter_mut().zip(chunk) {
								*d = v as u8;
							}
							offset += 8;
							i += 8;
							continue;
						}
					} else if (first.wrapping_sub(0x80)
						| last.wrapping_sub(0x80)) < 0x3F80
						&& chunk
							.iter()
							.fold(0, |acc, &v| acc | v.wrapping_sub(0x80))
							< 0x3F80
					{
						// One reduction checks both bounds: values
						// below 0x80 wrap to huge, values at or above
						// 0x4000 stay at 0x3F80 or more.
						// Eight two-byte values become two packed
						// words of four little-endian lanes each.
						if let Some(dst) =
							buf.get_mut(offset..offset + 16)
						{
							let mut half = 0;
							while half < 2 {
								let mut word = 0u64;
								let mut j = 0;
								while j < 4 {
									let v = chunk[half * 4 + j] as u64;
									let lane = 0x80
										| (v & 0x3F) | ((v >> 6) << 8);
									word |= lane << (16 * j);
									j += 1;
								}
								dst[half * 8..half * 8 + 8]
									.copy_from_slice(&word.to_le_bytes());
								half += 1;
							}
							offset += 16;
							i += 8;
							continue;
						}
					}
					// The window mixes size classes: encode it one
					// value at a time so the failed checks are
					// amortized across eight values.
					for &value in chunk {
						offset += value.encode(&mut buf[offset..])?;
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
				if let (Some(chunk), Some(slots)) =
					(buf.get(offset..offset + 8), out.get_mut(i..i + 8))
				{
					let word = u64::from_le_bytes(chunk.try_into().unwrap());
					if word & ONE_BYTE_RUN == 0 {
						// Eight one-byte encodings.
						for (slot, &b) in slots.iter_mut().zip(chunk) {
							*slot = b as $ut;
						}
						offset += 8;
						i += 8;
						continue;
					}
					if word & TWO_BYTE_MASK == TWO_BYTE_WANT {
						// Four two-byte encodings: reassemble all four
						// values inside 16-bit lanes at once.
						let lo = word & 0x003F_003F_003F_003F;
						let hi = (word >> 8) & 0x00FF_00FF_00FF_00FF;
						let packed = (hi << 6) | lo;
						slots[0] = (packed & 0xFFFF) as $ut;
						slots[1] = ((packed >> 16) & 0xFFFF) as $ut;
						slots[2] = ((packed >> 32) & 0xFFFF) as $ut;
						slots[3] = (packed >> 48) as $ut;
						offset += 8;
						i += 4;
						continue;
					}
					// The window holds larger encodings: decode the
					// next eight values one at a time so the failed
					// checks are amortized across eight values.
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

bulk_specialized! {
	/// Encodes a slice of `u32` values into `buf`, returning the total
	/// encoded length.
	///
	/// Produces exactly the same bytes as [`bulk_encode`], with SWAR
	/// fast paths that emit runs of one-byte encodings eight at a time
	/// and runs of two-byte encodings four at a time.
	bulk_encode_u32,
	/// Decodes exactly `out.len()` `u32` values from `buf`, returning
	/// the number of bytes consumed.
	///
	/// Accepts exactly the streams [`bulk_decode`] accepts, with SWAR
	/// fast paths that consume runs of one-byte encodings eight at a
	/// time and runs of two-byte encodings four at a time.
	bulk_decode_u32,
	u32
}

bulk_specialized! {
	/// Encodes a slice of `u64` values into `buf`, returning the total
	/// encoded length.
	///
	/// Produces exactly the same bytes as [`bulk_encode`], with SWAR
	/// fast paths that emit runs of one-byte encodings eight at a time
	/// and runs of two-byte encodings four at a time.
	bulk_encode_u64,
	/// Decodes exactly `out.len()` `u64` values from `buf`, returning
	/// the number of bytes consumed.
	///
	/// Accepts exactly the streams [`bulk_decode`] accepts, with SWAR
	/// fast paths that consume runs of one-byte encodings eight at a
	/// time and runs of two-byte encodings four at a time.
	bulk_decode_u64,
	u64
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
			// Every value occupies between 1 and MAX_ENCODED_SIZE bytes.
			(remaining.div_ceil(T::MAX_ENCODED_SIZE), Some(remaining))
		}
	}
}

impl<'a, T: Decode> core::iter::FusedIterator for DecodeIter<'a, T> {}
