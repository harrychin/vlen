#![cfg(feature = "serde")]

use serde::{Deserialize, Serialize};
use vlen::serde::{
	VlenF32, VlenF64, VlenI16, VlenI32, VlenI64, VlenI128, VlenU16, VlenU32,
	VlenU64, VlenU128,
};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct TestStruct {
	u16_val: VlenU16,
	u32_val: VlenU32,
	u64_val: VlenU64,
	u128_val: VlenU128,
	i16_val: VlenI16,
	i32_val: VlenI32,
	i64_val: VlenI64,
	i128_val: VlenI128,
	f32_val: VlenF32,
	f64_val: VlenF64,
}

fn sample() -> TestStruct {
	TestStruct {
		u16_val: VlenU16(12345),
		u32_val: VlenU32(123456789),
		u64_val: VlenU64(1234567890123456789),
		u128_val: VlenU128(123456789012345678901234567890123456789),
		i16_val: VlenI16(-12345),
		i32_val: VlenI32(-123456789),
		i64_val: VlenI64(-1234567890123456789),
		i128_val: VlenI128(-123456789012345678901234567890123456789),
		f32_val: VlenF32(core::f32::consts::PI),
		f64_val: VlenF64(core::f64::consts::E),
	}
}

#[test]
fn json_round_trip() {
	let data = sample();
	let json = serde_json::to_string(&data).unwrap();
	let back: TestStruct = serde_json::from_str(&json).unwrap();
	assert_eq!(data, back);
}

#[test]
fn postcard_round_trip() {
	let data = sample();
	let bytes = postcard::to_stdvec(&data).unwrap();
	let back: TestStruct = postcard::from_bytes(&bytes).unwrap();
	assert_eq!(data, back);
}

#[test]
fn binary_formats_get_raw_bytes_not_base64() {
	// In a binary format the payload must be the raw vlen encoding
	// (length prefix + bytes), not a base64 string blown up by a third.
	let bytes = postcard::to_stdvec(&VlenU32(5)).unwrap();
	assert_eq!(bytes, [1, 5]);

	let mut expected = [0u8; 5];
	let len = vlen::encode_u32(&mut expected, 123456789);
	let bytes = postcard::to_stdvec(&VlenU32(123456789)).unwrap();
	assert_eq!(bytes[0] as usize, len);
	assert_eq!(&bytes[1..], &expected[..len]);
}

#[test]
fn json_representation_is_base64() {
	let json = serde_json::to_string(&VlenU32(0)).unwrap();
	assert_eq!(json, "\"AA==\"");
}

#[test]
fn extreme_values_round_trip() {
	macro_rules! check {
		($wrapper:ident, $value:expr) => {
			let value = $wrapper($value);
			let json = serde_json::to_string(&value).unwrap();
			let back: $wrapper = serde_json::from_str(&json).unwrap();
			assert_eq!(value, back);
			let bytes = postcard::to_stdvec(&value).unwrap();
			let back: $wrapper = postcard::from_bytes(&bytes).unwrap();
			assert_eq!(value, back);
		};
	}

	check!(VlenU16, 0);
	check!(VlenU16, u16::MAX);
	check!(VlenU32, u32::MAX);
	check!(VlenU64, u64::MAX);
	check!(VlenU128, u128::MAX);
	check!(VlenI16, i16::MIN);
	check!(VlenI32, i32::MIN);
	check!(VlenI64, i64::MIN);
	check!(VlenI128, i128::MIN);
	check!(VlenF32, f32::MAX);
	check!(VlenF64, f64::MIN);
}

#[test]
fn malicious_input_is_rejected_not_panicked() {
	// Base64 longer than any valid encoding.
	let long = format!("\"{}\"", "A".repeat(64));
	assert!(serde_json::from_str::<VlenU32>(&long).is_err());

	// Valid base64, but more bytes than the type can use.
	let json = serde_json::to_string(&VlenU128(u128::MAX)).unwrap();
	assert!(serde_json::from_str::<VlenU16>(&json).is_err());

	// Not base64 at all.
	assert!(serde_json::from_str::<VlenU32>("\"!!!\"").is_err());

	// Wrong JSON type.
	assert!(serde_json::from_str::<VlenU32>("42").is_err());

	// Trailing garbage after a valid encoding.
	assert!(serde_json::from_str::<VlenU32>("\"AAAA\"").is_err());
}

#[test]
fn deref_and_deref_mut() {
	let mut u32_val = VlenU32(42);
	assert_eq!(*u32_val, 42);
	*u32_val = 100;
	assert_eq!(u32_val.0, 100);

	let mut f64_val = VlenF64(1.5);
	assert_eq!(*f64_val, 1.5);
	*f64_val = -2.25;
	assert_eq!(f64_val.0, -2.25);
}

#[test]
fn conversions() {
	let wrapped: VlenU32 = 42.into();
	assert_eq!(*wrapped, 42);
	let raw: u32 = wrapped.into();
	assert_eq!(raw, 42);

	let wrapped: VlenI64 = (-42).into();
	assert_eq!(i64::from(wrapped), -42);
}

#[test]
fn vectors_round_trip() {
	#[derive(Debug, Serialize, Deserialize, PartialEq)]
	struct VectorTest {
		u32_vec: Vec<VlenU32>,
		i64_vec: Vec<VlenI64>,
		f32_vec: Vec<VlenF32>,
	}

	let data = VectorTest {
		u32_vec: vec![VlenU32(1), VlenU32(2), VlenU32(3)],
		i64_vec: vec![VlenI64(-1), VlenI64(-2), VlenI64(-3)],
		f32_vec: vec![VlenF32(1.5), VlenF32(-2.25), VlenF32(3.75)],
	};

	let json = serde_json::to_string(&data).unwrap();
	let from_json: VectorTest = serde_json::from_str(&json).unwrap();
	assert_eq!(data, from_json);

	let bytes = postcard::to_stdvec(&data).unwrap();
	let from_postcard: VectorTest = postcard::from_bytes(&bytes).unwrap();
	assert_eq!(data, from_postcard);
}

#[test]
fn with_modules_annotate_plain_fields() {
	#[derive(Debug, Serialize, Deserialize, PartialEq)]
	struct Plain {
		#[serde(with = "vlen::serde::u32")]
		id: u32,
		#[serde(with = "vlen::serde::i64")]
		timestamp: i64,
		#[serde(with = "vlen::serde::usize")]
		count: usize,
		#[serde(with = "vlen::serde::f32")]
		score: f32,
	}

	let data = Plain {
		id: 123456789,
		timestamp: -1234567890,
		count: 42,
		score: 1.5,
	};

	let json = serde_json::to_string(&data).unwrap();
	let from_json: Plain = serde_json::from_str(&json).unwrap();
	assert_eq!(data, from_json);

	let bytes = postcard::to_stdvec(&data).unwrap();
	let from_postcard: Plain = postcard::from_bytes(&bytes).unwrap();
	assert_eq!(data, from_postcard);

	// The field representation is identical to the wrapper types'.
	let wrapper_json = serde_json::to_string(&VlenU32(123456789)).unwrap();
	assert!(json.contains(wrapper_json.trim_matches('"')));
}
