//! Serde integration for vlen encoding
//!
//! This module provides `Serialize` and `Deserialize` implementations for all
//! supported numeric types using vlen encoding. This allows you to use vlen
//! encoding with serde-based serialization formats.
//!
//! ## Example
//!
//! ```rust
//! use serde::{Serialize, Deserialize};
//! use vlen::serde::{VlenU32, VlenI64};
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
//! // Serialize to JSON (or any other serde format)
//! let json = serde_json::to_string(&data).unwrap();
//! let deserialized: MyStruct = serde_json::from_str(&json).unwrap();
//!
//! assert_eq!(data.id.0, deserialized.id.0);
//! assert_eq!(data.timestamp.0, deserialized.timestamp.0);
//! ```

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{decode::Decode, encode::Encode};
use core::ops;

/// A wrapper type that serializes and deserializes `u16` values using vlen encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VlenU16(pub u16);

/// A wrapper type that serializes and deserializes `u32` values using vlen encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VlenU32(pub u32);

/// A wrapper type that serializes and deserializes `u64` values using vlen encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VlenU64(pub u64);

/// A wrapper type that serializes and deserializes `u128` values using vlen encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VlenU128(pub u128);

/// A wrapper type that serializes and deserializes `i16` values using vlen encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VlenI16(pub i16);

/// A wrapper type that serializes and deserializes `i32` values using vlen encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VlenI32(pub i32);

/// A wrapper type that serializes and deserializes `i64` values using vlen encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VlenI64(pub i64);

/// A wrapper type that serializes and deserializes `i128` values using vlen encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VlenI128(pub i128);

/// A wrapper type that serializes and deserializes `f32` values using vlen encoding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VlenF32(pub f32);

/// A wrapper type that serializes and deserializes `f64` values using vlen encoding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VlenF64(pub f64);

// Macro to generate serde implementations for unsigned integer types
macro_rules! impl_serde_unsigned {
	($wrapper:ident, $inner:ty) => {
		#[cfg(feature = "serde")]
		impl Serialize for $wrapper {
			fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
			where
				S: Serializer,
			{
				let mut buf = [0u8; 17];
				let len = <$inner>::encode(&mut buf, self.0)
					.map_err(|e| serde::ser::Error::custom(e))?;
				#[cfg(feature = "alloc")]
				{
					use alloc::string::String;
					let base64 = base64::encode(&buf[..len]);
					serializer.serialize_str(&base64)
				}
				#[cfg(not(feature = "alloc"))]
				{
					serializer.serialize_bytes(&buf[..len])
				}
			}
		}

		#[cfg(feature = "serde")]
		impl<'de> Deserialize<'de> for $wrapper {
			fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
			where
				D: Deserializer<'de>,
			{
				#[cfg(feature = "alloc")]
				{
					use alloc::string::String;
					let s = String::deserialize(deserializer)?;
					let bytes = base64::decode(&s)
						.map_err(|e| serde::de::Error::custom(e))?;
					let value = match core::any::type_name::<$inner>() {
						"u16" => {
							let mut arr = [0u8; 3];
							arr[..bytes.len()].copy_from_slice(&bytes);
							<$inner>::decode(&arr)
						},
						"u32" => {
							let mut arr = [0u8; 5];
							arr[..bytes.len()].copy_from_slice(&bytes);
							<$inner>::decode(&arr)
						},
						"u64" => {
							let mut arr = [0u8; 9];
							arr[..bytes.len()].copy_from_slice(&bytes);
							<$inner>::decode(&arr)
						},
						"u128" => {
							let mut arr = [0u8; 17];
							arr[..bytes.len()].copy_from_slice(&bytes);
							<$inner>::decode(&arr)
						},
						_ => {
							return Err(serde::de::Error::custom(
								"unsupported type for vlen deserialization",
							))
						},
					};
					let (value, _) =
						value.map_err(|e| serde::de::Error::custom(e))?;
					Ok($wrapper(value))
				}
				#[cfg(not(feature = "alloc"))]
				{
					let bytes = <&[u8]>::deserialize(deserializer)?;
					let value = match core::any::type_name::<$inner>() {
						"u16" => {
							let mut arr = [0u8; 3];
							arr[..bytes.len()].copy_from_slice(bytes);
							<$inner>::decode(&arr)
						},
						"u32" => {
							let mut arr = [0u8; 5];
							arr[..bytes.len()].copy_from_slice(bytes);
							<$inner>::decode(&arr)
						},
						"u64" => {
							let mut arr = [0u8; 9];
							arr[..bytes.len()].copy_from_slice(bytes);
							<$inner>::decode(&arr)
						},
						"u128" => {
							let mut arr = [0u8; 17];
							arr[..bytes.len()].copy_from_slice(bytes);
							<$inner>::decode(&arr)
						},
						_ => {
							return Err(serde::de::Error::custom(
								"unsupported type for vlen deserialization",
							))
						},
					};
					let (value, _) =
						value.map_err(|e| serde::de::Error::custom(e))?;
					Ok($wrapper(value))
				}
			}
		}
	};
}

// Macro to generate serde implementations for signed integer types
macro_rules! impl_serde_signed {
	($wrapper:ident, $inner:ty) => {
		#[cfg(feature = "serde")]
		impl Serialize for $wrapper {
			fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
			where
				S: Serializer,
			{
				let mut buf = [0u8; 17];
				let len = <$inner>::encode(&mut buf, self.0)
					.map_err(|e| serde::ser::Error::custom(e))?;
				#[cfg(feature = "alloc")]
				{
					use alloc::string::String;
					let base64 = base64::encode(&buf[..len]);
					serializer.serialize_str(&base64)
				}
				#[cfg(not(feature = "alloc"))]
				{
					serializer.serialize_bytes(&buf[..len])
				}
			}
		}

		#[cfg(feature = "serde")]
		impl<'de> Deserialize<'de> for $wrapper {
			fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
			where
				D: Deserializer<'de>,
			{
				#[cfg(feature = "alloc")]
				{
					use alloc::string::String;
					let s = String::deserialize(deserializer)?;
					let bytes = base64::decode(&s)
						.map_err(|e| serde::de::Error::custom(e))?;
					let value = match core::any::type_name::<$inner>() {
						"i16" => {
							let mut arr = [0u8; 3];
							arr[..bytes.len()].copy_from_slice(&bytes);
							<$inner>::decode(&arr)
						},
						"i32" => {
							let mut arr = [0u8; 5];
							arr[..bytes.len()].copy_from_slice(&bytes);
							<$inner>::decode(&arr)
						},
						"i64" => {
							let mut arr = [0u8; 9];
							arr[..bytes.len()].copy_from_slice(&bytes);
							<$inner>::decode(&arr)
						},
						"i128" => {
							let mut arr = [0u8; 17];
							arr[..bytes.len()].copy_from_slice(&bytes);
							<$inner>::decode(&arr)
						},
						_ => {
							return Err(serde::de::Error::custom(
								"unsupported type for vlen deserialization",
							))
						},
					};
					let (value, _) =
						value.map_err(|e| serde::de::Error::custom(e))?;
					Ok($wrapper(value))
				}
				#[cfg(not(feature = "alloc"))]
				{
					let bytes = <&[u8]>::deserialize(deserializer)?;
					let value = match core::any::type_name::<$inner>() {
						"i16" => {
							let mut arr = [0u8; 3];
							arr[..bytes.len()].copy_from_slice(bytes);
							<$inner>::decode(&arr)
						},
						"i32" => {
							let mut arr = [0u8; 5];
							arr[..bytes.len()].copy_from_slice(bytes);
							<$inner>::decode(&arr)
						},
						"i64" => {
							let mut arr = [0u8; 9];
							arr[..bytes.len()].copy_from_slice(bytes);
							<$inner>::decode(&arr)
						},
						"i128" => {
							let mut arr = [0u8; 17];
							arr[..bytes.len()].copy_from_slice(bytes);
							<$inner>::decode(&arr)
						},
						_ => {
							return Err(serde::de::Error::custom(
								"unsupported type for vlen deserialization",
							))
						},
					};
					let (value, _) =
						value.map_err(|e| serde::de::Error::custom(e))?;
					Ok($wrapper(value))
				}
			}
		}
	};
}

// Macro to generate serde implementations for floating-point types
macro_rules! impl_serde_float {
	($wrapper:ident, $inner:ty) => {
		#[cfg(feature = "serde")]
		impl Serialize for $wrapper {
			fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
			where
				S: Serializer,
			{
				let mut buf = [0u8; 17];
				let len = <$inner>::encode(&mut buf, self.0)
					.map_err(|e| serde::ser::Error::custom(e))?;
				#[cfg(feature = "alloc")]
				{
					use alloc::string::String;
					let base64 = base64::encode(&buf[..len]);
					serializer.serialize_str(&base64)
				}
				#[cfg(not(feature = "alloc"))]
				{
					serializer.serialize_bytes(&buf[..len])
				}
			}
		}

		#[cfg(feature = "serde")]
		impl<'de> Deserialize<'de> for $wrapper {
			fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
			where
				D: Deserializer<'de>,
			{
				#[cfg(feature = "alloc")]
				{
					use alloc::string::String;
					let s = String::deserialize(deserializer)?;
					let bytes = base64::decode(&s)
						.map_err(|e| serde::de::Error::custom(e))?;
					let value = match core::any::type_name::<$inner>() {
						"f32" => {
							let mut arr = [0u8; 5];
							arr[..bytes.len()].copy_from_slice(&bytes);
							<$inner>::decode(&arr)
						},
						"f64" => {
							let mut arr = [0u8; 9];
							arr[..bytes.len()].copy_from_slice(&bytes);
							<$inner>::decode(&arr)
						},
						_ => {
							return Err(serde::de::Error::custom(
								"unsupported type for vlen deserialization",
							))
						},
					};
					let (value, _) =
						value.map_err(|e| serde::de::Error::custom(e))?;
					Ok($wrapper(value))
				}
				#[cfg(not(feature = "alloc"))]
				{
					let bytes = <&[u8]>::deserialize(deserializer)?;
					let value = match core::any::type_name::<$inner>() {
						"f32" => {
							let mut arr = [0u8; 5];
							arr[..bytes.len()].copy_from_slice(bytes);
							<$inner>::decode(&arr)
						},
						"f64" => {
							let mut arr = [0u8; 9];
							arr[..bytes.len()].copy_from_slice(bytes);
							<$inner>::decode(&arr)
						},
						_ => {
							return Err(serde::de::Error::custom(
								"unsupported type for vlen deserialization",
							))
						},
					};
					let (value, _) =
						value.map_err(|e| serde::de::Error::custom(e))?;
					Ok($wrapper(value))
				}
			}
		}
	};
}

// Generate serde implementations for all types
impl_serde_unsigned!(VlenU16, u16);
impl_serde_unsigned!(VlenU32, u32);
impl_serde_unsigned!(VlenU64, u64);
impl_serde_unsigned!(VlenU128, u128);

impl_serde_signed!(VlenI16, i16);
impl_serde_signed!(VlenI32, i32);
impl_serde_signed!(VlenI64, i64);
impl_serde_signed!(VlenI128, i128);

impl_serde_float!(VlenF32, f32);
impl_serde_float!(VlenF64, f64);

// Implement From traits for easy conversion
impl From<u16> for VlenU16 {
	fn from(value: u16) -> Self {
		VlenU16(value)
	}
}

impl From<u32> for VlenU32 {
	fn from(value: u32) -> Self {
		VlenU32(value)
	}
}

impl From<u64> for VlenU64 {
	fn from(value: u64) -> Self {
		VlenU64(value)
	}
}

impl From<u128> for VlenU128 {
	fn from(value: u128) -> Self {
		VlenU128(value)
	}
}

impl From<i16> for VlenI16 {
	fn from(value: i16) -> Self {
		VlenI16(value)
	}
}

impl From<i32> for VlenI32 {
	fn from(value: i32) -> Self {
		VlenI32(value)
	}
}

impl From<i64> for VlenI64 {
	fn from(value: i64) -> Self {
		VlenI64(value)
	}
}

impl From<i128> for VlenI128 {
	fn from(value: i128) -> Self {
		VlenI128(value)
	}
}

impl From<f32> for VlenF32 {
	fn from(value: f32) -> Self {
		VlenF32(value)
	}
}

impl From<f64> for VlenF64 {
	fn from(value: f64) -> Self {
		VlenF64(value)
	}
}

// Implement Deref for easy access to inner values
impl ops::Deref for VlenU16 {
	type Target = u16;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl ops::Deref for VlenU32 {
	type Target = u32;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl ops::Deref for VlenU64 {
	type Target = u64;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl ops::Deref for VlenU128 {
	type Target = u128;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl ops::Deref for VlenI16 {
	type Target = i16;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl ops::Deref for VlenI32 {
	type Target = i32;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl ops::Deref for VlenI64 {
	type Target = i64;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl ops::Deref for VlenI128 {
	type Target = i128;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl ops::Deref for VlenF32 {
	type Target = f32;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl ops::Deref for VlenF64 {
	type Target = f64;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

// Implement DerefMut for mutable access
impl ops::DerefMut for VlenU16 {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl ops::DerefMut for VlenU32 {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl ops::DerefMut for VlenU64 {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl ops::DerefMut for VlenU128 {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl ops::DerefMut for VlenI16 {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl ops::DerefMut for VlenI32 {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl ops::DerefMut for VlenI64 {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl ops::DerefMut for VlenI128 {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl ops::DerefMut for VlenF32 {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl ops::DerefMut for VlenF64 {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}
