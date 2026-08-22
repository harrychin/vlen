use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use vlen::{
	decode, decode_strict, decode_u8, decode_u16, decode_u32, decode_u64,
	decode_u128, encode_u16, encode_u32, encode_u64, encode_u128,
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

fn bench_decode_u32_checked(c: &mut Criterion) {
	let mut buf = [0u8; 5];
	let len = encode_u32(&mut buf, 12345678);
	c.bench_function("decode_u32_checked", |b| {
		b.iter(|| decode::<u32>(black_box(&buf[..len])).unwrap())
	});
}

fn bench_decode_u32_strict(c: &mut Criterion) {
	let mut buf = [0u8; 5];
	let len = encode_u32(&mut buf, 12345678);
	c.bench_function("decode_u32_strict", |b| {
		b.iter(|| decode_strict::<u32>(black_box(&buf[..len])).unwrap())
	});
}

fn bench_narrow_decoders(c: &mut Criterion) {
	let u8_two = [0xBF, 0x03];
	c.bench_function("decode_u8/two_byte", |b| {
		b.iter(|| decode_u8(black_box(&u8_two)))
	});
	c.bench_function("decode_u8_checked/two_byte", |b| {
		b.iter(|| decode::<u8>(black_box(&u8_two)).unwrap())
	});

	let u16_three = [0xDF, 0xFF, 0x07];
	c.bench_function("decode_u16/three_byte", |b| {
		b.iter(|| decode_u16(black_box(&u16_three)))
	});
	c.bench_function("decode_u16_checked/three_byte", |b| {
		b.iter(|| decode::<u16>(black_box(&u16_three)).unwrap())
	});
}

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
	bench_decode_u32_checked,
	bench_decode_u32_strict,
	bench_narrow_decoders,
);
criterion_main!(benches);
