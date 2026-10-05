#![no_main]

use libfuzzer_sys::fuzz_target;
use vlen::*;

/// Values in every size class: each 17-byte chunk is a little-endian
/// word shifted right by its first byte, so short and long encodings
/// both appear, in runs or interleaved as the input dictates.
fn values(data: &[u8]) -> Vec<u128> {
	data.chunks(17)
		.map(|chunk| {
			let (&shift, rest) = chunk.split_first().unwrap_or((&0, &[]));
			let mut word = [0u8; 16];
			word[..rest.len()].copy_from_slice(rest);
			u128::from_le_bytes(word) >> (shift % 128)
		})
		.collect()
}

/// The canonical encoding of `value`.
fn canonical<T: Encode>(value: T) -> Vec<u8> {
	let mut buf = [0u8; 17];
	let len = value.encode(&mut buf).unwrap();
	buf[..len].to_vec()
}

/// `encode_padded` at width `N` must decode back to `value` (compared
/// through canonical bytes, so floats compare by bits), consume exactly
/// `N` bytes, equal the canonical bytes at the canonical width, and fail
/// untouched when the slot is too narrow.
fn check_padded<T: Encode + Decode, const N: usize>(value: T) {
	let expected = canonical(value);
	let mut slot = [0x5Au8; N];
	match encode_padded(&mut slot, value) {
		Ok(()) => {
			assert!(expected.len() <= N);
			let (decoded, len) = T::decode(&slot).unwrap();
			assert_eq!(len, N);
			assert_eq!(canonical(decoded), expected);
			assert_eq!(
				decode_canonical::<T>(&slot).is_ok(),
				N == expected.len()
			);
			if N == expected.len() {
				assert_eq!(&slot[..], &expected[..]);
			}
		},
		Err(err) => {
			assert!(expected.len() > N);
			assert_eq!(
				err,
				Error::BufferTooSmall {
					needed: expected.len(),
					available: N,
				}
			);
			assert_eq!(slot, [0x5A; N]);
		},
	}
}

macro_rules! check_widths {
	($value:expr; $($n:literal)*) => {{
		$(check_padded::<_, $n>($value);)*
	}};
}

/// Specialized bulk encoders and the `Vec` helpers against the generic
/// encoder, then a round trip through the specialized decoder.
macro_rules! check_bulk {
	($t:ty, $values:expr, $encode:path, $decode:path) => {{
		let values: Vec<$t> = $values;
		let size = <$t as Encode>::MAX_ENCODED_SIZE;
		let mut generic = vec![0u8; values.len() * size];
		let len = bulk_encode(&mut generic, &values).unwrap();
		let generic = &generic[..len];

		// Exactly sized and roomy buffers exercise both the run paths and
		// their scratch-room fallbacks.
		for room in [len, values.len() * size] {
			let mut specialized = vec![0u8; room];
			assert_eq!($encode(&mut specialized, &values), Ok(len));
			assert_eq!(&specialized[..len], generic);
		}
		assert_eq!(bulk_encode_to_vec(&values), generic);
		let mut appended = vec![0xEE];
		bulk_encode_append(&mut appended, &values);
		assert_eq!(&appended[1..], generic);

		let mut decoded = vec![0 as $t; values.len()];
		assert_eq!($decode(generic, &mut decoded), Ok(len));
		assert_eq!(decoded, values);
		assert_eq!(bulk_decode_values::<$t>(generic), Ok(values));
	}};
}

/// Drives a `Writer` through writes, reservations, and fills chosen by
/// the input, then reads the message back against a model.
fn check_writer(data: &[u8], wide: &[u128]) {
	let mut buf = vec![0u8; usize::from(data.first().copied().unwrap_or(0))];
	let mut writer = Writer::new(&mut buf);
	let mut model: Vec<(u32, usize)> = Vec::new();
	let mut slots = Vec::new();
	for (&op, &value) in data.iter().zip(wide) {
		let value = value as u32;
		let before = writer.position();
		match op % 3 {
			0 => match writer.write(value) {
				Ok(()) => model.push((value, writer.position() - before)),
				Err(_) => assert_eq!(writer.position(), before),
			},
			1 => match writer.reserve::<5>() {
				Ok(slot) => {
					slots.push((slot, model.len()));
					model.push((0, 5));
				},
				Err(_) => assert_eq!(writer.position(), before),
			},
			_ => {
				if let Some((slot, index)) = slots.pop() {
					writer.fill(slot, value).unwrap();
					model[index].0 = value;
				}
			},
		}
	}
	// A zeroed slot reads as five zero values; fill the rest.
	for (slot, index) in slots {
		writer.fill(slot, index as u32).unwrap();
		model[index].0 = index as u32;
	}
	let len = writer.finish();

	let mut reader = Reader::new(&buf[..len]);
	for (value, width) in model {
		let before = reader.position();
		assert_eq!(reader.read::<u32>(), Ok(value));
		assert_eq!(reader.position() - before, width);
	}
	assert_eq!(reader.finish(), Ok(()));
}

fuzz_target!(|data: &[u8]| {
	let wide = values(data);

	for &v in &wide {
		check_widths!(v; 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17);
		check_widths!(v as i128; 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17);
		check_widths!(v as u64; 1 2 3 4 5 6 7 8 9);
		check_widths!(v as i64; 1 2 3 4 5 6 7 8 9);
		check_widths!(v as u32; 1 2 3 4 5);
		check_widths!(v as i32; 1 2 3 4 5);
		check_widths!(v as u16; 1 2 3);
		check_widths!(v as i16; 1 2 3);
		check_widths!(v as u8; 1 2);
		check_widths!(v as i8; 1 2);
		check_widths!(f64::from_bits(v as u64); 1 2 3 4 5 6 7 8 9);
		check_widths!(f32::from_bits(v as u32); 1 2 3 4 5);
	}

	check_bulk!(
		u32,
		wide.iter().map(|&v| v as u32).collect(),
		bulk_encode_u32,
		bulk_decode_u32
	);
	check_bulk!(
		u64,
		wide.iter().map(|&v| v as u64).collect(),
		bulk_encode_u64,
		bulk_decode_u64
	);
	check_bulk!(
		i32,
		wide.iter().map(|&v| v as i32).collect(),
		bulk_encode_i32,
		bulk_decode_i32
	);
	check_bulk!(
		i64,
		wide.iter().map(|&v| v as i64).collect(),
		bulk_encode_i64,
		bulk_decode_i64
	);

	check_writer(data, &wide);
});
