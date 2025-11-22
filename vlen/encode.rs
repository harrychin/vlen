//! Encoding functions for vlen

use crate::helpers::{is_aligned, ptr_from_mut};

/// Macro for writing aligned/unaligned values to a buffer at offset 1
macro_rules! write_aligned_at_offset {
	($buf:expr, $value:expr, $ut:ident, $shift:expr) => {
		unsafe {
			let ptr =
				ptr_from_mut::<[u8; core::mem::size_of::<$ut>() + 1]>($buf)
					.cast::<u8>()
					.add(1)
					.cast::<$ut>();
			if is_aligned::<{ core::mem::size_of::<$ut>() }>(ptr as *const u8) {
				ptr.write(($value >> $shift).to_le());
			} else {
				ptr.write_unaligned(($value >> $shift).to_le());
			}
		}
	};
}

/// Unified macro for size calculation and encoding of large integers
macro_rules! encode_large_int {
	($(#[$docs:meta])* $size_fn:ident, $encode_fn:ident ( $ut:ident, $smaller_ut:ident, $smaller_size_fn:ident, $smaller_encode_fn:ident, $max_smaller:expr, $buf_size:expr, $smaller_buf_size:expr ) ) => {
		$(#[$docs])*
		#[inline]
		#[must_use]
		pub const fn $size_fn(value: $ut) -> usize {
			match value {
				_ if value <= $max_smaller as $ut => $smaller_size_fn(value as $smaller_ut),
				_ => {
					const LEN_MASK: u8 = if $ut::BITS == 64 { 0b111 } else { 0b1111 };
					let len = (((value.leading_zeros() >> 3) as u8) ^ LEN_MASK);
					(len + 2) as usize
				},
			}
		}

		$(#[$docs])*
		#[inline]
		#[must_use]
		pub fn $encode_fn(buf: &mut [u8; $buf_size], value: $ut) -> usize {
			match value {
				_ if value <= $max_smaller as $ut => {
				let buf_smaller = unsafe {
						&mut *(ptr_from_mut::<[u8; $buf_size]>(buf).cast::<[u8; $smaller_buf_size]>())
				};
					$smaller_encode_fn(buf_smaller, value as $smaller_ut)
			},
			_ => {
					write_aligned_at_offset!(buf, value, $ut, 0);
					const LEN_MASK: u8 = if $ut::BITS == 64 { 0b111 } else { 0b1111 };
					let len = (((value.leading_zeros() >> 3) as u8) ^ LEN_MASK);
					buf[0] = 0xF0 | len;
				(len + 2) as usize
			},
			}
		}
	};
}

/// Returns the encoded length in a `vlen` prefix byte.
#[must_use]
pub const fn encoded_len(b: u8) -> usize {
	match b {
		_ if b < 0x80 => 1,
		_ if b < 0xC0 => 2,
		_ if b < 0xE0 => 3,
		_ if b < 0xF0 => 4,
		_ => ((b & 0x0F) + 2) as usize,
	}
}

/// Calculates the encoded size of a u16 value without encoding it.
#[inline]
#[must_use]
pub const fn encoded_size_u16(value: u16) -> usize {
	match value {
		_ if value < 0x80 => 1,
		_ if value < 0x4000 => 2,
		_ => 3,
	}
}

/// Calculates the encoded size of a u32 value without encoding it.
#[inline]
#[must_use]
pub const fn encoded_size_u32(value: u32) -> usize {
	match value {
		_ if value <= u16::MAX as u32 => encoded_size_u16(value as u16),
		_ if value < 0x200000 => 3,
		_ if value < 0x10000000 => 4,
		_ => 5,
	}
}

encode_large_int! {
	/// Calculates the encoded size of a u64 value without encoding it.
	encoded_size_u64,
	encode_u64(u64, u32, encoded_size_u32, encode_u32, u32::MAX, 9, 5)
}

encode_large_int! {
	/// Calculates the encoded size of a u128 value without encoding it.
	encoded_size_u128,
	encode_u128(u128, u64, encoded_size_u64, encode_u64, u64::MAX, 17, 9)
}

/// Encodes a `u16` into a buffer, returning the encoded length.
#[inline]
#[must_use]
pub fn encode_u16(buf: &mut [u8; 3], value: u16) -> usize {
	match value {
		_ if value < 0x80 => {
			buf[0] = value as u8;
			1
		},
		_ if value < 0x4000 => {
			buf[0] = 0x80 | ((value & 0x3F) as u8);
			buf[1] = (value >> 6) as u8;
			2
		},
		_ => {
			buf[0] = 0xDE;
			buf[1] = (value & 0xFF) as u8;
			buf[2] = (value >> 8) as u8;
			3
		},
	}
}

/// Encodes a `u32` into a buffer, returning the encoded length.
#[inline]
#[must_use]
pub fn encode_u32(buf: &mut [u8; 5], value: u32) -> usize {
	match value {
		_ if value < 0x4000 => {
			let buf_u16 = unsafe {
				&mut *(ptr_from_mut::<[u8; 5]>(buf).cast::<[u8; 3]>())
			};
			encode_u16(buf_u16, value as u16)
		},
		_ if value < 0x200000 => {
			buf[0] = 0xC0 | ((value & 0x1F) as u8);
			buf[1] = (value >> 5) as u8;
			buf[2] = (value >> 13) as u8;
			3
		},
		_ if value < 0x10000000 => {
			buf[0] = 0xE0 | ((value & 0x0F) as u8);
			write_aligned_at_offset!(buf, value, u32, 4);
			4
		},
		_ => {
			write_aligned_at_offset!(buf, value, u32, 0);
			buf[0] = 0xF3;
			5
		},
	}
}

/// Unified macro for signed integer encoding
macro_rules! encode_signed_int {
	($(#[$docs:meta])* $name:ident ( $it:ident, $ut:ident, $encode_fn:ident, $buf_size:expr ) ) => {
		$(#[$docs])*
		#[inline]
		#[must_use]
		pub fn $name(buf: &mut [u8; $buf_size], value: $it) -> usize {
			const ZIGZAG_SHIFT: u8 = ($ut::BITS as u8) - 1;
			let zigzag = ((value >> ZIGZAG_SHIFT) as $ut) ^ ((value << 1) as $ut);
			$encode_fn(buf, zigzag)
		}
	};
}

encode_signed_int! {
	/// Encodes an `i16` into a buffer, returning the encoded length.
	encode_i16(i16, u16, encode_u16, 3)
}

encode_signed_int! {
	/// Encodes an `i32` into a buffer, returning the encoded length.
	encode_i32(i32, u32, encode_u32, 5)
}

encode_signed_int! {
	/// Encodes an `i64` into a buffer, returning the encoded length.
	encode_i64(i64, u64, encode_u64, 9)
}

encode_signed_int! {
	/// Encodes an `i128` into a buffer, returning the encoded length.
	encode_i128(i128, u128, encode_u128, 17)
}

/// Unified macro for floating-point encoding
macro_rules! encode_float {
	($(#[$docs:meta])* $name:ident ( $ft:ident, $ut:ident, $encode_fn:ident, $buf_size:expr ) ) => {
		$(#[$docs])*
		#[inline]
		#[must_use]
		pub fn $name(buf: &mut [u8; $buf_size], value: $ft) -> usize {
			$encode_fn(buf, value.to_bits().swap_bytes())
		}
	};
}

encode_float! {
	/// Encodes an `f32` into a buffer, returning the encoded length.
	encode_f32(f32, u32, encode_u32, 5)
}

encode_float! {
	/// Encodes an `f64` into a buffer, returning the encoded length.
	encode_f64(f64, u64, encode_u64, 9)
}

/// Generic encoding function that works with any integer type.
#[inline]
pub fn encode<T>(buf: &mut [u8], value: T) -> Result<usize, &'static str>
where
	T: Encode,
{
	T::encode(buf, value)
}

/// Generic size calculation function that works with any integer type.
#[inline]
pub fn encoded_size<T>(value: T) -> Result<usize, &'static str>
where
	T: Encode,
{
	T::encoded_size(value)
}

/// Bulk encoding function for multiple values.
pub fn bulk_encode<T>(
	buf: &mut [u8],
	values: &[T],
) -> Result<usize, &'static str>
where
	T: Encode + Copy,
{
	let mut offset = 0;
	for &value in values {
		if offset >= buf.len() {
			return Err("buffer too small for bulk encoding");
		}
		let len = T::encode(&mut buf[offset..], value)?;
		offset += len;
	}
	Ok(offset)
}

/// Trait for types that can be encoded using vlen.
pub trait Encode: Sized {
	/// Encodes the value into the provided buffer.
	fn encode(buf: &mut [u8], value: Self) -> Result<usize, &'static str>;

	/// Calculates the encoded size of the value without encoding it.
	fn encoded_size(value: Self) -> Result<usize, &'static str>;
}

/// Macro to generate Encode implementation for unsigned integers
macro_rules! impl_encode_unsigned {
	($t:ty, $buf_size:expr, $encode_fn:ident, $size_fn:ident) => {
		impl Encode for $t {
			#[inline]
			fn encode(
				buf: &mut [u8],
				value: Self,
			) -> Result<usize, &'static str> {
				if buf.len() < $buf_size {
					return Err(concat!(
						"buffer too small for ",
						stringify!($t),
						" encoding"
					));
				}
				let buf_array =
					unsafe { &mut *(buf.as_mut_ptr() as *mut [u8; $buf_size]) };
				Ok($encode_fn(buf_array, value))
			}

			#[inline]
			fn encoded_size(value: Self) -> Result<usize, &'static str> {
				Ok($size_fn(value))
			}
		}
	};
}

/// Macro to generate Encode implementation for signed integers
macro_rules! impl_encode_signed {
	($t:ty, $buf_size:expr, $encode_fn:ident, $size_fn:ident, $cast_ty:ty) => {
		impl Encode for $t {
			#[inline]
			fn encode(
				buf: &mut [u8],
				value: Self,
			) -> Result<usize, &'static str> {
				if buf.len() < $buf_size {
					return Err(concat!(
						"buffer too small for ",
						stringify!($t),
						" encoding"
					));
				}
				let buf_array =
					unsafe { &mut *(buf.as_mut_ptr() as *mut [u8; $buf_size]) };
				Ok($encode_fn(buf_array, value))
			}

			#[inline]
			fn encoded_size(value: Self) -> Result<usize, &'static str> {
				// For signed integers, we need to convert to unsigned for size calculation
				const ZIGZAG_SHIFT: u8 =
					(core::mem::size_of::<$t>() * 8 - 1) as u8;
				let zigzag: $cast_ty =
					((value >> ZIGZAG_SHIFT) as $cast_ty) ^ ((value << 1) as $cast_ty);
				Ok($size_fn(zigzag))
			}
		}
	};
}

/// Macro to generate Encode implementation for floating-point types
macro_rules! impl_encode_float {
	($t:ty, $buf_size:expr, $encode_fn:ident, $size_fn:ident) => {
		impl Encode for $t {
			#[inline]
			fn encode(
				buf: &mut [u8],
				value: Self,
			) -> Result<usize, &'static str> {
				if buf.len() < $buf_size {
					return Err(concat!(
						"buffer too small for ",
						stringify!($t),
						" encoding"
					));
				}
				let buf_array =
					unsafe { &mut *(buf.as_mut_ptr() as *mut [u8; $buf_size]) };
				Ok($encode_fn(buf_array, value))
			}

			#[inline]
			fn encoded_size(value: Self) -> Result<usize, &'static str> {
				Ok($size_fn(value.to_bits().swap_bytes()))
			}
		}
	};
}

impl_encode_unsigned!(u16, 3, encode_u16, encoded_size_u16);
impl_encode_unsigned!(u32, 5, encode_u32, encoded_size_u32);
impl_encode_unsigned!(u64, 9, encode_u64, encoded_size_u64);
impl_encode_unsigned!(u128, 17, encode_u128, encoded_size_u128);

impl_encode_signed!(i16, 3, encode_i16, encoded_size_u16, u16);
impl_encode_signed!(i32, 5, encode_i32, encoded_size_u32, u32);
impl_encode_signed!(i64, 9, encode_i64, encoded_size_u64, u64);
impl_encode_signed!(i128, 17, encode_i128, encoded_size_u128, u128);

impl_encode_float!(f32, 5, encode_f32, encoded_size_u32);
impl_encode_float!(f64, 9, encode_f64, encoded_size_u64);
