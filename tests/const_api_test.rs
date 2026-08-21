//! Verifies that every array-based codec function is usable in const
//! contexts. Evaluation happens at compile time; the runtime assertions
//! only check the pre-computed results.

use vlen::{
	decode_f64, decode_i64, decode_u16, decode_u32, decode_u128, encode_f64,
	encode_i64, encode_u16, encode_u32, encode_u128, encoded_len,
	encoded_size_u32,
};

const fn round_trips() -> bool {
	let mut buf_u16 = [0u8; 3];
	let len = encode_u16(&mut buf_u16, 54321);
	let (value, decoded_len) = decode_u16(&buf_u16);
	if value != 54321 || len != decoded_len {
		return false;
	}

	let mut buf_u32 = [0u8; 5];
	let len = encode_u32(&mut buf_u32, 12345);
	let (value, decoded_len) = decode_u32(&buf_u32);
	if value != 12345 || len != decoded_len {
		return false;
	}
	if len != encoded_size_u32(12345) || len != encoded_len(buf_u32[0]) {
		return false;
	}

	let mut buf_i64 = [0u8; 9];
	let len = encode_i64(&mut buf_i64, -1234567890);
	let (value, decoded_len) = decode_i64(&buf_i64);
	if value != -1234567890 || len != decoded_len {
		return false;
	}

	let mut buf_u128 = [0u8; 17];
	let len = encode_u128(&mut buf_u128, u128::MAX);
	let (value, decoded_len) = decode_u128(&buf_u128);
	if value != u128::MAX || len != decoded_len {
		return false;
	}

	let mut buf_f64 = [0u8; 9];
	let len = encode_f64(&mut buf_f64, -2.5);
	let (value, decoded_len) = decode_f64(&buf_f64);
	if value != -2.5 || len != decoded_len {
		return false;
	}

	true
}

// Evaluated at compile time: a regression fails the build itself.
const _: () = assert!(round_trips());

#[test]
fn const_evaluation_round_trips() {
	// The same function must also agree at runtime.
	assert!(round_trips());
}
