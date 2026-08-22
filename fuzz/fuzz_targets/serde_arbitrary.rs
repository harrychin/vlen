#![no_main]

use libfuzzer_sys::fuzz_target;
use vlen::serde::{VlenF64, VlenI128, VlenU128, VlenUsize};

fuzz_target!(|data: &[u8]| {
	macro_rules! check {
		($t:ty) => {{
			let _ = serde_json::from_slice::<$t>(data);
			let _ = postcard::from_bytes::<$t>(data);
		}};
	}

	check!(VlenU128);
	check!(VlenI128);
	check!(VlenF64);
	check!(VlenUsize);
});
