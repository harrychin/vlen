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
