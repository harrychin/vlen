use core::fmt;
use rstest::*;

macro_rules! assert_expected {
	($test_name:ident, $value:expr, $expect:expr, $got:expr) => {
		if $got != $expect {
			panic!(
				"{}({}): expected {}, got {}",
				stringify!($test_name),
				($value).arg_fmt(),
				($expect).arg_fmt(),
				($got).arg_fmt(),
			);
		}
	};
}

// Consolidated test cases
#[fixture]
fn u16_cases() -> Vec<(u16, &'static [u8])> {
	vec![
		(0x0000, &[0x00]),
		(0x007F, &[0x7F]),
		(0x0080, &[0b10000000, 0x02]),
		(0x3FFF, &[0b10111111, 0xFF]),
		(0x4000, &[0xDE, 0x00, 0x40]),
		(0xFFFF, &[0xDE, 0xFF, 0xFF]),
	]
}

#[fixture]
fn u32_cases() -> Vec<(u32, &'static [u8])> {
	vec![
		(0x00000000, &[0x00000000]),
		(0x0000007F, &[0x0000007F]),
		(0x00000080, &[0b10000000, 0x02]),
		(0x00003FFF, &[0b10111111, 0xFF]),
		(0x00004000, &[0b11000000, 0x00, 0x02]),
		(0x001FFFFF, &[0b11011111, 0xFF, 0xFF]),
		(0x00200000, &[0b11100000, 0x00, 0x00, 0x02]),
		(0x0FFFFFFF, &[0b11101111, 0xFF, 0xFF, 0xFF]),
		(0x10000000, &[0b11110011, 0x00, 0x00, 0x00, 0x10]),
		(0xFFFFFFFF, &[0b11110011, 0xFF, 0xFF, 0xFF, 0xFF]),
	]
}

#[fixture]
fn u64_cases() -> Vec<(u64, &'static [u8])> {
	vec![
		(0x00000000_00000000, &[0x00000000]),
		(0x00000000_0000007F, &[0x0000007F]),
		(0x00000000_00000080, &[0b10000000, 0x02]),
		(0x00000000_00003FFF, &[0b10111111, 0xFF]),
		(0x00000000_00004000, &[0b11000000, 0x00, 0x02]),
		(0x00000000_001FFFFF, &[0b11011111, 0xFF, 0xFF]),
		(0x00000000_00200000, &[0b11100000, 0x00, 0x00, 0x02]),
		(0x00000000_0FFFFFFF, &[0b11101111, 0xFF, 0xFF, 0xFF]),
		(0x00000000_10000000, &[0b11110011, 0x00, 0x00, 0x00, 0x10]),
		(0x00000000_FFFFFFFF, &[0b11110011, 0xFF, 0xFF, 0xFF, 0xFF]),
		(
			0x00000001_FFFFFFFF,
			&[0b11110100, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
		),
		(
			0x000000FF_FFFFFFFF,
			&[0b11110100, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
		),
		(
			0x000001FF_FFFFFFFF,
			&[0b11110101, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
		),
		(
			0x0000FFFF_FFFFFFFF,
			&[0b11110101, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
		),
		(
			0x0001FFFF_FFFFFFFF,
			&[0b11110110, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
		),
		(
			0x00FFFFFF_FFFFFFFF,
			&[0b11110110, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
		),
		(
			0x01FFFFFF_FFFFFFFF,
			&[0b11110111, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
		),
		(
			0xFFFFFFFF_FFFFFFFF,
			&[0b11110111, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
		),
	]
}

#[fixture]
fn i16_cases() -> Vec<(i16, &'static [u8])> {
	vec![
		(0x0000, &[0x00]),
		(0x007F, &[0xBE, 0x03]),
		(0x0080, &[0x80, 0x04]),
		(0x3FFF, &[0xDE, 0xFE, 0x7F]),
		(0x4000, &[0xDE, 0x00, 0x80]),
		(0x7FFF, &[0xDE, 0xFE, 0xFF]),
		(-0x0001, &[0x01]),
		(-0x007F, &[0xBD, 0x03]),
		(-0x0080, &[0xBF, 0x03]),
		(-0x3FFF, &[0xDE, 0xFD, 0x7F]),
		(-0x4000, &[0xDE, 0xFF, 0x7F]),
		(-0x7FFF, &[0xDE, 0xFD, 0xFF]),
		(-0x8000, &[0xDE, 0xFF, 0xFF]),
	]
}

#[fixture]
fn i32_cases() -> Vec<(i32, &'static [u8])> {
	vec![
		(0x00000000, &[0x00]),
		(0x0000007F, &[0xBE, 0x03]),
		(0x00000080, &[0x80, 0x04]),
		(0x000000FF, &[0xBE, 0x07]),
		(0x000001FF, &[0xBE, 0x0F]),
		(0x0000FFFF, &[0xDE, 0xFF, 0x0F]),
		(0x0001FFFF, &[0xDE, 0xFF, 0x1F]),
		(0x00FFFFFF, &[0xEE, 0xFF, 0xFF, 0x1F]),
		(0x01FFFFFF, &[0xEE, 0xFF, 0xFF, 0x3F]),
		(0xFFFFFFFFu32 as i32, &[0x01]),
		(0xFFFFFF00u32 as i32, &[0xBF, 0x07]),
		(0xFFFF0000u32 as i32, &[0xDF, 0xFF, 0x0F]),
		(0xFF000000u32 as i32, &[0xEF, 0xFF, 0xFF, 0x1F]),
		(0x80000000u32 as i32, &[0xF3, 0xFF, 0xFF, 0xFF, 0xFF]),
	]
}

#[fixture]
fn i64_cases() -> Vec<(i64, &'static [u8])> {
	vec![
		(0x00000000_00000000, &[0x00]),
		(0x00000000_0000007F, &[0xBE, 0x03]),
		(0x00000000_00000080, &[0x80, 0x04]),
		(0x00000000_000000FF, &[0xBE, 0x07]),
		(0x00000000_000001FF, &[0xBE, 0x0F]),
		(0x00000000_0000FFFF, &[0xDE, 0xFF, 0x0F]),
		(0x00000000_0001FFFF, &[0xDE, 0xFF, 0x1F]),
		(0x00000000_00FFFFFF, &[0xEE, 0xFF, 0xFF, 0x1F]),
		(0x00000000_01FFFFFF, &[0xEE, 0xFF, 0xFF, 0x3F]),
		(0x00000000_FFFFFFFF, &[0xF4, 0xFE, 0xFF, 0xFF, 0xFF, 0x01]),
		(0x00000001_FFFFFFFF, &[0xF4, 0xFE, 0xFF, 0xFF, 0xFF, 0x03]),
		(
			0x000000FF_FFFFFFFF,
			&[0xF5, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
		),
		(
			0x000001FF_FFFFFFFF,
			&[0xF5, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0x03],
		),
		(
			0x0000FFFF_FFFFFFFF,
			&[0xF6, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
		),
		(
			0x0001FFFF_FFFFFFFF,
			&[0xF6, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x03],
		),
		(
			0x00FFFFFF_FFFFFFFF,
			&[0xF7, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
		),
		(
			0x01FFFFFF_FFFFFFFF,
			&[0xF7, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x03],
		),
		(0xFFFFFFFF_FFFFFFFFu64 as i64, &[0x01]),
		(0xFFFFFFFF_FFFFFF00u64 as i64, &[0xBF, 0x07]),
		(0xFFFFFFFF_FFFF0000u64 as i64, &[0xDF, 0xFF, 0x0F]),
		(0xFFFFFFFF_FF000000u64 as i64, &[0xEF, 0xFF, 0xFF, 0x1F]),
		(
			0xFFFFFFFF_00000000u64 as i64,
			&[0xF4, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
		),
		(
			0xFFFFFF00_00000000u64 as i64,
			&[0xF5, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
		),
		(
			0xFFFF0000_00000000u64 as i64,
			&[0xF6, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
		),
		(
			0xFF000000_00000000u64 as i64,
			&[0xF7, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
		),
		(
			0x80000000_00000000u64 as i64,
			&[0xF7, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
		),
	]
}

#[fixture]
fn f32_cases() -> Vec<(f32, &'static [u8])> {
	vec![(0.0, &[0x00]), (-0.0, &[0x80, 0x02])]
}

#[fixture]
fn f64_cases() -> Vec<(f64, &'static [u8])> {
	vec![(0.0, &[0x00]), (-0.0, &[0x80, 0x02])]
}

// Generic encode/decode tests using fixtures
#[rstest]
fn test_encode_u16(u16_cases: Vec<(u16, &'static [u8])>) {
	for (value, expect) in u16_cases {
		let mut buf = [0u8; 3];
		let len = vlen::encode_u16(&mut buf, value);
		assert_expected!(encode_u16, value, expect, &buf[..len]);
	}
}

#[rstest]
fn test_decode_u16(u16_cases: Vec<(u16, &'static [u8])>) {
	for (expect, encoded_value) in u16_cases {
		let mut buf = [0u8; 3];
		buf[0..encoded_value.len()].copy_from_slice(encoded_value);
		let expect = (expect, encoded_value.len());
		for padding in [0u8, 255] {
			buf[encoded_value.len()..].fill(padding);
			let got = vlen::decode_u16(&buf);
			assert_expected!(decode_u16, encoded_value, expect, got);
		}
	}
}

#[rstest]
fn test_encode_u32(u32_cases: Vec<(u32, &'static [u8])>) {
	for (value, expect) in u32_cases {
		let mut buf = [0u8; 5];
		let len = vlen::encode_u32(&mut buf, value);
		assert_expected!(encode_u32, value, expect, &buf[..len]);
	}
}

#[rstest]
fn test_decode_u32(u32_cases: Vec<(u32, &'static [u8])>) {
	for (expect, encoded_value) in u32_cases {
		let mut buf = [0u8; 5];
		buf[0..encoded_value.len()].copy_from_slice(encoded_value);
		let expect = (expect, encoded_value.len());
		for padding in [0u8, 255] {
			buf[encoded_value.len()..].fill(padding);
			let got = vlen::decode_u32(&buf);
			assert_expected!(decode_u32, encoded_value, expect, got);
		}
	}
}

#[rstest]
fn test_encode_u64(u64_cases: Vec<(u64, &'static [u8])>) {
	for (value, expect) in u64_cases {
		let mut buf = [0u8; 9];
		let len = vlen::encode_u64(&mut buf, value);
		assert_expected!(encode_u64, value, expect, &buf[..len]);
	}
}

#[rstest]
fn test_decode_u64(u64_cases: Vec<(u64, &'static [u8])>) {
	for (expect, encoded_value) in u64_cases {
		let mut buf = [0u8; 9];
		buf[0..encoded_value.len()].copy_from_slice(encoded_value);
		let expect = (expect, encoded_value.len());
		for padding in [0u8, 255] {
			buf[encoded_value.len()..].fill(padding);
			let got = vlen::decode_u64(&buf);
			assert_expected!(decode_u64, encoded_value, expect, got);
		}
	}
}

#[rstest]
fn test_encode_i16(i16_cases: Vec<(i16, &'static [u8])>) {
	for (value, expect) in i16_cases {
		let mut buf = [0u8; 3];
		let len = vlen::encode_i16(&mut buf, value);
		assert_expected!(encode_i16, value, expect, &buf[..len]);
	}
}

#[rstest]
fn test_decode_i16(i16_cases: Vec<(i16, &'static [u8])>) {
	for (expect, encoded_value) in i16_cases {
		let mut buf = [0u8; 3];
		buf[0..encoded_value.len()].copy_from_slice(encoded_value);
		let expect = (expect, encoded_value.len());
		for padding in [0u8, 255] {
			buf[encoded_value.len()..].fill(padding);
			let got = vlen::decode_i16(&buf);
			assert_expected!(decode_i16, encoded_value, expect, got);
		}
	}
}

#[rstest]
fn test_encode_i32(i32_cases: Vec<(i32, &'static [u8])>) {
	for (value, expect) in i32_cases {
		let mut buf = [0u8; 5];
		let len = vlen::encode_i32(&mut buf, value);
		assert_expected!(encode_i32, value, expect, &buf[..len]);
	}
}

#[rstest]
fn test_decode_i32(i32_cases: Vec<(i32, &'static [u8])>) {
	for (expect, encoded_value) in i32_cases {
		let mut buf = [0u8; 5];
		buf[0..encoded_value.len()].copy_from_slice(encoded_value);
		let expect = (expect, encoded_value.len());
		for padding in [0u8, 255] {
			buf[encoded_value.len()..].fill(padding);
			let got = vlen::decode_i32(&buf);
			assert_expected!(decode_i32, encoded_value, expect, got);
		}
	}
}

#[rstest]
fn test_encode_i64(i64_cases: Vec<(i64, &'static [u8])>) {
	for (value, expect) in i64_cases {
		let mut buf = [0u8; 9];
		let len = vlen::encode_i64(&mut buf, value);
		assert_expected!(encode_i64, value, expect, &buf[..len]);
	}
}

#[rstest]
fn test_decode_i64(i64_cases: Vec<(i64, &'static [u8])>) {
	for (expect, encoded_value) in i64_cases {
		let mut buf = [0u8; 9];
		buf[0..encoded_value.len()].copy_from_slice(encoded_value);
		let expect = (expect, encoded_value.len());
		for padding in [0u8, 255] {
			buf[encoded_value.len()..].fill(padding);
			let got = vlen::decode_i64(&buf);
			assert_expected!(decode_i64, encoded_value, expect, got);
		}
	}
}

#[rstest]
fn test_encode_f32(f32_cases: Vec<(f32, &'static [u8])>) {
	for (value, expect) in f32_cases {
		let mut buf = [0u8; 5];
		let len = vlen::encode_f32(&mut buf, value);
		assert_expected!(encode_f32, value, expect, &buf[..len]);
	}
}

#[rstest]
fn test_decode_f32(f32_cases: Vec<(f32, &'static [u8])>) {
	for (expect, encoded_value) in f32_cases {
		let mut buf = [0u8; 5];
		buf[0..encoded_value.len()].copy_from_slice(encoded_value);
		let expect = (expect, encoded_value.len());
		for padding in [0u8, 255] {
			buf[encoded_value.len()..].fill(padding);
			let got = vlen::decode_f32(&buf);
			assert_expected!(decode_f32, encoded_value, expect, got);
		}
	}
}

#[rstest]
fn test_encode_f64(f64_cases: Vec<(f64, &'static [u8])>) {
	for (value, expect) in f64_cases {
		let mut buf = [0u8; 9];
		let len = vlen::encode_f64(&mut buf, value);
		assert_expected!(encode_f64, value, expect, &buf[..len]);
	}
}

#[rstest]
fn test_decode_f64(f64_cases: Vec<(f64, &'static [u8])>) {
	for (expect, encoded_value) in f64_cases {
		let mut buf = [0u8; 9];
		buf[0..encoded_value.len()].copy_from_slice(encoded_value);
		let expect = (expect, encoded_value.len());
		for padding in [0u8, 255] {
			buf[encoded_value.len()..].fill(padding);
			let got = vlen::decode_f64(&buf);
			assert_expected!(decode_f64, encoded_value, expect, got);
		}
	}
}

#[rstest]
fn test_encode_u128(
	u32_cases: Vec<(u32, &'static [u8])>,
	u64_cases: Vec<(u64, &'static [u8])>,
) {
	// Test u128 encoding with values that fit in u32 and u64
	for (value, expect) in u32_cases {
		let mut buf = [0u8; 17];
		let len = vlen::encode_u128(&mut buf, value as u128);
		assert_expected!(encode_u128, value, expect, &buf[..len]);
	}
	for (value, expect) in u64_cases {
		let mut buf = [0u8; 17];
		let len = vlen::encode_u128(&mut buf, value as u128);
		assert_expected!(encode_u128, value, expect, &buf[..len]);
	}
}

#[rstest]
fn test_decode_u128(
	u32_cases: Vec<(u32, &'static [u8])>,
	u64_cases: Vec<(u64, &'static [u8])>,
) {
	// Test u128 decoding with values that fit in u32 and u64
	for (expect, encoded_value) in u32_cases {
		let mut buf = [0u8; 17];
		buf[0..encoded_value.len()].copy_from_slice(encoded_value);
		let expect = (expect as u128, encoded_value.len());
		for padding in [0u8, 255] {
			buf[encoded_value.len()..].fill(padding);
			let got = vlen::decode_u128(&buf);
			assert_expected!(decode_u128, encoded_value, expect, got);
		}
	}
	for (expect, encoded_value) in u64_cases {
		let mut buf = [0u8; 17];
		buf[0..encoded_value.len()].copy_from_slice(encoded_value);
		let expect = (expect as u128, encoded_value.len());
		for padding in [0u8, 255] {
			buf[encoded_value.len()..].fill(padding);
			let got = vlen::decode_u128(&buf);
			assert_expected!(decode_u128, encoded_value, expect, got);
		}
	}
}

#[rstest]
fn test_encode_i128(
	i32_cases: Vec<(i32, &'static [u8])>,
	i64_cases: Vec<(i64, &'static [u8])>,
) {
	// Test i128 encoding with values that fit in i32 and i64
	for (value, expect) in i32_cases {
		let mut buf = [0u8; 17];
		let len = vlen::encode_i128(&mut buf, value as i128);
		assert_expected!(encode_i128, value, expect, &buf[..len]);
	}
	for (value, expect) in i64_cases {
		let mut buf = [0u8; 17];
		let len = vlen::encode_i128(&mut buf, value as i128);
		assert_expected!(encode_i128, value, expect, &buf[..len]);
	}
}

#[rstest]
fn test_decode_i128(
	i32_cases: Vec<(i32, &'static [u8])>,
	i64_cases: Vec<(i64, &'static [u8])>,
) {
	// Test i128 decoding with values that fit in i32 and i64
	for (expect, encoded_value) in i32_cases {
		let mut buf = [0u8; 17];
		buf[0..encoded_value.len()].copy_from_slice(encoded_value);
		let expect = (expect as i128, encoded_value.len());
		for padding in [0u8, 255] {
			buf[encoded_value.len()..].fill(padding);
			let got = vlen::decode_i128(&buf);
			assert_expected!(decode_i128, encoded_value, expect, got);
		}
	}
	for (expect, encoded_value) in i64_cases {
		let mut buf = [0u8; 17];
		buf[0..encoded_value.len()].copy_from_slice(encoded_value);
		let expect = (expect as i128, encoded_value.len());
		for padding in [0u8, 255] {
			buf[encoded_value.len()..].fill(padding);
			let got = vlen::decode_i128(&buf);
			assert_expected!(decode_i128, encoded_value, expect, got);
		}
	}
}

#[test]
fn test_overlong_encodings_u16() {
	let mut buf = [0u8; 3];
	// Test over-long encoding for u16
	let value = 0x1234u16;
	let len = vlen::encode_u16(&mut buf, value);
	assert_eq!(len, 2);
	assert_eq!(buf[0], 0xB4); // 0x80 | (0x34 & 0x3F) = 0x80 | 0x34 = 0xB4
	assert_eq!(buf[1], 0x48); // 0x1234 >> 6 = 0x48
}

#[test]
fn test_overlong_encodings() {
	let mut buf = [0u8; 5];
	// Test over-long encoding for u32
	let value = 0x12345678u32;
	let len = vlen::encode_u32(&mut buf, value);
	assert_eq!(len, 5);
	assert_eq!(buf[0], 0xF3);
	assert_eq!(buf[1], 0x78);
	assert_eq!(buf[2], 0x56);
	assert_eq!(buf[3], 0x34);
	assert_eq!(buf[4], 0x12);
}

#[test]
fn test_bulk_decode_u32() {
	let mut buf = [0u8; 20];
	let values = [1u32, 1000, 1000000, 1000000000];
	let encoded_len = unsafe { vlen::bulk_encode_u32(&mut buf, &values) };
	let mut decoded_values = [0u32; 4];
	let decoded_len = unsafe {
		vlen::bulk_decode_u32(&buf[..encoded_len], &mut decoded_values)
	};
	assert_eq!(decoded_len, encoded_len);
	assert_eq!(values, decoded_values);
}

#[test]
fn test_bulk_decode_u32_mixed() {
	let mut buf = [0u8; 20];
	let values = [1u32, 1000, 1000000, 1000000000];
	let encoded_len = unsafe { vlen::bulk_encode_u32(&mut buf, &values) };
	let mut decoded_values = [0u32; 4];
	let decoded_len = unsafe {
		vlen::bulk_decode_u32(&buf[..encoded_len], &mut decoded_values)
	};
	assert_eq!(decoded_len, encoded_len);
	assert_eq!(values, decoded_values);
}

#[test]
fn test_generic_encode_decode() {
	let mut buf = [0u8; 17];
	let value = 12345u32;
	let encoded_len = vlen::encode(&mut buf, value).unwrap();
	let (decoded_value, decoded_len) = vlen::decode::<u32>(&buf).unwrap();
	assert_eq!(value, decoded_value);
	assert_eq!(encoded_len, decoded_len);
}

#[test]
fn test_generic_encoded_size() {
	let value = 12345u32;
	let size = vlen::encoded_size(value).unwrap();
	let mut buf = [0u8; 17];
	let encoded_len = vlen::encode(&mut buf, value).unwrap();
	assert_eq!(size, encoded_len);
}

#[test]
fn test_manual_vs_bulk_encoding() {
	let values = [1u32, 1000, 1000000, 1000000000];
	let mut manual_buf = [0u8; 20];
	let mut bulk_buf = [0u8; 20];

	// Manual encoding
	let mut manual_offset = 0;
	for value in values {
		let len =
			vlen::encode(&mut manual_buf[manual_offset..], value).unwrap();
		manual_offset += len;
	}

	// Bulk encoding using generic function
	let bulk_offset = vlen::bulk_encode(&mut bulk_buf, &values).unwrap();

	// Verify the encoded data matches
	assert_eq!(&manual_buf[..manual_offset], &bulk_buf[..bulk_offset]);
	assert_eq!(manual_offset, bulk_offset);
}

#[test]
fn test_generic_bulk_operations() {
	let values = [1u32, 1000, 1000000, 1000000000];
	let mut buf = [0u8; 20];
	let encoded_len = vlen::bulk_encode(&mut buf, &values).unwrap();
	let mut decoded_values = [0u32; 4];
	let decoded_len =
		vlen::bulk_decode(&buf[..encoded_len], &mut decoded_values).unwrap();
	assert_eq!(decoded_len, encoded_len);
	assert_eq!(values, decoded_values);
}

#[test]
fn test_decode_value() {
	let mut buf = [0u8; 5];
	let value = 12345u32;
	let _len = vlen::encode_u32(&mut buf, value);
	let decoded_value = vlen::decode_value::<u32>(&buf).unwrap();
	assert_eq!(value, decoded_value);
}

#[test]
fn test_buffer_size_errors() {
	let mut buf = [0u8; 1];
	let value = 12345u32;
	let result = vlen::encode(&mut buf, value);
	assert!(result.is_err());
}

#[test]
fn test_safe_bulk_operations() {
	let values = [1u32, 1000, 1000000, 1000000000];
	let mut buf = [0u8; 20];
	let encoded_len = vlen::bulk_encode_u32_safe(&mut buf, &values).unwrap();
	let mut decoded_values = [0u32; 4];
	let decoded_len =
		vlen::bulk_decode_u32_safe(&buf[..encoded_len], &mut decoded_values)
			.unwrap();
	assert_eq!(decoded_len, encoded_len);
	assert_eq!(values, decoded_values);
}

#[test]
fn test_safe_bulk_buffer_too_small() {
	let values = [1u32, 1000, 1000000, 1000000000];
	let mut buf = [0u8; 5];
	let result = vlen::bulk_encode_u32_safe(&mut buf, &values);
	assert!(result.is_err());
}

trait ArgFmt: fmt::Debug {
	fn arg_fmt(&self) -> String {
		format!("{self:?}")
	}
}

impl ArgFmt for u16 {}
impl ArgFmt for u32 {}
impl ArgFmt for u64 {}
impl ArgFmt for u128 {}
impl ArgFmt for i16 {}
impl ArgFmt for i32 {}
impl ArgFmt for i64 {}
impl ArgFmt for i128 {}
impl ArgFmt for f32 {}
impl ArgFmt for f64 {}
impl ArgFmt for usize {}

// Add implementations for tuples and slices used in tests
impl<T: ArgFmt, U: ArgFmt> ArgFmt for (T, U) {}
impl ArgFmt for &[u8] {
	fn arg_fmt(&self) -> String {
		format!("{:?}", HexArray(self))
	}
}

struct HexArray<'a>(&'a [u8]);

impl fmt::Debug for HexArray<'_> {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
		write!(fmt, "[")?;
		for (i, &byte) in self.0.iter().enumerate() {
			if i > 0 {
				write!(fmt, ", ")?;
			}
			write!(fmt, "0x{byte:02X}")?;
		}
		write!(fmt, "]")
	}
}
