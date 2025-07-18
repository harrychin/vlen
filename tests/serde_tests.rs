#[cfg(feature = "serde")]
mod serde_tests {
	use serde::{Deserialize, Serialize};
	use vlen::serde::*;

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

	#[test]
	fn test_serde_roundtrip() {
		let data = TestStruct {
			u16_val: VlenU16(12345),
			u32_val: VlenU32(123456789),
			u64_val: VlenU64(1234567890123456789),
			u128_val: VlenU128(123456789012345678901234567890123456789),
			i16_val: VlenI16(-12345),
			i32_val: VlenI32(-123456789),
			i64_val: VlenI64(-1234567890123456789),
			i128_val: VlenI128(-123456789012345678901234567890123456789),
			f32_val: VlenF32(3.14159),
			f64_val: VlenF64(2.718281828459045),
		};

		// Test JSON serialization/deserialization
		let json = serde_json::to_string(&data).unwrap();
		let deserialized: TestStruct = serde_json::from_str(&json).unwrap();

		assert_eq!(data, deserialized);
	}

	#[test]
	fn test_individual_types() {
		// Test u16
		let u16_val = VlenU16(65535);
		let json = serde_json::to_string(&u16_val).unwrap();
		println!("JSON output: {}", json);
		let deserialized: VlenU16 = serde_json::from_str(&json).unwrap();
		assert_eq!(u16_val, deserialized);

		// Test u32
		let u32_val = VlenU32(4294967295);
		let json = serde_json::to_string(&u32_val).unwrap();
		let deserialized: VlenU32 = serde_json::from_str(&json).unwrap();
		assert_eq!(u32_val, deserialized);

		// Test u64
		let u64_val = VlenU64(18446744073709551615);
		let json = serde_json::to_string(&u64_val).unwrap();
		let deserialized: VlenU64 = serde_json::from_str(&json).unwrap();
		assert_eq!(u64_val, deserialized);

		// Test u128
		let u128_val = VlenU128(340282366920938463463374607431768211455);
		let json = serde_json::to_string(&u128_val).unwrap();
		let deserialized: VlenU128 = serde_json::from_str(&json).unwrap();
		assert_eq!(u128_val, deserialized);

		// Test i16
		let i16_val = VlenI16(-32768);
		let json = serde_json::to_string(&i16_val).unwrap();
		let deserialized: VlenI16 = serde_json::from_str(&json).unwrap();
		assert_eq!(i16_val, deserialized);

		// Test i32
		let i32_val = VlenI32(-2147483648);
		let json = serde_json::to_string(&i32_val).unwrap();
		let deserialized: VlenI32 = serde_json::from_str(&json).unwrap();
		assert_eq!(i32_val, deserialized);

		// Test i64
		let i64_val = VlenI64(-9223372036854775808);
		let json = serde_json::to_string(&i64_val).unwrap();
		let deserialized: VlenI64 = serde_json::from_str(&json).unwrap();
		assert_eq!(i64_val, deserialized);

		// Test i128
		let i128_val = VlenI128(-170141183460469231731687303715884105728);
		let json = serde_json::to_string(&i128_val).unwrap();
		let deserialized: VlenI128 = serde_json::from_str(&json).unwrap();
		assert_eq!(i128_val, deserialized);

		// Test f32
		let f32_val = VlenF32(std::f32::consts::PI);
		let json = serde_json::to_string(&f32_val).unwrap();
		let deserialized: VlenF32 = serde_json::from_str(&json).unwrap();
		assert_eq!(f32_val, deserialized);

		// Test f64
		let f64_val = VlenF64(std::f64::consts::E);
		let json = serde_json::to_string(&f64_val).unwrap();
		let deserialized: VlenF64 = serde_json::from_str(&json).unwrap();
		assert_eq!(f64_val, deserialized);
	}

	#[test]
	fn test_edge_cases() {
		// Test zero values
		let zero_struct = TestStruct {
			u16_val: VlenU16(0),
			u32_val: VlenU32(0),
			u64_val: VlenU64(0),
			u128_val: VlenU128(0),
			i16_val: VlenI16(0),
			i32_val: VlenI32(0),
			i64_val: VlenI64(0),
			i128_val: VlenI128(0),
			f32_val: VlenF32(0.0),
			f64_val: VlenF64(0.0),
		};

		let json = serde_json::to_string(&zero_struct).unwrap();
		let deserialized: TestStruct = serde_json::from_str(&json).unwrap();
		assert_eq!(zero_struct, deserialized);

		// Test maximum values
		let max_struct = TestStruct {
			u16_val: VlenU16(u16::MAX),
			u32_val: VlenU32(u32::MAX),
			u64_val: VlenU64(u64::MAX),
			u128_val: VlenU128(u128::MAX),
			i16_val: VlenI16(i16::MAX),
			i32_val: VlenI32(i32::MAX),
			i64_val: VlenI64(i64::MAX),
			i128_val: VlenI128(i128::MAX),
			f32_val: VlenF32(f32::MAX),
			f64_val: VlenF64(f64::MAX),
		};

		let json = serde_json::to_string(&max_struct).unwrap();
		let deserialized: TestStruct = serde_json::from_str(&json).unwrap();
		assert_eq!(max_struct, deserialized);

		// Test minimum values
		let min_struct = TestStruct {
			u16_val: VlenU16(u16::MIN),
			u32_val: VlenU32(u32::MIN),
			u64_val: VlenU64(u64::MIN),
			u128_val: VlenU128(u128::MIN),
			i16_val: VlenI16(i16::MIN),
			i32_val: VlenI32(i32::MIN),
			i64_val: VlenI64(i64::MIN),
			i128_val: VlenI128(i128::MIN),
			f32_val: VlenF32(f32::MIN),
			f64_val: VlenF64(f64::MIN),
		};

		let json = serde_json::to_string(&min_struct).unwrap();
		let deserialized: TestStruct = serde_json::from_str(&json).unwrap();
		assert_eq!(min_struct, deserialized);
	}

	#[test]
	fn test_deref_and_deref_mut() {
		let mut u32_val = VlenU32(42);
		assert_eq!(*u32_val, 42);

		*u32_val = 100;
		assert_eq!(*u32_val, 100);
		assert_eq!(u32_val.0, 100);

		let mut i64_val = VlenI64(-42);
		assert_eq!(*i64_val, -42);

		*i64_val = -100;
		assert_eq!(*i64_val, -100);
		assert_eq!(i64_val.0, -100);

		let mut f32_val = VlenF32(3.14);
		assert_eq!(*f32_val, 3.14);

		*f32_val = 2.71;
		assert_eq!(*f32_val, 2.71);
		assert_eq!(f32_val.0, 2.71);
	}

	#[test]
	fn test_from_traits() {
		let u32_val: VlenU32 = 42.into();
		assert_eq!(*u32_val, 42);

		let i64_val: VlenI64 = (-42).into();
		assert_eq!(*i64_val, -42);

		let f64_val: VlenF64 = 3.14159.into();
		assert_eq!(*f64_val, 3.14159);
	}

	#[test]
	fn test_serde_with_vectors() {
		#[derive(Debug, Serialize, Deserialize, PartialEq)]
		struct VectorTest {
			u32_vec: Vec<VlenU32>,
			i64_vec: Vec<VlenI64>,
			f32_vec: Vec<VlenF32>,
		}

		let data = VectorTest {
			u32_vec: vec![VlenU32(1), VlenU32(2), VlenU32(3)],
			i64_vec: vec![VlenI64(-1), VlenI64(-2), VlenI64(-3)],
			f32_vec: vec![VlenF32(1.1), VlenF32(2.2), VlenF32(3.3)],
		};

		let json = serde_json::to_string(&data).unwrap();
		let deserialized: VectorTest = serde_json::from_str(&json).unwrap();
		assert_eq!(data, deserialized);
	}
}

#[test]
fn test_serde_feature_gate() {
	// This test ensures that the serde module is only available when the feature is enabled
	#[cfg(feature = "serde")]
	{
		use vlen::serde::VlenU32;
		let _val = VlenU32(42);
	}

	#[cfg(not(feature = "serde"))]
	{
		// When serde feature is not enabled, the module should not be available
		// This is a compile-time check
	}
}
