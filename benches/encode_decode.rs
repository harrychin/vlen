use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use vlen::{
	decode_u16, decode_u32, decode_u64, decode_u128, encode_u16, encode_u32,
	encode_u64, encode_u128,
};

macro_rules! encode_bench {
	($name:ident, $fn:ident, $buf_size:expr, $value:expr) => {
		fn $name(c: &mut Criterion) {
			let mut buf = [0u8; $buf_size];
			c.bench_function(stringify!($fn), |b| {
				b.iter(|| $fn(black_box(&mut buf), black_box($value)))
			});
		}
	};
}

macro_rules! decode_bench {
	($name:ident, $encode_fn:ident, $decode_fn:ident, $buf_size:expr, $value:expr) => {
		fn $name(c: &mut Criterion) {
			let mut buf = [0u8; $buf_size];
			let _len = $encode_fn(&mut buf, $value);
			c.bench_function(stringify!($decode_fn), |b| {
				b.iter(|| $decode_fn(black_box(&buf)))
			});
		}
	};
}

encode_bench!(bench_encode_u16, encode_u16, 3, 12345u16);
encode_bench!(bench_encode_u32, encode_u32, 5, 12345678u32);
encode_bench!(bench_encode_u64, encode_u64, 9, 0x1234567890ABCDEFu64);
encode_bench!(
	bench_encode_u128,
	encode_u128,
	17,
	0x1234567890ABCDEF1234567890ABCDEFu128
);

decode_bench!(bench_decode_u16, encode_u16, decode_u16, 3, 12345u16);
decode_bench!(bench_decode_u32, encode_u32, decode_u32, 5, 12345678u32);
decode_bench!(
	bench_decode_u64,
	encode_u64,
	decode_u64,
	9,
	0x1234567890ABCDEFu64
);
decode_bench!(
	bench_decode_u128,
	encode_u128,
	decode_u128,
	17,
	0x1234567890ABCDEF1234567890ABCDEFu128
);

criterion_group!(
	benches,
	bench_encode_u16,
	bench_encode_u32,
	bench_encode_u64,
	bench_encode_u128,
	bench_decode_u16,
	bench_decode_u32,
	bench_decode_u64,
	bench_decode_u128,
);
criterion_main!(benches);
