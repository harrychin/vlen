//! Sequential cursors for encoding and decoding mixed-type messages
//! without manual offset bookkeeping.

use crate::decode::Decode;
use crate::encode::Encode;
use crate::error::Result;

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
	/// was already written.
	pub fn write<T: Encode>(&mut self, value: T) -> Result<()> {
		let len = value.encode(&mut self.buf[self.pos..])?;
		self.pos += len;
		Ok(())
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
}
