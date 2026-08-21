//! Bulk encoding and decoding of many values.
//!
//! All functions here produce and consume the canonical vlen byte
//! stream — output is byte-for-byte identical to encoding each value
//! individually, and any mix of the bulk and per-value APIs
//! interoperates. The `u32`-specialized functions add a SWAR fast path
//! that processes runs of one-byte encodings eight at a time; it is
//! portable safe Rust, active on every architecture.

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

/// Encodes a slice of `u32` values into `buf`, returning the total
/// encoded length.
///
/// Produces exactly the same bytes as [`bulk_encode`], with a fast path
/// that emits runs of eight one-byte encodings at once.
pub fn bulk_encode_u32(buf: &mut [u8], values: &[u32]) -> Result<usize> {
	let mut offset = 0;
	let mut i = 0;
	while i < values.len() {
		// Fast path: eight one-byte values become eight bytes.
		if let (Some(chunk), Some(dst)) =
			(values.get(i..i + 8), buf.get_mut(offset..offset + 8))
		{
			if chunk.iter().fold(0, |acc, &v| acc | v) < 0x80 {
				for (d, &v) in dst.iter_mut().zip(chunk) {
					*d = v as u8;
				}
				offset += 8;
				i += 8;
				continue;
			}
			// The window holds a multi-byte value: encode the whole
			// window one value at a time so the failed check is
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

/// Decodes exactly `out.len()` `u32` values from `buf`, returning the
/// number of bytes consumed.
///
/// Accepts exactly the streams [`bulk_decode`] accepts, with a fast
/// path that consumes runs of eight one-byte encodings at once.
pub fn bulk_decode_u32(buf: &[u8], out: &mut [u32]) -> Result<usize> {
	let mut offset = 0;
	let mut i = 0;
	while i < out.len() {
		// Fast path: eight bytes with clear continuation bits are
		// eight one-byte encodings.
		if let (Some(chunk), Some(slots)) =
			(buf.get(offset..offset + 8), out.get_mut(i..i + 8))
		{
			let word = u64::from_le_bytes(chunk.try_into().unwrap());
			if word & 0x8080_8080_8080_8080 == 0 {
				for (slot, &b) in slots.iter_mut().zip(chunk) {
					*slot = b as u32;
				}
				offset += 8;
				i += 8;
				continue;
			}
			// The window holds a multi-byte encoding: decode the next
			// eight values one at a time so the failed check is
			// amortized across eight values.
			for slot in slots {
				let (value, len) = u32::decode(&buf[offset..])?;
				*slot = value;
				offset += len;
			}
			i += 8;
			continue;
		}
		// Tail shorter than a window.
		let (value, len) = u32::decode(&buf[offset..])?;
		out[i] = value;
		offset += len;
		i += 1;
	}
	Ok(offset)
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
