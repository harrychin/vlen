use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use vlen::{
	bulk_decode_u32,
	bulk_encode_u32,
	decode_u128,
	decode_u16,
	decode_u32,
	decode_u64,
	encode_u128,
	encode_u16,
	encode_u32,
	encode_u64,
};

fn bench_encode_u16(c: &mut Criterion) {
	let mut buf = [0u8; 3];
	c.bench_function("encode_u16", |b| {
		b.iter(|| {
			let v = black_box(12345u16);
			encode_u16(&mut buf, v)
		})
	});
}

fn bench_encode_u32(c: &mut Criterion) {
	let mut buf = [0u8; 5];
	c.bench_function("encode_u32", |b| {
		b.iter(|| {
			let v = black_box(12345678u32);
			encode_u32(&mut buf, v)
		})
	});
}

fn bench_encode_u64(c: &mut Criterion) {
	let mut buf = [0u8; 9];
	c.bench_function("encode_u64", |b| {
		b.iter(|| {
			let v = black_box(0x1234567890ABCDEFu64);
			encode_u64(&mut buf, v)
		})
	});
}

fn bench_encode_u128(c: &mut Criterion) {
	let mut buf = [0u8; 17];
	c.bench_function("encode_u128", |b| {
		b.iter(|| {
			let v = black_box(0x1234567890ABCDEF1234567890ABCDEFu128);
			encode_u128(&mut buf, v)
		})
	});
}

fn bench_decode_u16(c: &mut Criterion) {
	let mut buf = [0u8; 3];
	let _len = encode_u16(&mut buf, 12345u16);
	c.bench_function("decode_u16", |b| b.iter(|| decode_u16(&buf)));
}

fn bench_decode_u32(c: &mut Criterion) {
	let mut buf = [0u8; 5];
	let _len = encode_u32(&mut buf, 12345678u32);
	c.bench_function("decode_u32", |b| b.iter(|| decode_u32(&buf)));
}

fn bench_decode_u64(c: &mut Criterion) {
	let mut buf = [0u8; 9];
	let _len = encode_u64(&mut buf, 0x1234567890ABCDEFu64);
	c.bench_function("decode_u64", |b| b.iter(|| decode_u64(&buf)));
}

fn bench_decode_u128(c: &mut Criterion) {
	let mut buf = [0u8; 17];
	let _len = encode_u128(&mut buf, 0x1234567890ABCDEF1234567890ABCDEFu128);
	c.bench_function("decode_u128", |b| b.iter(|| decode_u128(&buf)));
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn bench_bulk_encode_u32(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 1024];
	let values: Vec<u32> = (0..1024)
		.map(|i| match i % 4 {
			0 => i as u32,
			1 => 1000 + i as u32,
			2 => 1000000 + i as u32,
			_ => 1000000000 + i as u32,
		})
		.collect();

	c.bench_function("bulk_encode_u32", |b| {
		b.iter(|| unsafe { bulk_encode_u32(&mut buf, &values) })
	});
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn bench_bulk_decode_u32(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 1024];
	let values: Vec<u32> = (0..1024)
		.map(|i| match i % 4 {
			0 => i as u32,
			1 => 1000 + i as u32,
			2 => 1000000 + i as u32,
			_ => 1000000000 + i as u32,
		})
		.collect();

	let encoded_len = unsafe { bulk_encode_u32(&mut buf, &values) };
	let mut decoded_values = [0u32; 1024];

	c.bench_function("bulk_decode_u32", |b| {
		b.iter(|| unsafe {
			bulk_decode_u32(&buf[..encoded_len], &mut decoded_values)
		})
	});
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn bench_bulk_encode_u32(_c: &mut Criterion) {}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn bench_bulk_decode_u32(_c: &mut Criterion) {}

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
	bench_bulk_encode_u32,
	bench_bulk_decode_u32
);
criterion_main!(benches);
