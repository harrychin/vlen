//! Serde integration for vlen encoding.
//!
//! The `Vlen*` wrapper types serialize their inner value through the
//! vlen codec. The representation adapts to the format: binary formats
//! (bincode, postcard, ...) receive the raw encoded bytes, while
//! human-readable formats (JSON, TOML, ...) receive the bytes as a
//! base64 string. Neither path allocates.
//!
//! ## Example
//!
//! ```rust
//! use serde::{Deserialize, Serialize};
//! use vlen::serde::{VlenI64, VlenU32};
//!
//! #[derive(Serialize, Deserialize)]
//! struct MyStruct {
//!     id: VlenU32,
//!     timestamp: VlenI64,
//! }
//!
//! let data = MyStruct {
//!     id: VlenU32(12345),
//!     timestamp: VlenI64(-1234567890),
//! };
//!
//! let json = serde_json::to_string(&data).unwrap();
//! let back: MyStruct = serde_json::from_str(&json).unwrap();
//! assert_eq!(data.id, back.id);
//! assert_eq!(data.timestamp, back.timestamp);
//! ```

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::decode::Decode;
use crate::encode::Encode;

/// Longest base64 text for any encoding (17 bytes -> 24 characters).
const MAX_BASE64_LEN: usize = 24;

macro_rules! vlen_wrapper {
	(
		$(#[$docs:meta])*
		$wrapper:ident($inner:ident) $(, $derive:ident)*
	) => {
		$(#[$docs])*
		#[derive(
			Debug, Default, Clone, Copy, PartialEq, PartialOrd $(, $derive)*
		)]
		#[repr(transparent)]
		pub struct $wrapper(pub $inner);

		impl From<$inner> for $wrapper {
			#[inline]
			fn from(value: $inner) -> Self {
				$wrapper(value)
			}
		}

		impl From<$wrapper> for $inner {
			#[inline]
			fn from(wrapper: $wrapper) -> Self {
				wrapper.0
			}
		}

		impl core::ops::Deref for $wrapper {
			type Target = $inner;

			#[inline]
			fn deref(&self) -> &Self::Target {
				&self.0
			}
		}

		impl core::ops::DerefMut for $wrapper {
			#[inline]
			fn deref_mut(&mut self) -> &mut Self::Target {
				&mut self.0
			}
		}

		impl Serialize for $wrapper {
			fn serialize<S: Serializer>(
				&self,
				serializer: S,
			) -> Result<S::Ok, S::Error> {
				let mut buf = [0u8; <$inner as Encode>::MAX_ENCODED_SIZE];
				let len = self
					.0
					.encode(&mut buf)
					.map_err(serde::ser::Error::custom)?;
				if serializer.is_human_readable() {
					let mut text = [0u8; MAX_BASE64_LEN];
					let text_len = BASE64
						.encode_slice(&buf[..len], &mut text)
						.map_err(serde::ser::Error::custom)?;
					let text = core::str::from_utf8(&text[..text_len])
						.expect("base64 output is ASCII");
					serializer.serialize_str(text)
				} else {
					serializer.serialize_bytes(&buf[..len])
				}
			}
		}

		impl<'de> Deserialize<'de> for $wrapper {
			fn deserialize<D: Deserializer<'de>>(
				deserializer: D,
			) -> Result<Self, D::Error> {
				struct Visitor;

				impl<'de> serde::de::Visitor<'de> for Visitor {
					type Value = $wrapper;

					fn expecting(
						&self,
						f: &mut core::fmt::Formatter<'_>,
					) -> core::fmt::Result {
						write!(
							f,
							concat!(
								"a vlen-encoded ",
								stringify!($inner)
							)
						)
					}

					fn visit_str<E: serde::de::Error>(
						self,
						text: &str,
					) -> Result<Self::Value, E> {
						if text.len() > MAX_BASE64_LEN {
							return Err(E::invalid_length(
								text.len(),
								&self,
							));
						}
						// Base64 never expands beyond 3/4 of its input.
						let mut bytes = [0u8; MAX_BASE64_LEN];
						let len = BASE64
							.decode_slice(text, &mut bytes)
							.map_err(E::custom)?;
						self.visit_bytes(&bytes[..len])
					}

					fn visit_bytes<E: serde::de::Error>(
						self,
						bytes: &[u8],
					) -> Result<Self::Value, E> {
						let (value, len) =
							<$inner as Decode>::decode(bytes)
								.map_err(E::custom)?;
						if len != bytes.len() {
							return Err(E::invalid_length(
								bytes.len(),
								&self,
							));
						}
						Ok($wrapper(value))
					}

					fn visit_seq<A: serde::de::SeqAccess<'de>>(
						self,
						mut seq: A,
					) -> Result<Self::Value, A::Error> {
						// Some binary formats represent bytes as a
						// sequence of u8.
						let mut bytes =
							[0u8; <$inner as Encode>::MAX_ENCODED_SIZE];
						let mut len = 0;
						while let Some(byte) = seq.next_element::<u8>()? {
							if len >= bytes.len() {
								return Err(
									serde::de::Error::invalid_length(
										len + 1,
										&self,
									),
								);
							}
							bytes[len] = byte;
							len += 1;
						}
						self.visit_bytes(&bytes[..len])
					}
				}

				if deserializer.is_human_readable() {
					deserializer.deserialize_str(Visitor)
				} else {
					deserializer.deserialize_bytes(Visitor)
				}
			}
		}
	};
}

vlen_wrapper! {
    /// Serializes a `u16` using vlen encoding.
	VlenU16(u16), Eq, Ord, Hash
}
vlen_wrapper! {
    /// Serializes a `u32` using vlen encoding.
	VlenU32(u32), Eq, Ord, Hash
}
vlen_wrapper! {
    /// Serializes a `u64` using vlen encoding.
	VlenU64(u64), Eq, Ord, Hash
}
vlen_wrapper! {
    /// Serializes a `u128` using vlen encoding.
	VlenU128(u128), Eq, Ord, Hash
}
vlen_wrapper! {
    /// Serializes an `i16` using vlen encoding.
	VlenI16(i16), Eq, Ord, Hash
}
vlen_wrapper! {
    /// Serializes an `i32` using vlen encoding.
	VlenI32(i32), Eq, Ord, Hash
}
vlen_wrapper! {
    /// Serializes an `i64` using vlen encoding.
	VlenI64(i64), Eq, Ord, Hash
}
vlen_wrapper! {
    /// Serializes an `i128` using vlen encoding.
	VlenI128(i128), Eq, Ord, Hash
}
vlen_wrapper! {
    /// Serializes an `f32` using vlen encoding.
	VlenF32(f32)
}
vlen_wrapper! {
    /// Serializes an `f64` using vlen encoding.
	VlenF64(f64)
}
