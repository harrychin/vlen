//! Exports `u32` encode and decode through one API surface so the
//! linked code can be measured on a bare-metal target.

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
	loop {}
}

/// The infallible array API.
#[cfg(feature = "unchecked")]
mod api {
	#[unsafe(no_mangle)]
	pub fn size_check_encode(buf: &mut [u8; 5], value: u32) -> usize {
		vlen::encode_u32(buf, value)
	}

	#[unsafe(no_mangle)]
	pub fn size_check_decode(buf: &[u8; 5], value: &mut u32) -> usize {
		let (decoded, len) = vlen::decode_u32(buf);
		*value = decoded;
		len
	}
}

/// The validating slice API.
#[cfg(feature = "checked")]
mod api {
	use vlen::{Decode, Encode};

	#[unsafe(no_mangle)]
	pub fn size_check_encode(buf: &mut [u8], value: u32) -> Option<usize> {
		value.encode(buf).ok()
	}

	#[unsafe(no_mangle)]
	pub fn size_check_decode(buf: &[u8], value: &mut u32) -> Option<usize> {
		let (decoded, len) = u32::decode(buf).ok()?;
		*value = decoded;
		Some(len)
	}
}
