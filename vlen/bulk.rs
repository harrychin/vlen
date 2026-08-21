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
/// Four-byte encodings start `1110xxxx`: top four bits of bytes 0
/// and 4.
const FOUR_BYTE_MASK: u64 = 0x0000_00F0_0000_00F0;
const FOUR_BYTE_WANT: u64 = 0x0000_00E0_0000_00E0;

/// Generates the specialized bulk codec for one unsigned width.
///
/// Both directions detect runs of equal-length encodings, whose
/// boundaries are known in advance and therefore need no per-value
/// branching: one- and two-byte runs move through SWAR lanes, and
/// three- to five-byte runs use class-known constructions — a single
/// full-width store or masked load per value with no length
/// computation at all. Windows without a run fall back to the branchy
/// scalar codec, which branch prediction serves best.
macro_rules! bulk_specialized {
	(
		$(#[$enc_docs:meta])* $enc_name:ident,
		$(#[$dec_docs:meta])* $dec_name:ident,
		$ut:ident
	) => {
		$(#[$enc_docs])*
		pub fn $enc_name(buf: &mut [u8], values: &[$ut]) -> Result<usize> {
			// Verifies that all eight values sit in the class
			// `[lo, lo + span)` with one or-reduction: values below
			// `lo` wrap to huge, values at or beyond the class stay
			// at `span` or more.
			#[inline(always)]
			fn in_class(chunk: &[$ut], lo: $ut, span: $ut) -> bool {
				chunk
					.iter()
					.fold(0, |acc, &v| acc | v.wrapping_sub(lo))
					< span
			}

			let mut offset = 0;
			let mut i = 0;
			while i < values.len() {
				if let Some(chunk) = values.get(i..i + 8) {
					// The two hottest classes get dedicated cheap gates;
					// wider classes share one dispatch on the encoded
					// size of the window's first and last values.
					let first = chunk[0];
					let last = chunk[7];
					if (first | last) < 0x80
						&& in_class(chunk, 0, 0x80)
					{
						// Eight one-byte values in one step.
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
						&& in_class(chunk, 0x80, 0x3F80)
					{
						// Eight two-byte values as two packed words of
						// four little-endian lanes.
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
									.copy_from_slice(
										&word.to_le_bytes(),
									);
								half += 1;
							}
							offset += 16;
							i += 8;
							continue;
						}
					} else if first >= 0x4000
						&& first.encoded_size() == last.encoded_size()
					{
						match first.encoded_size() {
							3 if in_class(chunk, 0x4000, 0x1F_C000) => {
								// One full-width store per value at a
								// three-byte stride; the next store
								// overwrites the scratch bytes.
								if let Some(dst) = buf
									.get_mut(offset..offset + 3 * 8 + 5)
								{
									let mut o = 0;
									for &v in chunk {
										let word = (((v >> 5) as u64)
											<< 8) | (0xC0
											| ((v & 0x1F) as u64));
										dst[o..o + 8].copy_from_slice(
											&word.to_le_bytes(),
										);
										o += 3;
									}
									offset += 3 * 8;
									i += 8;
									continue;
								}
							},
							4 if in_class(
								chunk, 0x20_0000, 0xFE0_0000,
							) =>
							{
								if let Some(dst) = buf
									.get_mut(offset..offset + 4 * 8 + 4)
								{
									let mut o = 0;
									for &v in chunk {
										let word = (((v >> 4) as u64)
											<< 8) | (0xE0
											| ((v & 0x0F) as u64));
										dst[o..o + 8].copy_from_slice(
											&word.to_le_bytes(),
										);
										o += 4;
									}
									offset += 4 * 8;
									i += 8;
									continue;
								}
							},
							5 if in_class(
								chunk,
								0x1000_0000,
								0xF000_0000,
							) =>
							{
								if let Some(dst) = buf
									.get_mut(offset..offset + 5 * 8 + 3)
								{
									let mut o = 0;
									for &v in chunk {
										let word =
											((v as u64) << 8) | 0xF3;
										dst[o..o + 8].copy_from_slice(
											&word.to_le_bytes(),
										);
										o += 5;
									}
									offset += 5 * 8;
									i += 8;
									continue;
								}
							},
							_ => {},
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
					// Dispatch on the first encoding's size class; each
					// class runs at most one run test.
					let b0 = (word & 0xFF) as u8;
					if b0 < 0x80 {
						if word & ONE_BYTE_RUN == 0 {
							// Eight one-byte encodings.
							for (slot, &b) in slots.iter_mut().zip(chunk) {
								*slot = b as $ut;
							}
							offset += 8;
							i += 8;
							continue;
						}
					} else if b0 < 0xC0 {
						if word & TWO_BYTE_MASK == TWO_BYTE_WANT {
							// Four two-byte encodings: reassemble all
							// four values inside 16-bit lanes at once.
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
					} else if (0xE0..0xF0).contains(&b0) {
						if word & FOUR_BYTE_MASK == FOUR_BYTE_WANT {
							// Two four-byte encodings in two 32-bit
							// lanes: value bits sit above the prefix
							// nibble.
							let lane0 = word & 0xFFFF_FFFF;
							let lane1 = word >> 32;
							slots[0] = (((lane0 >> 8) << 4)
								| (lane0 & 0x0F)) as $ut;
							slots[1] = (((lane1 >> 8) << 4)
								| (lane1 & 0x0F)) as $ut;
							offset += 8;
							i += 2;
							continue;
						}
					} else if b0 >= 0xF0 {
						// Binary length prefix: decode pairs of
						// equal-length encodings with plain masked
						// loads, no length arithmetic per value.
						const WIDTH: usize = core::mem::size_of::<$ut>();
						let len = ((b0 & 0x0F) as usize) + 2;
						if len <= WIDTH + 1 {
							if let Some(pair) =
								buf.get(offset..offset + 2 * len)
							{
								if pair[len] == b0
									&& offset + len + 1 + WIDTH
										<= buf.len()
								{
									let payload = len - 1;
									let mask = if payload >= WIDTH {
										$ut::MAX
									} else {
										$ut::MAX
											>> ((WIDTH - payload) * 8)
									};
									let lo = $ut::from_le_bytes(
										buf[offset + 1
											..offset + 1 + WIDTH]
											.try_into()
											.unwrap(),
									);
									let hi = $ut::from_le_bytes(
										buf[offset + len + 1
											..offset + len + 1 + WIDTH]
											.try_into()
											.unwrap(),
									);
									slots[0] = lo & mask;
									slots[1] = hi & mask;
									offset += 2 * len;
									i += 2;
									continue;
								}
							}
						}
					}
					// No run at this position (three-byte runs land
					// here on purpose: uniform runs predict perfectly,
					// and their SWAR lane math costs more than the
					// branchy scalar path): decode the next eight
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

bulk_specialized! {
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
	u32
}

bulk_specialized! {
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
