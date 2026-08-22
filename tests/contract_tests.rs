//! Contract tests for the checked (slice-based) API.
//!
//! These pin the guarantees that make `vlen` safe to use on untrusted
//! input: decoding needs only the bytes a value actually occupies,
//! encoding fits exactly-sized buffers, invalid prefixes are rejected
//! instead of desynchronizing the stream, and the bulk operations are
//! byte-for-byte compatible with the per-value scalar codec.

use vlen::{
	Decode, Encode, Error, StrictError, bulk_decode, bulk_decode_u32,
	bulk_decode_u64, bulk_encode, bulk_encode_u32, bulk_encode_u64,
	decode_iter,
};

#[test]
fn round_trip_from_exact_slices() {
	fn check<T: Encode + Decode + PartialEq + core::fmt::Debug>(value: T) {
		let mut buf = [0u8; 17];
		let len = value.encode(&mut buf).unwrap();
		assert_eq!(len, value.encoded_size());

		// Decoding must succeed from a slice holding only the value.
		let exact = &buf[..len];
		let (decoded, decoded_len) = T::decode(exact).unwrap();
		assert_eq!(decoded, value);
		assert_eq!(decoded_len, len);
		assert_eq!(vlen::decode_strict::<T>(exact), Ok(value));

		// Encoding must succeed into a buffer of exactly the right size.
		let mut tight = vec![0u8; len];
		let tight_len = value.encode(&mut tight).unwrap();
		assert_eq!(tight_len, len);
		assert_eq!(&tight[..], exact);
	}

	for value in [0u16, 0x7F, 0x80, 0x3FFF, 0x4000, u16::MAX] {
		check(value);
	}
	for value in [0u32, 0x7F, 0x4000, 0x200000, 0x10000000, u32::MAX] {
		check(value);
	}
	for value in [0u64, 0x7F, 0x10000000, u32::MAX as u64 + 1, u64::MAX] {
		check(value);
	}
	for value in [0u128, 0x7F, u64::MAX as u128 + 1, u128::MAX] {
		check(value);
	}
	for value in [0i16, 1, -1, i16::MIN, i16::MAX] {
		check(value);
	}
	for value in [0i32, 1, -1, i32::MIN, i32::MAX] {
		check(value);
	}
	for value in [0i64, 1, -1, i64::MIN, i64::MAX] {
		check(value);
	}
	for value in [0i128, 1, -1, i128::MIN, i128::MAX] {
		check(value);
	}
	for value in [0.0f32, -0.0, 1.5, f32::MAX, f32::INFINITY] {
		check(value);
	}
	for value in [0.0f64, -0.0, 1.5, f64::MAX, f64::NEG_INFINITY] {
		check(value);
	}
}

#[test]
fn decode_empty_buffer_is_an_error() {
	assert_eq!(
		u32::decode(&[]),
		Err(Error::BufferTooSmall {
			needed: 1,
			available: 0
		})
	);
}

#[test]
fn decode_truncated_value_is_an_error() {
	let mut buf = [0u8; 5];
	let len = vlen::encode_u32(&mut buf, 0x12345678);
	assert_eq!(len, 5);
	assert_eq!(
		u32::decode(&buf[..3]),
		Err(Error::BufferTooSmall {
			needed: 5,
			available: 3
		})
	);
}

#[test]
fn exact_decode_rejects_trailing_bytes_but_accepts_overlong_values() {
	assert_eq!(vlen::decode_exact::<u32>(&[5]), Ok(5));
	assert_eq!(
		vlen::decode_exact::<u32>(&[5, 6]),
		Err(StrictError::TrailingBytes {
			consumed: 1,
			available: 2,
		})
	);
	// Exact consumption and canonicality are independent validation modes.
	assert_eq!(vlen::decode_exact::<u32>(&[0x85, 0]), Ok(5));
}

#[test]
fn canonical_decode_rejects_overlong_values_but_allows_a_trailing_value() {
	assert_eq!(
		vlen::decode_canonical::<u32>(&[0x85, 0]),
		Err(StrictError::NonCanonical {
			encoded_len: 2,
			canonical_len: 1,
		})
	);
	assert_eq!(vlen::decode_canonical::<u32>(&[5, 6]), Ok((5, 1)));

	// Binary prefixes below 2^28 are non-canonical even when the alternate
	// form happens to use the same number of bytes as the canonical encoding.
	for (bytes, value) in [
		(&[0xF0, 0x80][..], 0x80),
		(&[0xF1, 0x00, 0x40][..], 0x4000),
		(&[0xF2, 0x00, 0x00, 0x20][..], 0x20_0000),
	] {
		assert_eq!(u32::decode(bytes), Ok((value, bytes.len())));
		assert_eq!(
			vlen::decode_canonical::<u32>(bytes),
			Err(StrictError::NonCanonical {
				encoded_len: bytes.len(),
				canonical_len: bytes.len(),
			})
		);
	}
}

#[test]
fn strict_decode_requires_one_canonical_whole_value() {
	assert_eq!(vlen::decode_strict::<u32>(&[5]), Ok(5));
	assert_eq!(
		vlen::decode_strict::<u32>(&[5, 6]),
		Err(StrictError::TrailingBytes {
			consumed: 1,
			available: 2,
		})
	);
	assert_eq!(
		vlen::decode_strict::<u32>(&[0x85, 0]),
		Err(StrictError::NonCanonical {
			encoded_len: 2,
			canonical_len: 1,
		})
	);
	assert_eq!(
		vlen::decode_strict::<u32>(&[0xF9]),
		Err(StrictError::Decode(Error::InvalidPrefix { prefix: 0xF9 }))
	);
}

#[test]
fn decode_rejects_prefixes_too_long_for_the_type() {
	// 0xF4 announces a 6-byte encoding: valid for u64/u128, not u32.
	let six_byte = [0xF4u8, 1, 0, 0, 0, 1];
	assert_eq!(
		u32::decode(&six_byte),
		Err(Error::InvalidPrefix { prefix: 0xF4 })
	);
	assert!(u64::decode(&six_byte).is_ok());

	// 0xF8 announces a 10-byte encoding: valid only for u128.
	let ten_byte = [0xF8u8, 1, 0, 0, 0, 0, 0, 0, 0, 1];
	assert_eq!(
		u64::decode(&ten_byte),
		Err(Error::InvalidPrefix { prefix: 0xF8 })
	);
	assert!(u128::decode(&ten_byte).is_ok());

	// Anything announcing four or more bytes is invalid for u16.
	assert_eq!(
		u16::decode(&[0xE0u8, 0, 0, 0]),
		Err(Error::InvalidPrefix { prefix: 0xE0 })
	);
	assert_eq!(
		u16::decode(&[0xFFu8, 0, 0]),
		Err(Error::InvalidPrefix { prefix: 0xFF })
	);
}

#[test]
fn decode_u16_rejects_out_of_range_values() {
	// A 3-byte prefix varint can hold up to 2^21 - 1; values above
	// u16::MAX must be rejected rather than silently truncated.
	let mut buf = [0u8; 5];
	let len = vlen::encode_u32(&mut buf, 0x10000);
	assert_eq!(len, 3);
	assert_eq!(u16::decode(&buf[..len]), Err(Error::Overflow));
}

#[test]
fn u16_shares_the_u32_wire_format() {
	// A value encoded as u16 must decode as u32/u64/u128 and vice
	// versa; every width shares one grammar.
	for value in [0u16, 0x7F, 0x80, 0x3FFF, 0x4000, 0xABCD, u16::MAX] {
		let mut buf16 = [0u8; 3];
		let mut buf32 = [0u8; 5];
		let len16 = vlen::encode_u16(&mut buf16, value);
		let len32 = vlen::encode_u32(&mut buf32, value as u32);
		assert_eq!(len16, len32);
		assert_eq!(&buf16[..len16], &buf32[..len32]);

		let (as_u64, _) = u64::decode(&buf16[..len16]).unwrap();
		assert_eq!(as_u64, value as u64);
		let (as_u16, _) = u16::decode(&buf32[..len32]).unwrap();
		assert_eq!(as_u16, value);
	}
}

#[test]
fn narrow_decoders_accept_short_binary_prefix_encodings() {
	// The binary-prefix grammar is shared across widths, including its
	// over-long forms. Results must not depend on whether trailing bytes are
	// available to the checked decoder.
	assert_eq!(u8::decode(&[0xF0, 0xFF]), Ok((0xFF, 2)));
	assert_eq!(u16::decode(&[0xF0, 0x3D]), Ok((0x3D, 2)));
	assert_eq!(u16::decode(&[0xF0, 0x3D, 0]), Ok((0x3D, 2)));
	assert_eq!(u16::decode(&[0xF1, 0x34, 0x12]), Ok((0x1234, 3)));
	assert_eq!(vlen::decode_u8(&[0xF0, 0xFF]), (0xFF, 2));
	assert_eq!(vlen::decode_u16(&[0xF0, 0x3D, 0]), (0x3D, 2));
	assert_eq!(vlen::decode_u16(&[0xF1, 0x34, 0x12]), (0x1234, 3));
}

#[test]
fn encode_into_too_small_buffer_is_an_error() {
	let mut buf = [0u8; 2];
	assert_eq!(
		0x12345678u32.encode(&mut buf),
		Err(Error::BufferTooSmall {
			needed: 5,
			available: 2
		})
	);
	// A small value still fits in the same buffer.
	assert_eq!(5u32.encode(&mut buf), Ok(1));
}

#[test]
fn bulk_round_trip_through_exactly_sized_buffer() {
	let values = [
		0u32,
		1,
		0x7F,
		0x80,
		0x3FFF,
		0x4000,
		0x1FFFFF,
		0x200000,
		0xFFFFFFF,
		0x10000000,
		u32::MAX,
	];
	let total: usize = values.iter().map(|v| v.encoded_size()).sum();

	// Guard bytes after the exactly-sized buffer must stay untouched.
	let mut backing = vec![0xAAu8; total + 8];
	let written = bulk_encode(&mut backing[..total], &values).unwrap();
	assert_eq!(written, total);
	assert!(backing[total..].iter().all(|&b| b == 0xAA));

	let mut decoded = [0u32; 11];
	let read = bulk_decode(&backing[..total], &mut decoded).unwrap();
	assert_eq!(read, total);
	assert_eq!(decoded, values);
}

#[test]
fn bulk_decode_of_truncated_input_is_an_error() {
	let values = [1u32, 70000, 2];
	let mut buf = [0u8; 15];
	let len = bulk_encode(&mut buf, &values).unwrap();
	let mut out = [0u32; 3];
	assert!(bulk_decode(&buf[..len - 1], &mut out).is_err());
}

#[test]
fn specialized_u32_bulk_matches_generic_bulk() {
	// Mixed sizes, including long runs of one-byte and two-byte values
	// that hit the fast paths, must produce canonical bytes and
	// round-trip.
	let mut values = Vec::new();
	for i in 0..64u32 {
		values.push(i % 0x50);
	}
	values.extend([0x80, 0x3FFF, 0x4000, 0x1FFFFF, 0x10000000, u32::MAX]);
	for i in 0..64u32 {
		values.push(0x80 + (i * 37) % 0x3F80);
	}
	values.extend([1, 0x4000, 2]);
	for i in 0..64u32 {
		values.push(0x4000 + (i * 97) % 0x1F_C000);
	}
	values.extend([5, u32::MAX]);
	for i in 0..64u32 {
		values.push(0x20_0000 + (i * 997) % 0xFE0_0000);
	}
	for i in 0..64u32 {
		values.push(0x1000_0000 + (i * 9973) % 0xF000_0000);
	}
	values.extend([7, 0x1234]);
	for i in 0..64u32 {
		values.push(i % 0x50);
	}

	let mut generic = vec![0u8; values.len() * 5];
	let generic_len = bulk_encode(&mut generic, &values).unwrap();
	let mut specialized = vec![0u8; values.len() * 5];
	let specialized_len = bulk_encode_u32(&mut specialized, &values).unwrap();
	assert_eq!(specialized_len, generic_len);
	assert_eq!(&specialized[..specialized_len], &generic[..generic_len]);

	let mut decoded = vec![0u32; values.len()];
	let read =
		bulk_decode_u32(&specialized[..specialized_len], &mut decoded).unwrap();
	assert_eq!(read, specialized_len);
	assert_eq!(decoded, values);

	// The specialized decoder must accept generic output and vice versa.
	let mut decoded2 = vec![0u32; values.len()];
	bulk_decode_u32(&generic[..generic_len], &mut decoded2).unwrap();
	assert_eq!(decoded2, values);
	let mut decoded3 = vec![0u32; values.len()];
	bulk_decode(&specialized[..specialized_len], &mut decoded3).unwrap();
	assert_eq!(decoded3, values);
}

#[test]
fn specialized_u64_bulk_matches_generic_bulk() {
	// Runs of one-byte and two-byte values interleaved with every
	// larger size class, compared against the generic codec.
	let mut values = Vec::new();
	for i in 0..32u64 {
		values.push(i % 0x50);
	}
	for i in 0..32u64 {
		values.push(0x80 + (i * 41) % 0x3F80);
	}
	values.extend([
		0x4000,
		0x1FFFFF,
		0x10000000,
		u32::MAX as u64 + 1,
		u64::MAX,
		0,
	]);
	for i in 0..32u64 {
		values.push(0x4000 + (i * 173) % 0x1F_C000);
	}
	for i in 0..32u64 {
		values.push(0x1000_0000 + (i * 9973) % 0xF000_0000);
	}
	// Runs of wide binary-prefix encodings (six and nine bytes).
	for i in 0..32u64 {
		values.push(0x1_0000_0000 + i * 0x100);
	}
	for i in 0..32u64 {
		values.push(u64::MAX - i * 0x1_0000);
	}
	for i in 0..32u64 {
		values.push(i % 0x50);
	}

	let mut generic = vec![0u8; values.len() * 9];
	let generic_len = bulk_encode(&mut generic, &values).unwrap();
	let mut specialized = vec![0u8; values.len() * 9];
	let specialized_len = bulk_encode_u64(&mut specialized, &values).unwrap();
	assert_eq!(specialized_len, generic_len);
	assert_eq!(&specialized[..specialized_len], &generic[..generic_len]);

	let mut decoded = vec![0u64; values.len()];
	let read =
		bulk_decode_u64(&specialized[..specialized_len], &mut decoded).unwrap();
	assert_eq!(read, specialized_len);
	assert_eq!(decoded, values);

	// Cross-compatibility with the generic path in both directions.
	let mut decoded2 = vec![0u64; values.len()];
	bulk_decode_u64(&generic[..generic_len], &mut decoded2).unwrap();
	assert_eq!(decoded2, values);
	let mut decoded3 = vec![0u64; values.len()];
	bulk_decode(&specialized[..specialized_len], &mut decoded3).unwrap();
	assert_eq!(decoded3, values);
}

#[test]
fn two_byte_run_decode_rejects_invalid_interior() {
	// A stream that starts like a two-byte run but is truncated inside
	// a later value must error, not desynchronize.
	let values = [0x100u32, 0x200, 0x300, 0x400, 0x12345678];
	let mut buf = [0u8; 25];
	let len = bulk_encode(&mut buf, &values).unwrap();
	let mut out = [0u32; 5];
	assert!(bulk_decode_u32(&buf[..len - 2], &mut out).is_err());
}

#[test]
fn decode_iter_yields_each_value() {
	let values = [3u64, 0x4000, u64::MAX, 0, 42];
	let mut buf = [0u8; 45];
	let len = bulk_encode(&mut buf, &values).unwrap();

	let decoded: Result<Vec<u64>, Error> = decode_iter(&buf[..len]).collect();
	assert_eq!(decoded.unwrap(), values);
}

#[test]
fn decode_iter_reports_errors_and_stops() {
	// A prefix invalid for u32 mid-stream must surface as an error.
	let buf = [5u8, 0xF9, 0, 0, 0, 0];
	let mut iter = decode_iter::<u32>(&buf);
	assert_eq!(iter.next(), Some(Ok(5)));
	assert_eq!(
		iter.next(),
		Some(Err(Error::InvalidPrefix { prefix: 0xF9 }))
	);
	assert_eq!(iter.next(), None);
}

#[test]
fn decode_iter_size_hints_allow_an_immediate_error() {
	// Six bytes can hold at least two valid u32 encodings, but a malformed
	// first prefix makes the iterator yield one terminal error and stop.
	let malformed = [0xF9, 0, 0, 0, 0, 0];
	assert_eq!(decode_iter::<u32>(&malformed).size_hint(), (1, Some(6)));
	assert_eq!(vlen::decode_iter_u32(&malformed).size_hint(), (1, Some(6)));
	assert_eq!(vlen::decode_iter_i32(&malformed).size_hint(), (1, Some(6)));
}

#[test]
fn error_implements_display_and_error() {
	fn assert_error<E: core::error::Error>(_: &E) {}
	let err = Error::InvalidPrefix { prefix: 0xF9 };
	assert_error(&err);
	assert!(!format!("{err}").is_empty());
}

#[test]
fn strict_error_displays_context_and_exposes_decode_source() {
	fn assert_error<E: core::error::Error>(_: &E) {}

	let decode = StrictError::from(Error::InvalidPrefix { prefix: 0xF9 });
	let non_canonical = StrictError::NonCanonical {
		encoded_len: 2,
		canonical_len: 1,
	};
	let trailing = StrictError::TrailingBytes {
		consumed: 1,
		available: 2,
	};
	for error in [decode, non_canonical, trailing] {
		assert_error(&error);
		assert!(!format!("{error}").is_empty());
	}
	assert!(core::error::Error::source(&decode).is_some());
	assert!(core::error::Error::source(&non_canonical).is_none());
	assert!(core::error::Error::source(&trailing).is_none());

	let same_length = StrictError::NonCanonical {
		encoded_len: 2,
		canonical_len: 2,
	};
	assert!(format!("{same_length}").contains("differs"));
}

#[cfg(feature = "alloc")]
#[test]
fn vec_convenience_functions_round_trip() {
	let encoded = vlen::encode_to_vec(0x12345u32);
	assert_eq!(encoded.len(), 0x12345u32.encoded_size());
	assert_eq!(vlen::decode_value::<u32>(&encoded), Ok(0x12345));

	let values = [-5i32, 0, 1, i32::MIN, i32::MAX];
	let encoded = vlen::bulk_encode_to_vec(&values);
	let decoded = vlen::bulk_decode_values::<i32>(&encoded).unwrap();
	assert_eq!(decoded, values);
}

#[test]
fn invalid_prefix_wins_over_short_buffer() {
	// A prefix invalid for the type is reported as InvalidPrefix even
	// when the buffer is also shorter than the announced encoding.
	assert_eq!(
		u32::decode(&[0xF7u8]),
		Err(Error::InvalidPrefix { prefix: 0xF7 })
	);
	assert_eq!(
		u16::decode(&[0xE5u8]),
		Err(Error::InvalidPrefix { prefix: 0xE5 })
	);
	// A prefix valid for the type but with missing bytes stays
	// BufferTooSmall.
	assert_eq!(
		u32::decode(&[0xF3u8]),
		Err(Error::BufferTooSmall {
			needed: 5,
			available: 1
		})
	);
	// u128 accepts every prefix; only truncation can fail it.
	assert_eq!(
		u128::decode(&[0xFFu8]),
		Err(Error::BufferTooSmall {
			needed: 17,
			available: 1
		})
	);
}

#[test]
fn specialized_signed_bulk_matches_generic_bulk() {
	// Delta-style signed streams: runs of small magnitudes (the
	// zigzag one- and two-byte classes) mixed with every wider class.
	let mut i32_values = Vec::new();
	for i in 0..64i32 {
		i32_values.push((i % 63) - 31);
	}
	for i in 0..64i32 {
		i32_values.push(((i * 37) % 0x1F00) - 0xF80);
	}
	i32_values.extend([0, 1, -1, 0x40, -0x41, i32::MIN, i32::MAX]);
	for i in 0..64i32 {
		i32_values.push((i % 63) - 31);
	}

	let mut generic = vec![0u8; i32_values.len() * 5];
	let generic_len = vlen::bulk_encode(&mut generic, &i32_values).unwrap();
	let mut specialized = vec![0u8; i32_values.len() * 5];
	let specialized_len =
		vlen::bulk_encode_i32(&mut specialized, &i32_values).unwrap();
	assert_eq!(specialized_len, generic_len);
	assert_eq!(&specialized[..specialized_len], &generic[..generic_len]);

	let mut decoded = vec![0i32; i32_values.len()];
	let read =
		vlen::bulk_decode_i32(&generic[..generic_len], &mut decoded).unwrap();
	assert_eq!(read, generic_len);
	assert_eq!(decoded, i32_values);

	let i64_values: Vec<i64> = i32_values
		.iter()
		.map(|&v| v as i64)
		.chain([i64::MIN, i64::MAX, -0x1_0000_0000])
		.collect();
	let mut generic = vec![0u8; i64_values.len() * 9];
	let generic_len = vlen::bulk_encode(&mut generic, &i64_values).unwrap();
	let mut specialized = vec![0u8; i64_values.len() * 9];
	let specialized_len =
		vlen::bulk_encode_i64(&mut specialized, &i64_values).unwrap();
	assert_eq!(specialized_len, generic_len);
	assert_eq!(&specialized[..specialized_len], &generic[..generic_len]);

	let mut decoded = vec![0i64; i64_values.len()];
	let read =
		vlen::bulk_decode_i64(&generic[..generic_len], &mut decoded).unwrap();
	assert_eq!(read, generic_len);
	assert_eq!(decoded, i64_values);
}

#[test]
fn specialized_decode_iter_matches_generic_iter() {
	// Runs of every size class plus hostile interleaving; the
	// specialized iterator must yield exactly what the generic one
	// yields.
	let mut values = Vec::new();
	for i in 0..64u32 {
		values.push(i % 0x50);
	}
	for i in 0..64u32 {
		values.push(0x80 + (i * 37) % 0x3F80);
	}
	values.extend([1, 0x12345678, 0x4000, 7]);
	for i in 0..64u32 {
		values.push(0x1000_0000 + (i * 9973) % 0xF000_0000);
	}

	let mut buf = vec![0u8; values.len() * 5];
	let len = bulk_encode(&mut buf, &values).unwrap();
	let encoded = &buf[..len];

	let generic: Vec<u32> =
		decode_iter(encoded).collect::<Result<_, _>>().unwrap();
	let specialized: Vec<u32> = vlen::decode_iter_u32(encoded)
		.collect::<Result<_, _>>()
		.unwrap();
	assert_eq!(generic, values);
	assert_eq!(specialized, values);

	let wide: Vec<u64> = values.iter().map(|&v| v as u64).collect();
	let specialized64: Vec<u64> = vlen::decode_iter_u64(encoded)
		.collect::<Result<_, _>>()
		.unwrap();
	assert_eq!(specialized64, wide);
}

#[test]
fn specialized_decode_iter_reports_errors_and_stops() {
	// Same error semantics as the generic iterator: one Err, then
	// fused None — including when the error follows buffered values.
	let buf = [5u8, 6, 7, 8, 9, 10, 11, 12, 0xF9, 0, 0, 0, 0];
	let mut iter = vlen::decode_iter_u32(&buf);
	for expect in 5u32..=12 {
		assert_eq!(iter.next(), Some(Ok(expect)));
	}
	assert_eq!(
		iter.next(),
		Some(Err(Error::InvalidPrefix { prefix: 0xF9 }))
	);
	assert_eq!(iter.next(), None);
	assert_eq!(iter.next(), None);
}

#[test]
fn signed_run_iter_matches_generic_iter() {
	let mut values = Vec::new();
	for i in 0..64i64 {
		values.push((i % 63) - 31);
	}
	values.extend([i64::MIN, i64::MAX, 0, -1]);
	for i in 0..64i64 {
		values.push(((i * 41) % 0x1F00) - 0xF80);
	}

	let mut buf = vec![0u8; values.len() * 9];
	let len = bulk_encode(&mut buf, &values).unwrap();
	let encoded = &buf[..len];

	let generic: Vec<i64> =
		decode_iter(encoded).collect::<Result<_, _>>().unwrap();
	let specialized: Vec<i64> = vlen::decode_iter_i64(encoded)
		.collect::<Result<_, _>>()
		.unwrap();
	assert_eq!(generic, values);
	assert_eq!(specialized, values);

	let narrow: Vec<i32> = values
		.iter()
		.filter(|v| i32::try_from(**v).is_ok())
		.map(|&v| v as i32)
		.collect();
	let mut buf32 = vec![0u8; narrow.len() * 5];
	let len32 = bulk_encode(&mut buf32, &narrow).unwrap();
	let specialized32: Vec<i32> = vlen::decode_iter_i32(&buf32[..len32])
		.collect::<Result<_, _>>()
		.unwrap();
	assert_eq!(specialized32, narrow);
}

#[test]
fn byte_and_pointer_width_types_round_trip() {
	// u8/i8 share the u16 grammar; usize/isize share u64/i64, so the
	// wire format stays identical across platforms.
	for value in [0u8, 1, 0x7F, 0x80, u8::MAX] {
		let mut buf = [0u8; 2];
		let len = value.encode(&mut buf).unwrap();
		assert_eq!(len, value.encoded_size());
		let (back, back_len) = u8::decode(&buf[..len]).unwrap();
		assert_eq!((back, back_len), (value, len));
		// Byte-identical with the u16 encoding of the same value.
		let mut wide = [0u8; 3];
		let wide_len = vlen::encode_u16(&mut wide, value as u16);
		assert_eq!(&wide[..wide_len], &buf[..len]);
	}
	for value in [0i8, 1, -1, i8::MIN, i8::MAX] {
		let mut buf = [0u8; 2];
		let len = value.encode(&mut buf).unwrap();
		let (back, _) = i8::decode(&buf[..len]).unwrap();
		assert_eq!(back, value);
	}
	for value in [0usize, 1, 0x7F, 0xFFFF, usize::MAX] {
		let mut buf = [0u8; 9];
		let len = value.encode(&mut buf).unwrap();
		assert_eq!(len, value.encoded_size());
		let (back, _) = usize::decode(&buf[..len]).unwrap();
		assert_eq!(back, value);
		// Byte-identical with the u64 encoding.
		let mut wide = [0u8; 9];
		let wide_len = vlen::encode_u64(&mut wide, value as u64);
		assert_eq!(&wide[..wide_len], &buf[..len]);
	}
	for value in [0isize, 1, -1, isize::MIN, isize::MAX] {
		let mut buf = [0u8; 9];
		let len = value.encode(&mut buf).unwrap();
		let (back, _) = isize::decode(&buf[..len]).unwrap();
		assert_eq!(back, value);
	}
}

#[test]
fn byte_types_reject_out_of_range_and_invalid() {
	// A two-byte encoding carrying more than u8::MAX.
	let mut buf = [0u8; 3];
	let len = vlen::encode_u16(&mut buf, 0x100);
	assert_eq!(u8::decode(&buf[..len]), Err(Error::Overflow));
	// Prefixes announcing three or more bytes are invalid for u8.
	assert_eq!(
		u8::decode(&[0xC0u8, 0, 0]),
		Err(Error::InvalidPrefix { prefix: 0xC0 })
	);
	// usize on 32-bit targets rejects values above u32::MAX.
	let mut wide = [0u8; 9];
	let wide_len = vlen::encode_u64(&mut wide, u64::from(u32::MAX) + 1);
	let decoded = usize::decode(&wide[..wide_len]);
	#[cfg(target_pointer_width = "64")]
	assert_eq!(decoded, Ok((u32::MAX as usize + 1, wide_len)));
	#[cfg(target_pointer_width = "32")]
	assert_eq!(decoded, Err(Error::Overflow));
}

#[test]
fn writer_and_reader_round_trip_mixed_types() {
	let mut buf = [0u8; 64];
	let mut writer = vlen::Writer::new(&mut buf);
	writer.write(7u32).unwrap();
	writer.write(-42i64).unwrap();
	writer.write(1.5f32).unwrap();
	writer.write(usize::MAX).unwrap();
	assert_eq!(writer.remaining(), 64 - writer.position());
	let len = writer.finish();

	let mut reader = vlen::Reader::new(&buf[..len]);
	assert_eq!(reader.read::<u32>().unwrap(), 7);
	assert_eq!(reader.read::<i64>().unwrap(), -42);
	assert_eq!(reader.read::<f32>().unwrap(), 1.5);
	assert_eq!(reader.read::<usize>().unwrap(), usize::MAX);
	assert!(reader.is_empty());
	assert_eq!(reader.position(), len);

	// Reading past the end is a clean error, not a panic.
	assert_eq!(
		reader.read::<u32>(),
		Err(Error::BufferTooSmall {
			needed: 1,
			available: 0
		})
	);
}

#[test]
fn reader_canonical_read_rejects_overlong_input_without_advancing() {
	let bytes = [0x85, 0x00, 7]; // over-long encoding of 5, followed by 7
	let mut reader = vlen::Reader::new(&bytes);

	assert_eq!(
		reader.read_canonical::<u32>(),
		Err(StrictError::NonCanonical {
			encoded_len: 2,
			canonical_len: 1,
		})
	);
	assert_eq!(reader.position(), 0);
	assert_eq!(reader.remaining_bytes(), &bytes);
}

#[test]
fn reader_canonical_read_rejects_same_length_alternate_without_advancing() {
	let bytes = [0xF0, 0x80, 7]; // alternate two-byte encoding of 128
	let mut reader = vlen::Reader::new(&bytes);

	assert_eq!(
		reader.read_canonical::<u32>(),
		Err(StrictError::NonCanonical {
			encoded_len: 2,
			canonical_len: 2,
		})
	);
	assert_eq!(reader.position(), 0);
	assert_eq!(reader.remaining_bytes(), &bytes);
}

#[test]
fn reader_canonical_read_advances_after_success() {
	let mut reader = vlen::Reader::new(&[5, 6]);

	assert_eq!(reader.read_canonical::<u32>(), Ok(5));
	assert_eq!(reader.position(), 1);
	assert_eq!(reader.remaining_bytes(), &[6]);
}

#[test]
fn reader_canonical_decode_error_does_not_advance() {
	let bytes = [0xF9]; // invalid for u32
	let mut reader = vlen::Reader::new(&bytes);

	assert_eq!(
		reader.read_canonical::<u32>(),
		Err(StrictError::Decode(Error::InvalidPrefix { prefix: 0xF9 }))
	);
	assert_eq!(reader.position(), 0);
	assert_eq!(reader.remaining_bytes(), &bytes);
}

#[test]
fn reader_canonical_chain_finishes_mixed_message() {
	let mut bytes = [0u8; 32];
	let mut writer = vlen::Writer::new(&mut bytes);
	writer.write(0x4000u32).unwrap();
	writer.write(-300i64).unwrap();
	writer.write(1.5f32).unwrap();
	let len = writer.finish();

	let mut reader = vlen::Reader::new(&bytes[..len]);
	assert_eq!(reader.read_canonical::<u32>(), Ok(0x4000));
	assert_eq!(reader.read_canonical::<i64>(), Ok(-300));
	assert_eq!(reader.read_canonical::<f32>(), Ok(1.5));
	assert_eq!(reader.finish(), Ok(()));
}

#[test]
fn reader_read_error_does_not_advance() {
	let bytes = [0x80]; // truncated two-byte encoding
	let mut reader = vlen::Reader::new(&bytes);

	assert_eq!(
		reader.read::<u32>(),
		Err(Error::BufferTooSmall {
			needed: 2,
			available: 1,
		})
	);
	assert_eq!(reader.position(), 0);
	assert_eq!(reader.remaining_bytes(), &bytes);
}

#[test]
fn reader_finish_rejects_unread_bytes() {
	let mut reader = vlen::Reader::new(&[5, 6]);
	assert_eq!(reader.read::<u32>(), Ok(5));

	assert_eq!(
		reader.finish(),
		Err(StrictError::TrailingBytes {
			consumed: 1,
			available: 2,
		})
	);
}

#[test]
fn reader_finish_accepts_fully_consumed_input() {
	let mut reader = vlen::Reader::new(&[5]);
	assert_eq!(reader.read::<u32>(), Ok(5));

	assert_eq!(reader.finish(), Ok(()));
}

#[test]
fn writer_reports_out_of_space() {
	let mut buf = [0u8; 3];
	let mut writer = vlen::Writer::new(&mut buf);
	writer.write(1u32).unwrap();
	assert!(writer.write(u32::MAX).is_err());
	// A failed write leaves the position unchanged.
	assert_eq!(writer.position(), 1);
	assert_eq!(writer.finish(), 1);
}

#[cfg(feature = "alloc")]
#[test]
fn encode_append_matches_encode_to_vec() {
	let mut appended = Vec::new();
	vlen::encode_append(&mut appended, 5u32);
	vlen::encode_append(&mut appended, u64::MAX);
	vlen::encode_append(&mut appended, -7i32);

	let mut expected = vlen::encode_to_vec(5u32);
	expected.extend(vlen::encode_to_vec(u64::MAX));
	expected.extend(vlen::encode_to_vec(-7i32));
	assert_eq!(appended, expected);

	let values = [1u32, 0x4000, u32::MAX];
	let mut bulk = Vec::from(&b"header"[..]);
	vlen::bulk_encode_append(&mut bulk, &values);
	assert_eq!(&bulk[..6], b"header");
	assert_eq!(&bulk[6..], &vlen::bulk_encode_to_vec(&values)[..]);
}

#[cfg(feature = "alloc")]
#[derive(Clone, Copy)]
struct EighteenBytes;

#[cfg(feature = "alloc")]
impl Encode for EighteenBytes {
	const MAX_ENCODED_SIZE: usize = 18;

	fn encoded_size(self) -> usize {
		18
	}

	fn encode(self, buf: &mut [u8]) -> vlen::Result<usize> {
		let available = buf.len();
		let dst = buf.get_mut(..18).ok_or(Error::BufferTooSmall {
			needed: 18,
			available,
		})?;
		dst.copy_from_slice(b"eighteen-byte-data");
		Ok(18)
	}
}

#[cfg(feature = "alloc")]
#[test]
fn encode_append_honors_downstream_encoded_sizes() {
	let mut bytes = Vec::from(&b"prefix"[..]);
	vlen::encode_append(&mut bytes, EighteenBytes);
	assert_eq!(&bytes[6..], b"eighteen-byte-data");
}
