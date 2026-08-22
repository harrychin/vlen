//! Error type for the checked encoding and decoding APIs.

/// Errors returned by the checked (slice-based) encode and decode APIs.
///
/// The infallible array-based functions ([`encode_u32`](crate::encode_u32)
/// and friends) never produce these; they place the buffer-size burden on
/// the caller through their array parameter types instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Error {
	/// The buffer is too small to hold or supply the encoded value.
	BufferTooSmall {
		/// Minimum number of bytes required.
		needed: usize,
		/// Number of bytes that were available.
		available: usize,
	},
	/// The first byte announces an encoding longer than the target type
	/// can ever produce.
	InvalidPrefix {
		/// The offending prefix byte.
		prefix: u8,
	},
	/// The encoded value does not fit in the target type.
	Overflow,
}

impl core::fmt::Display for Error {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Error::BufferTooSmall { needed, available } => write!(
				f,
				"buffer too small: needed {needed} bytes, had {available}"
			),
			Error::InvalidPrefix { prefix } => write!(
				f,
				"prefix byte {prefix:#04X} announces an encoding longer \
				 than the target type allows"
			),
			Error::Overflow => {
				write!(f, "encoded value does not fit in the target type")
			},
		}
	}
}

impl core::error::Error for Error {}

/// Convenience alias for results produced by this crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Errors returned by strict and exact decoding APIs.
///
/// This type is separate from [`Error`] so strict validation can remain an
/// additive API while `Error` stays exhaustively matchable throughout v0.4.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrictError {
	/// The underlying checked decoder rejected the input.
	Decode(Error),
	/// The supplied bytes were not the value's canonical encoding.
	NonCanonical {
		/// Length of the supplied encoding.
		encoded_len: usize,
		/// Length of the canonical encoding for the decoded value.
		canonical_len: usize,
	},
	/// Unread bytes remained when complete consumption was required.
	TrailingBytes {
		/// Bytes consumed before complete consumption was checked.
		consumed: usize,
		/// Total bytes available in the supplied slice.
		available: usize,
	},
}

impl core::fmt::Display for StrictError {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			StrictError::Decode(error) => error.fmt(f),
			StrictError::NonCanonical {
				encoded_len,
				canonical_len,
			} if encoded_len == canonical_len => write!(
				f,
				"non-canonical encoding: {encoded_len}-byte input differs from the canonical byte representation"
			),
			StrictError::NonCanonical {
				encoded_len,
				canonical_len,
			} => write!(
				f,
				"non-canonical encoding: used {encoded_len} bytes, canonical form uses {canonical_len}"
			),
			StrictError::TrailingBytes {
				consumed,
				available,
			} => write!(
				f,
				"trailing bytes: consumed {consumed} of {available} bytes"
			),
		}
	}
}

impl core::error::Error for StrictError {
	fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
		match self {
			StrictError::Decode(error) => Some(error),
			StrictError::NonCanonical { .. }
			| StrictError::TrailingBytes { .. } => None,
		}
	}
}

impl From<Error> for StrictError {
	fn from(error: Error) -> Self {
		StrictError::Decode(error)
	}
}

/// Convenience alias for results produced by strict decoding APIs.
pub type StrictResult<T> = core::result::Result<T, StrictError>;
