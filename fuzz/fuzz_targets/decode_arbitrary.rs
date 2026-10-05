#![no_main]

use libfuzzer_sys::fuzz_target;
use vlen::*;

fuzz_target!(|data: &[u8]| {
	let (selector, bytes) = match data.split_first() {
		Some(parts) => parts,
		None => return,
	};
	let count = usize::from(*selector) % 17;

	macro_rules! check_scalar {
		($t:ty) => {
			if let Ok((_, consumed)) = <$t>::decode(bytes) {
				assert!(consumed >= 1);
				assert!(consumed <= bytes.len());
				assert!(consumed <= <$t as Decode>::MAX_ENCODED_SIZE);
			}
		};
	}

	check_scalar!(u8);
	check_scalar!(u16);
	check_scalar!(u32);
	check_scalar!(u64);
	check_scalar!(u128);
	check_scalar!(usize);
	check_scalar!(i8);
	check_scalar!(i16);
	check_scalar!(i32);
	check_scalar!(i64);
	check_scalar!(i128);
	check_scalar!(isize);
	check_scalar!(f32);
	check_scalar!(f64);

	let _ = decode_exact::<u128>(bytes);
	let _ = decode_canonical::<u128>(bytes);
	let _ = decode_strict::<u128>(bytes);

	macro_rules! compare_bulk {
		($t:ty, $specialized:path) => {{
			let mut generic = [0 as $t; 16];
			let mut specialized = [0 as $t; 16];
			let generic_result = bulk_decode(bytes, &mut generic[..count]);
			let specialized_result =
				$specialized(bytes, &mut specialized[..count]);
			assert_eq!(specialized_result, generic_result);
			if generic_result.is_ok() {
				assert_eq!(specialized[..count], generic[..count]);
			}
		}};
	}

	compare_bulk!(u32, bulk_decode_u32);
	compare_bulk!(u64, bulk_decode_u64);
	compare_bulk!(i32, bulk_decode_i32);
	compare_bulk!(i64, bulk_decode_i64);

	assert_eq!(
		decode_iter::<u32>(bytes).collect::<Vec<_>>(),
		decode_iter_u32(bytes).collect::<Vec<_>>()
	);
	assert_eq!(
		decode_iter::<u64>(bytes).collect::<Vec<_>>(),
		decode_iter_u64(bytes).collect::<Vec<_>>()
	);
	assert_eq!(
		decode_iter::<i32>(bytes).collect::<Vec<_>>(),
		decode_iter_i32(bytes).collect::<Vec<_>>()
	);
	assert_eq!(
		decode_iter::<i64>(bytes).collect::<Vec<_>>(),
		decode_iter_i64(bytes).collect::<Vec<_>>()
	);

	macro_rules! compare_vec_helpers {
		($t:ty) => {{
			let collected =
				decode_iter::<$t>(bytes).collect::<Result<Vec<$t>>>();
			assert_eq!(bulk_decode_values::<$t>(bytes), collected);
			if let Ok(values) = collected {
				let mut generic = vec![0u8; values.len() * 9];
				let len = bulk_encode(&mut generic, &values).unwrap();
				assert_eq!(bulk_encode_to_vec(&values), &generic[..len]);
			}
		}};
	}

	compare_vec_helpers!(u32);
	compare_vec_helpers!(u64);
	compare_vec_helpers!(i32);
	compare_vec_helpers!(i64);
});
