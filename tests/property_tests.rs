use arbtest::arbtest;
use vlen::*;

// Helper macro for round-trip tests
macro_rules! round_trip_test {
	($name:ident, $type:ty, $encode_fn:ident, $decode_fn:ident, $buf_size:expr) => {
		#[test]
		fn $name() {
			arbtest(|u| {
				let value = u.arbitrary::<$type>()?;
				let mut buf = [0u8; $buf_size];
				let encoded_len = $encode_fn(&mut buf, value);
				let (decoded_value, decoded_len) = $decode_fn(&buf);
				assert_eq!(value, decoded_value);
				assert_eq!(encoded_len, decoded_len);
				Ok(())
			});
		}
	};
}

// Helper macro for size bounds tests
macro_rules! size_bounds_test {
	($name:ident, $type:ty, $encode_fn:ident, $buf_size:expr, $max_size:expr) => {
		#[test]
		fn $name() {
			arbtest(|u| {
				let value = u.arbitrary::<$type>()?;
				let mut buf = [0u8; $buf_size];
				let encoded_len = $encode_fn(&mut buf, value);
				assert!(encoded_len >= 1);
				assert!(encoded_len <= $max_size);
				if value < 128 {
					assert_eq!(encoded_len, 1);
				}
				Ok(())
			});
		}
	};
}

// Helper macro for signed integer size bounds tests
macro_rules! signed_size_bounds_test {
	($name:ident, $type:ty, $encode_fn:ident, $buf_size:expr, $max_size:expr) => {
		#[test]
		fn $name() {
			arbtest(|u| {
				let value = u.arbitrary::<$type>()?;
				let mut buf = [0u8; $buf_size];
				let encoded_len = $encode_fn(&mut buf, value);
				assert!(encoded_len >= 1);
				assert!(encoded_len <= $max_size);
				if value.abs() < 64 {
					assert_eq!(encoded_len, 1);
				}
				Ok(())
			});
		}
	};
}

// Helper macro for encoded size consistency tests
macro_rules! encoded_size_consistency_test {
	($name:ident, $type:ty, $encode_fn:ident, $encoded_size_fn:ident, $buf_size:expr) => {
		#[test]
		fn $name() {
			arbtest(|u| {
				let value = u.arbitrary::<$type>()?;
				let mut buf = [0u8; $buf_size];
				let actual_len = $encode_fn(&mut buf, value);
				let calculated_size = $encoded_size_fn(value);
				assert_eq!(actual_len, calculated_size);
				Ok(())
			});
		}
	};
}

// Helper macro for compression efficiency tests
macro_rules! compression_efficiency_test {
	($name:ident, $type:ty, $encode_fn:ident, $buf_size:expr) => {
		#[test]
		fn $name() {
			arbtest(|u| {
				let value = u.arbitrary::<$type>()?;
				let mut buf = [0u8; $buf_size];
				let encoded_len = $encode_fn(&mut buf, value);
				if value < 128 {
					assert_eq!(encoded_len, 1);
				}
				if (128..0x10000000).contains(&value) {
					assert!(encoded_len <= 4);
				}
				Ok(())
			});
		}
	};
}

// Helper macro for signed compression efficiency tests
macro_rules! signed_compression_efficiency_test {
	($name:ident, $type:ty, $encode_fn:ident, $buf_size:expr) => {
		#[test]
		fn $name() {
			arbtest(|u| {
				let value = u.arbitrary::<$type>()?;
				let mut buf = [0u8; $buf_size];
				let encoded_len = $encode_fn(&mut buf, value);
				if value.abs() < 64 {
					assert_eq!(encoded_len, 1);
				}
				if (64..0x10000000).contains(&value.abs()) {
					assert!(encoded_len <= 4);
				}
				Ok(())
			});
		}
	};
}

// Helper macro for floating-point round-trip tests
macro_rules! float_round_trip_test {
	($name:ident, $type:ty, $encode_fn:ident, $decode_fn:ident, $buf_size:expr) => {
		#[test]
		fn $name() {
			arbtest(|u| {
				let value = u.arbitrary::<$type>()?;
				let mut buf = [0u8; $buf_size];
				let encoded_len = $encode_fn(&mut buf, value);
				let (decoded_value, decoded_len) = $decode_fn(&buf);
				if value.is_nan() {
					assert!(decoded_value.is_nan());
				} else {
					assert_eq!(value, decoded_value);
				}
				assert_eq!(encoded_len, decoded_len);
				Ok(())
			});
		}
	};
}

// Generate all basic round-trip tests
round_trip_test!(test_u16_round_trip, u16, encode_u16, decode_u16, 3);
round_trip_test!(test_u32_round_trip, u32, encode_u32, decode_u32, 5);
round_trip_test!(test_u64_round_trip, u64, encode_u64, decode_u64, 9);
round_trip_test!(test_u128_round_trip, u128, encode_u128, decode_u128, 17);
round_trip_test!(test_i16_round_trip, i16, encode_i16, decode_i16, 3);
round_trip_test!(test_i32_round_trip, i32, encode_i32, decode_i32, 5);
round_trip_test!(test_i64_round_trip, i64, encode_i64, decode_i64, 9);
round_trip_test!(test_i128_round_trip, i128, encode_i128, decode_i128, 17);
float_round_trip_test!(test_f32_round_trip, f32, encode_f32, decode_f32, 5);
float_round_trip_test!(test_f64_round_trip, f64, encode_f64, decode_f64, 9);

// Generate all size bounds tests
size_bounds_test!(test_u16_size_bounds, u16, encode_u16, 3, 3);
size_bounds_test!(test_u32_size_bounds, u32, encode_u32, 5, 5);
size_bounds_test!(test_u64_size_bounds, u64, encode_u64, 9, 9);
size_bounds_test!(test_u128_size_bounds, u128, encode_u128, 17, 17);
signed_size_bounds_test!(test_i128_size_bounds, i128, encode_i128, 17, 17);

// Generate all encoded size consistency tests
encoded_size_consistency_test!(
	test_u16_encoded_size_consistency,
	u16,
	encode_u16,
	encoded_size_u16,
	3
);
encoded_size_consistency_test!(
	test_u32_encoded_size_consistency,
	u32,
	encode_u32,
	encoded_size_u32,
	5
);
encoded_size_consistency_test!(
	test_u64_encoded_size_consistency,
	u64,
	encode_u64,
	encoded_size_u64,
	9
);
encoded_size_consistency_test!(
	test_u128_encoded_size_consistency,
	u128,
	encode_u128,
	encoded_size_u128,
	17
);

// Generate all compression efficiency tests
compression_efficiency_test!(
	test_compression_efficiency_u32,
	u32,
	encode_u32,
	5
);
compression_efficiency_test!(
	test_compression_efficiency_u64,
	u64,
	encode_u64,
	9
);
signed_compression_efficiency_test!(
	test_i128_compression_efficiency,
	i128,
	encode_i128,
	17
);

// Specialized tests
#[test]
fn test_u32_prefix_consistency() {
	arbtest(|u| {
		let value = u.arbitrary::<u32>()?;
		let mut buf = [0u8; 5];
		let encoded_len = encode_u32(&mut buf, value);
		let prefix_len = vlen::encoded_len(buf[0]);
		assert_eq!(encoded_len, prefix_len);
		Ok(())
	});
}

#[test]
fn test_little_endian_encoding() {
	arbtest(|u| {
		let value = u.arbitrary::<u32>()?;
		let mut buf = [0u8; 5];
		let encoded_len = encode_u32(&mut buf, value);
		if encoded_len == 5 {
			let stored_value =
				u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]);
			assert_eq!(value, stored_value);
		}
		Ok(())
	});
}

#[test]
fn test_signed_integer_zigzag_encoding() {
	arbtest(|u| {
		let value = u.arbitrary::<i32>()?;
		let mut buf = [0u8; 5];
		let _ = encode_i32(&mut buf, value);
		if value < 0 {
			let zigzag_value = ((-value as u32) << 1) - 1;
			let (decoded_unsigned, _) = decode_u32(&buf);
			assert_eq!(zigzag_value, decoded_unsigned);
		} else {
			let zigzag_value = (value as u32) << 1;
			let (decoded_unsigned, _) = decode_u32(&buf);
			assert_eq!(zigzag_value, decoded_unsigned);
		}
		Ok(())
	});
}

#[test]
fn test_signed_integer_sign_preservation() {
	arbtest(|u| {
		let value = u.arbitrary::<i64>()?;
		let mut buf = [0u8; 9];
		let _ = encode_i64(&mut buf, value);
		let (decoded_value, _) = decode_i64(&buf);
		assert_eq!(value.signum(), decoded_value.signum());
		Ok(())
	});
}

#[test]
fn test_floating_point_infinity_handling() {
	arbtest(|u| {
		let value = if u.arbitrary::<bool>()? {
			f32::INFINITY
		} else {
			f32::NEG_INFINITY
		};
		let mut buf = [0u8; 5];
		let encoded_len = encode_f32(&mut buf, value);
		let (decoded_value, decoded_len) = decode_f32(&buf);
		assert_eq!(value, decoded_value);
		assert_eq!(encoded_len, decoded_len);
		Ok(())
	});
}

#[test]
fn test_floating_point_subnormal_handling() {
	arbtest(|u| {
		let value = f32::from_bits(u.arbitrary::<u32>()? & 0x007FFFFF);
		if value.is_subnormal() {
			let mut buf = [0u8; 5];
			let encoded_len = encode_f32(&mut buf, value);
			let (decoded_value, decoded_len) = decode_f32(&buf);
			assert_eq!(value, decoded_value);
			assert_eq!(encoded_len, decoded_len);
		}
		Ok(())
	});
}

#[test]
fn test_floating_point_byte_swapping() {
	arbtest(|u| {
		let value = u.arbitrary::<f64>()?;
		let mut buf = [0u8; 9];
		let _ = encode_f64(&mut buf, value);
		let (encoded_bits, _) = decode_u64(&buf);
		let original_bits = value.to_bits();
		assert_eq!(encoded_bits, original_bits.swap_bytes());
		Ok(())
	});
}

#[test]
fn test_bulk_encode_decode_round_trip() {
	arbtest(|u| {
		let values: Vec<u32> = (0..u.arbitrary::<u8>()? as usize % 5 + 1)
			.map(|_| u.arbitrary::<u32>().unwrap())
			.collect();
		let mut buf = [0u8; 25];
		let encoded_len = bulk_encode(&mut buf, &values).unwrap();
		let mut decode_buf = [0u8; 25];
		decode_buf[..encoded_len].copy_from_slice(&buf[..encoded_len]);
		let mut decoded_values = vec![0u32; values.len()];
		let decoded_len =
			bulk_decode(&decode_buf, &mut decoded_values).unwrap();
		assert_eq!(values, decoded_values);
		assert_eq!(encoded_len, decoded_len);
		Ok(())
	});
}

#[test]
fn test_bulk_encode_decode_mixed_types() {
	arbtest(|u| {
		let u32_values: Vec<u32> = (0..u.arbitrary::<u8>()? as usize % 3 + 1)
			.map(|_| u.arbitrary::<u32>().unwrap())
			.collect();
		let i32_values: Vec<i32> = (0..u.arbitrary::<u8>()? as usize % 3 + 1)
			.map(|_| u.arbitrary::<i32>().unwrap())
			.collect();

		let mut buf = [0u8; 15];
		let encoded_len = bulk_encode(&mut buf, &u32_values).unwrap();
		let mut decode_buf = [0u8; 15];
		decode_buf[..encoded_len].copy_from_slice(&buf[..encoded_len]);
		let mut decoded_values = vec![0u32; u32_values.len()];
		let decoded_len =
			bulk_decode(&decode_buf, &mut decoded_values).unwrap();
		assert_eq!(u32_values, decoded_values);
		assert_eq!(encoded_len, decoded_len);

		let mut buf = [0u8; 15];
		let encoded_len = bulk_encode(&mut buf, &i32_values).unwrap();
		let mut decode_buf = [0u8; 15];
		decode_buf[..encoded_len].copy_from_slice(&buf[..encoded_len]);
		let mut decoded_values = vec![0i32; i32_values.len()];
		let decoded_len =
			bulk_decode(&decode_buf, &mut decoded_values).unwrap();
		assert_eq!(i32_values, decoded_values);
		assert_eq!(encoded_len, decoded_len);
		Ok(())
	});
}

#[test]
#[cfg(feature = "alloc")]
fn test_convenience_functions_round_trip() {
	arbtest(|u| {
		let value = u.arbitrary::<u32>()?;
		let encoded = encode_to_vec(value).unwrap();
		let decoded = decode_value::<u32>(&encoded).unwrap();
		assert_eq!(value, decoded);
		Ok(())
	});
}

#[test]
#[cfg(feature = "alloc")]
fn test_bulk_convenience_functions_round_trip() {
	arbtest(|u| {
		let values: Vec<i32> = (0..u.arbitrary::<u8>()? as usize % 5 + 1)
			.map(|_| u.arbitrary::<i32>().unwrap())
			.collect();
		let encoded = bulk_encode_to_vec(&values).unwrap();
		let decoded = bulk_decode_values::<i32>(&encoded).unwrap();
		assert_eq!(values, decoded);
		Ok(())
	});
}

#[test]
fn test_edge_case_values() {
	arbtest(|_u| {
		let mut buf = [0u8; 5];

		let zero_u32 = 0u32;
		let encoded_len = encode_u32(&mut buf, zero_u32);
		let (decoded_value, _) = decode_u32(&buf);
		assert_eq!(zero_u32, decoded_value);
		assert_eq!(encoded_len, 1);

		let max_u32 = u32::MAX;
		let encoded_len = encode_u32(&mut buf, max_u32);
		let (decoded_value, _) = decode_u32(&buf);
		assert_eq!(max_u32, decoded_value);
		assert_eq!(encoded_len, 5);

		let boundary_values =
			[127u32, 128u32, 16383u32, 16384u32, 2097151u32, 2097152u32];
		for &value in &boundary_values {
			let _encoded_len = encode_u32(&mut buf, value);
			let (decoded_value, _) = decode_u32(&buf);
			assert_eq!(value, decoded_value);
		}
		Ok(())
	});
}

#[test]
fn test_buffer_overflow_handling() {
	arbtest(|u| {
		let value = u.arbitrary::<u32>()?;
		let required_size = encoded_size(value).unwrap();

		if required_size > 1 {
			let mut small_buf = [0u8; 1];
			let result = encode(&mut small_buf, value);
			assert!(result.is_err());
		}

		let mut adequate_buf = vec![0u8; 5];
		let result = encode(&mut adequate_buf, value);
		assert!(result.is_ok());

		let mut large_buf = vec![0u8; 10];
		let result = encode(&mut large_buf, value);
		assert!(result.is_ok());
		Ok(())
	});
}

#[test]
fn test_invalid_prefix_handling() {
	arbtest(|_u| {
		let buf = [0xFFu8; 5];
		let _result = decode_u32(&buf);
		Ok(())
	});
}

#[test]
fn test_truncated_data_handling() {
	arbtest(|u| {
		let value = u.arbitrary::<u32>()?;
		let mut buf = [0u8; 5];
		let encoded_len = encode_u32(&mut buf, value);

		if encoded_len > 1 {
			let mut truncated_buf = [0u8; 5];
			truncated_buf[..encoded_len - 1]
				.copy_from_slice(&buf[..encoded_len - 1]);
			let _result = decode_u32(&truncated_buf);
		}
		Ok(())
	});
}

#[test]
#[cfg(feature = "simd")]
fn test_simd_bulk_operations_when_available() {
	arbtest(|u| {
		let values: Vec<u32> = (0..u.arbitrary::<u8>()? as usize % 10 + 1)
			.map(|_| u.arbitrary::<u32>().unwrap())
			.collect();
		let mut buf = vec![0u8; values.len() * 5];
		let encoded_len = bulk_encode_u32_safe(&mut buf, &values).unwrap();
		buf.truncate(encoded_len);
		let mut decoded_values = vec![0u32; values.len()];
		let _decoded_len =
			bulk_decode_u32_safe(&buf, &mut decoded_values).unwrap();
		assert_eq!(values.len(), decoded_values.len());
		Ok(())
	});
}

#[test]
#[cfg(feature = "simd")]
fn test_simd_buffer_size_validation() {
	arbtest(|u| {
		let values: Vec<u32> = (0..u.arbitrary::<u8>()? as usize % 10 + 1)
			.map(|_| u.arbitrary::<u32>().unwrap())
			.collect();

		let mut small_buf = vec![0u8; values.len() * 2];
		let result = bulk_encode_u32_safe(&mut small_buf, &values);
		assert!(result.is_err());

		let mut adequate_buf = vec![0u8; values.len() * 5];
		let result = bulk_encode_u32_safe(&mut adequate_buf, &values);
		assert!(result.is_ok());
		Ok(())
	});
}
