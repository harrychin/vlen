//! Decoding functions for vlen

use crate::helpers::{is_aligned, ptr_from_ref};

/// Macro for casting buffer to smaller type
macro_rules! cast_buffer_ref {
	($buf:expr, $from_size:expr, $to_size:expr) => {
		unsafe {
			&*(ptr_from_ref::<[u8; $from_size]>($buf).cast::<[u8; $to_size]>())
		}
	};
}

/// Macro for delegating to smaller type decoder
macro_rules! decode_delegate {
	($buf:expr, $smaller_fn:ident, $from_size:expr, $to_size:expr) => {{
		let buf_smaller = cast_buffer_ref!($buf, $from_size, $to_size);
		$smaller_fn(buf_smaller)
	}};
}

/// Macro to generate binary length prefix decoding for a specific type
macro_rules! decode_binary_length_prefix {
	($buf:expr, $T:ty, $size:expr) => {{
		let len = $buf[0] & 0x0F;
		let payload_bytes = (len + 1) as usize;
		let mask = if payload_bytes >= $size {
			<$T>::MAX
		} else {
			<$T>::MAX >> (($size - payload_bytes) * 8)
		};
		let value = unsafe {
			let ptr = $buf.as_ptr().add(1).cast::<$T>();
			if is_aligned::<{ $size }>(ptr as *const u8) {
				ptr.read()
			} else {
				ptr.read_unaligned()
			}
		};
		(<$T>::from_le(value) & mask, (len + 2) as usize)
	}};
}

/// Unified macro for large integer decoding (u64/u128)
macro_rules! decode_large_int {
	($(#[$docs:meta])* $name:ident ( $ut:ident, $smaller_ut:ident, $smaller_fn:ident, $buf_size:expr, $smaller_buf_size:expr ) ) => {
		$(#[$docs])*
		#[inline]
		#[must_use]
		pub fn $name(buf: &[u8; $buf_size]) -> ($ut, usize) {
			match buf[0] {
				_ if buf[0] >= 0xF0 => decode_binary_length_prefix!(buf, $ut, core::mem::size_of::<$ut>()),
				_ => {
					let (value, len) = decode_delegate!(buf, $smaller_fn, $buf_size, $smaller_buf_size);
					(value as $ut, len)
				},
			}
		}
	};
}

/// Decodes a `u16` from a buffer, returning the value and encoded length.
#[inline]
#[must_use]
pub fn decode_u16(buf: &[u8; 3]) -> (u16, usize) {
	let buf0 = buf[0] as u16;
	match buf0 {
		_ if buf0 < 0x80 => (buf0, 1),
		_ if buf0 < 0xC0 => {
			let low = (buf0 as u8) & 0x3F;
			let value = ((buf[1] as u16) << 6) | (low as u16);
			(value, 2)
		},
		_ if buf0 == 0xDE => {
			let value = ((buf[2] as u16) << 8) | (buf[1] as u16);
			(value, 3)
		},
		_ => decode_binary_length_prefix!(buf, u16, 2),
	}
}

/// Decodes a `u32` from a buffer, returning the value and encoded length.
#[inline]
#[must_use]
pub fn decode_u32(buf: &[u8; 5]) -> (u32, usize) {
	let buf0 = buf[0] as u32;
	match buf0 {
		_ if buf0 >= 0xF0 => decode_binary_length_prefix!(buf, u32, 4),
		_ if buf0 < 0xC0 => {
			let (value, len) = decode_delegate!(buf, decode_u16, 5, 3);
			(value as u32, len)
		},
		_ if buf0 < 0xE0 => {
			let low = buf0 & 0x1F;
			let value = ((buf[2] as u32) << 13) | ((buf[1] as u32) << 5) | low;
			(value, 3)
		},
		_ => {
			let value = ((buf[3] as u32) << 20)
				| ((buf[2] as u32) << 12)
				| ((buf[1] as u32) << 4)
				| (buf0 & 0x0F);
			(value, 4)
		},
	}
}

decode_large_int! {
	/// Decodes a `u64` from a buffer, returning the value and encoded length.
	decode_u64(u64, u32, decode_u32, 9, 5)
}

decode_large_int! {
	/// Decodes a `u128` from a buffer, returning the value and encoded length.
	decode_u128(u128, u32, decode_u32, 17, 5)
}

/// Unified macro for signed integer decoding
macro_rules! decode_signed_int {
	($(#[$docs:meta])* $name:ident ( $it:ident, $ut:ident, $decode_fn:ident, $buf_size:expr ) ) => {
		$(#[$docs])*
		#[inline]
		#[must_use]
		pub fn $name(buf: &[u8; $buf_size]) -> ($it, usize) {
			const ZIGZAG_SHIFT: u8 = 1;
			let (zigzag, len) = $decode_fn(buf);
			let value = ((zigzag >> ZIGZAG_SHIFT) as $it) ^ (-((zigzag & 1) as $it));
			(value, len)
		}
	};
}

decode_signed_int! {
	/// Decodes an `i16` from a buffer, returning the value and encoded length.
	decode_i16(i16, u16, decode_u16, 3)
}

decode_signed_int! {
	/// Decodes an `i32` from a buffer, returning the value and encoded length.
	decode_i32(i32, u32, decode_u32, 5)
}

decode_signed_int! {
	/// Decodes an `i64` from a buffer, returning the value and encoded length.
	decode_i64(i64, u64, decode_u64, 9)
}

decode_signed_int! {
	/// Decodes an `i128` from a buffer, returning the value and encoded length.
	decode_i128(i128, u128, decode_u128, 17)
}

/// Unified macro for floating-point decoding
macro_rules! decode_float {
	($(#[$docs:meta])* $name:ident ( $ft:ident, $ut:ident, $decode_fn:ident, $buf_size:expr ) ) => {
		$(#[$docs])*
		#[inline]
		#[must_use]
		pub fn $name(buf: &[u8; $buf_size]) -> ($ft, usize) {
			let (swapped, len) = $decode_fn(buf);
			($ft::from_bits(swapped.swap_bytes()), len)
		}
	};
}

decode_float! {
	/// Decodes an `f32` from a buffer, returning the value and encoded length.
	decode_f32(f32, u32, decode_u32, 5)
}

decode_float! {
	/// Decodes an `f64` from a buffer, returning the value and encoded length.
	decode_f64(f64, u64, decode_u64, 9)
}

/// Generic decoding function that works with any integer type.
#[inline]
pub fn decode<T>(buf: &[u8]) -> Result<(T, usize), &'static str>
where
	T: Decode,
{
	T::decode(buf)
}

/// Bulk decoding function for multiple values.
pub fn bulk_decode<T>(
	buf: &[u8],
	values: &mut [T],
) -> Result<usize, &'static str>
where
	T: Decode,
{
	let mut offset = 0;
	let mut i = 0;
	while i < values.len() && offset < buf.len() {
		let (value, len) = T::decode(&buf[offset..])?;
		values[i] = value;
		offset += len;
		i += 1;
	}
	Ok(offset)
}

/// Trait for types that can be decoded using vlen.
pub trait Decode: Sized {
	/// Decodes the value from the provided buffer.
	fn decode(buf: &[u8]) -> Result<(Self, usize), &'static str>;
}

/// Macro to generate Decode implementation for unsigned integers
macro_rules! impl_decode_unsigned {
	($t:ty, $buf_size:expr, $decode_fn:ident) => {
		impl Decode for $t {
			#[inline]
			fn decode(buf: &[u8]) -> Result<(Self, usize), &'static str> {
				if buf.len() < $buf_size {
					return Err(concat!(
						"buffer too small for ",
						stringify!($t),
						" decoding"
					));
				}
				let buf_array =
					unsafe { &*(buf.as_ptr() as *const [u8; $buf_size]) };
				Ok($decode_fn(buf_array))
			}
		}
	};
}

/// Macro to generate Decode implementation for signed integers
macro_rules! impl_decode_signed {
	($t:ty, $buf_size:expr, $decode_fn:ident) => {
		impl Decode for $t {
			#[inline]
			fn decode(buf: &[u8]) -> Result<(Self, usize), &'static str> {
				if buf.len() < $buf_size {
					return Err(concat!(
						"buffer too small for ",
						stringify!($t),
						" decoding"
					));
				}
				let buf_array =
					unsafe { &*(buf.as_ptr() as *const [u8; $buf_size]) };
				Ok($decode_fn(buf_array))
			}
		}
	};
}

/// Macro to generate Decode implementation for floating-point types
macro_rules! impl_decode_float {
	($t:ty, $buf_size:expr, $decode_fn:ident) => {
		impl Decode for $t {
			#[inline]
			fn decode(buf: &[u8]) -> Result<(Self, usize), &'static str> {
				if buf.len() < $buf_size {
					return Err(concat!(
						"buffer too small for ",
						stringify!($t),
						" decoding"
					));
				}
				let buf_array =
					unsafe { &*(buf.as_ptr() as *const [u8; $buf_size]) };
				Ok($decode_fn(buf_array))
			}
		}
	};
}

impl_decode_unsigned!(u16, 3, decode_u16);
impl_decode_unsigned!(u32, 5, decode_u32);
impl_decode_unsigned!(u64, 9, decode_u64);
impl_decode_unsigned!(u128, 17, decode_u128);

impl_decode_signed!(i16, 3, decode_i16);
impl_decode_signed!(i32, 5, decode_i32);
impl_decode_signed!(i64, 9, decode_i64);
impl_decode_signed!(i128, 17, decode_i128);

impl_decode_float!(f32, 5, decode_f32);
impl_decode_float!(f64, 9, decode_f64);
