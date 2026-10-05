//! Sequential cursors for encoding and decoding mixed-type messages
//! without manual offset bookkeeping.

use crate::decode::{Decode, decode_canonical};
use crate::encode::{Encode, encode_padded};
use crate::error::{Error, Result, StrictError, StrictResult};

/// Writes consecutive values into a byte slice, tracking the position.
///
/// ```rust
/// let mut buf = [0u8; 16];
/// let mut writer = vlen::Writer::new(&mut buf);
/// writer.write(7u32)?;
/// writer.write(-42i64)?;
/// let len = writer.finish();
/// assert_eq!(len, 2); // both values fit in one byte each
/// # Ok::<(), vlen::Error>(())
/// ```
#[derive(Debug)]
pub struct Writer<'a> {
	buf: &'a mut [u8],
	pos: usize,
}

impl<'a> Writer<'a> {
	/// Starts writing at the beginning of `buf`.
	#[must_use]
	pub fn new(buf: &'a mut [u8]) -> Self {
		Writer { buf, pos: 0 }
	}

	/// Encodes `value` at the current position and advances past it.
	///
	/// On error the position is unchanged, so a failed write can be
	/// retried into a larger buffer or reported without losing what
	/// was already written. On success, bytes past the new position
	/// may be overwritten with scratch data.
	pub fn write<T: Encode>(&mut self, value: T) -> Result<()> {
		let len = value.encode(&mut self.buf[self.pos..])?;
		self.pos += len;
		Ok(())
	}

	/// Reserves `N` zeroed bytes at the current position for a value
	/// written later with [`fill`](Writer::fill), and advances past them.
	///
	/// On error the position is unchanged. `N` must be between 1 and 17
	/// (the widest encoding), which is checked at compile time.
	///
	/// ```rust
	/// let mut buf = [0u8; 16];
	/// let mut writer = vlen::Writer::new(&mut buf);
	/// let body_len = writer.reserve::<2>()?;
	/// writer.write(7u32)?;
	/// writer.write(-42i64)?;
	/// let len = writer.position() - body_len.end();
	/// writer.fill(body_len, len)?;
	/// let total = writer.finish();
	///
	/// let mut reader = vlen::Reader::new(&buf[..total]);
	/// assert_eq!(reader.read::<usize>()?, 2);
	/// assert_eq!(reader.read::<u32>()?, 7);
	/// assert_eq!(reader.read::<i64>()?, -42);
	/// # Ok::<(), vlen::Error>(())
	/// ```
	pub fn reserve<const N: usize>(&mut self) -> Result<Slot<N>> {
		const {
			assert!(N >= 1 && N <= 17, "slot width must be between 1 and 17");
		}
		let start = self.pos;
		match self.buf.get_mut(start..).and_then(<[u8]>::first_chunk_mut) {
			Some(slot) => {
				*slot = [0; N];
				self.pos += N;
				Ok(Slot { start })
			},
			None => Err(Error::BufferTooSmall {
				needed: N,
				available: self.remaining(),
			}),
		}
	}

	/// Fills a slot from [`reserve`](Writer::reserve) with `value`,
	/// padded to the slot's width as by
	/// [`encode_padded`](crate::encode_padded).
	///
	/// Fails with [`Error::BufferTooSmall`] if `value` needs more than
	/// `N` bytes, leaving the slot unchanged. `N` must not exceed
	/// `T::MAX_ENCODED_SIZE`, which is checked at compile time.
	pub fn fill<T: Encode, const N: usize>(
		&mut self,
		slot: Slot<N>,
		value: T,
	) -> Result<()> {
		match self
			.buf
			.get_mut(slot.start..)
			.and_then(<[u8]>::first_chunk_mut::<N>)
		{
			Some(dst) => encode_padded(dst, value),
			// Only a slot reserved in a longer buffer lands here.
			None => Err(Error::BufferTooSmall {
				needed: slot.end(),
				available: self.buf.len(),
			}),
		}
	}

	/// The number of bytes written so far.
	#[must_use]
	pub fn position(&self) -> usize {
		self.pos
	}

	/// The number of bytes still available.
	#[must_use]
	pub fn remaining(&self) -> usize {
		self.buf.len() - self.pos
	}

	/// The bytes written so far.
	#[must_use]
	pub fn written(&self) -> &[u8] {
		&self.buf[..self.pos]
	}

	/// Consumes the writer, returning the number of bytes written.
	#[must_use]
	pub fn finish(self) -> usize {
		self.pos
	}
}

/// A fixed-width slot reserved by [`Writer::reserve`], to be filled
/// with [`Writer::fill`] once its value is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "a reserved slot stays zeroed until it is filled"]
pub struct Slot<const N: usize> {
	start: usize,
}

impl<const N: usize> Slot<N> {
	/// The offset of the slot's first byte in the writer's buffer.
	#[must_use]
	pub fn start(&self) -> usize {
		self.start
	}

	/// The offset just past the slot's last byte.
	#[must_use]
	pub fn end(&self) -> usize {
		self.start + N
	}
}

/// Reads consecutive values from a byte slice, tracking the position.
///
/// ```rust
/// let mut buf = [0u8; 16];
/// let mut writer = vlen::Writer::new(&mut buf);
/// writer.write(7u32)?;
/// writer.write(-42i64)?;
/// let len = writer.finish();
///
/// let mut reader = vlen::Reader::new(&buf[..len]);
/// assert_eq!(reader.read::<u32>()?, 7);
/// assert_eq!(reader.read::<i64>()?, -42);
/// assert!(reader.is_empty());
/// # Ok::<(), vlen::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct Reader<'a> {
	buf: &'a [u8],
	pos: usize,
}

impl<'a> Reader<'a> {
	/// Starts reading at the beginning of `buf`.
	#[must_use]
	pub fn new(buf: &'a [u8]) -> Self {
		Reader { buf, pos: 0 }
	}

	/// Decodes the next value and advances past it.
	///
	/// On error the position is unchanged.
	pub fn read<T: Decode>(&mut self) -> Result<T> {
		let (value, len) = T::decode(&self.buf[self.pos..])?;
		self.pos += len;
		Ok(value)
	}

	/// Decodes the next value only if its encoding is canonical.
	///
	/// On error the position is unchanged.
	pub fn read_canonical<T: Decode + Encode>(&mut self) -> StrictResult<T> {
		let (value, len) = decode_canonical(&self.buf[self.pos..])?;
		self.pos += len;
		Ok(value)
	}

	/// The number of bytes consumed so far.
	#[must_use]
	pub fn position(&self) -> usize {
		self.pos
	}

	/// The number of bytes not yet consumed.
	#[must_use]
	pub fn remaining(&self) -> usize {
		self.buf.len() - self.pos
	}

	/// Whether every byte has been consumed.
	#[must_use]
	pub fn is_empty(&self) -> bool {
		self.pos >= self.buf.len()
	}

	/// The unread remainder of the buffer.
	#[must_use]
	pub fn remaining_bytes(&self) -> &'a [u8] {
		&self.buf[self.pos..]
	}

	/// Consumes the reader and verifies that every input byte was read.
	///
	/// Returns [`StrictError::TrailingBytes`] when unread input remains.
	pub fn finish(self) -> StrictResult<()> {
		if self.pos == self.buf.len() {
			Ok(())
		} else {
			Err(StrictError::TrailingBytes {
				consumed: self.pos,
				available: self.buf.len(),
			})
		}
	}
}
