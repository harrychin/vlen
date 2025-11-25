use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use vlen::{bulk_decode, bulk_encode, decode_u32, encode_u32};

fn bench_single_encode_u32(c: &mut Criterion) {
	let mut buf = [0u8; 5];
	c.bench_function("single_encode_u32", |b| {
		b.iter(|| {
			let v = black_box(12345678u32);
			encode_u32(&mut buf, v)
		})
	});
}

fn bench_single_decode_u32(c: &mut Criterion) {
	let mut buf = [0u8; 5];
	let _len = encode_u32(&mut buf, 12345678u32);
	c.bench_function("single_decode_u32", |b| b.iter(|| decode_u32(&buf)));
}

fn bench_bulk_encode_u32_small(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 256];
	let values: Vec<u32> = (0..256).collect();

	c.bench_function("bulk_encode_u32_small", |b| {
		b.iter(|| bulk_encode(&mut buf, &values))
	});
}

fn bench_bulk_decode_u32_small(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 256];
	let values: Vec<u32> = (0..256).collect();

	let encoded_len = bulk_encode(&mut buf, &values).unwrap();
	let mut decoded_values = [0u32; 256];

	c.bench_function("bulk_decode_u32_small", |b| {
		b.iter(|| bulk_decode(&buf[..encoded_len], &mut decoded_values))
	});
}

fn bench_bulk_encode_u32_medium(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 1024];
	let values: Vec<u32> = (0..1024).collect();

	c.bench_function("bulk_encode_u32_medium", |b| {
		b.iter(|| bulk_encode(&mut buf, &values))
	});
}

fn bench_bulk_decode_u32_medium(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 1024];
	let values: Vec<u32> = (0..1024).collect();

	let encoded_len = bulk_encode(&mut buf, &values).unwrap();
	let mut decoded_values = [0u32; 1024];

	c.bench_function("bulk_decode_u32_medium", |b| {
		b.iter(|| bulk_decode(&buf[..encoded_len], &mut decoded_values))
	});
}

fn bench_bulk_encode_u32_large(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 4096];
	let values: Vec<u32> = (0..4096).collect();

	c.bench_function("bulk_encode_u32_large", |b| {
		b.iter(|| bulk_encode(&mut buf, &values))
	});
}

fn bench_bulk_decode_u32_large(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 4096];
	let values: Vec<u32> = (0..4096).collect();

	let encoded_len = bulk_encode(&mut buf, &values).unwrap();
	let mut decoded_values = [0u32; 4096];

	c.bench_function("bulk_decode_u32_large", |b| {
		b.iter(|| bulk_decode(&buf[..encoded_len], &mut decoded_values))
	});
}

fn bench_bulk_encode_u32_mixed(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 1024];
	let values: Vec<u32> = (0..1024)
		.map(|i| match i % 4 {
			0 => i as u32,
			1 => 1000 + i as u32,
			2 => 1000000 + i as u32,
			_ => 1000000000 + i as u32,
		})
		.collect();

	c.bench_function("bulk_encode_u32_mixed", |b| {
		b.iter(|| bulk_encode(&mut buf, &values))
	});
}

fn bench_bulk_decode_u32_mixed(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 1024];
	let values: Vec<u32> = (0..1024)
		.map(|i| match i % 4 {
			0 => i as u32,
			1 => 1000 + i as u32,
			2 => 1000000 + i as u32,
			_ => 1000000000 + i as u32,
		})
		.collect();

	let encoded_len = bulk_encode(&mut buf, &values).unwrap();
	let mut decoded_values = [0u32; 1024];

	c.bench_function("bulk_decode_u32_mixed", |b| {
		b.iter(|| bulk_decode(&buf[..encoded_len], &mut decoded_values))
	});
}

fn bench_bulk_encode_u32_small_values(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 1024];
	let values: Vec<u32> = (0..1024).map(|i| (i % 128) as u32).collect();

	c.bench_function("bulk_encode_u32_small_values", |b| {
		b.iter(|| bulk_encode(&mut buf, &values))
	});
}

fn bench_bulk_decode_u32_small_values(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 1024];
	let values: Vec<u32> = (0..1024).map(|i| (i % 128) as u32).collect();

	let encoded_len = bulk_encode(&mut buf, &values).unwrap();
	let mut decoded_values = [0u32; 1024];

	c.bench_function("bulk_decode_u32_small_values", |b| {
		b.iter(|| bulk_decode(&buf[..encoded_len], &mut decoded_values))
	});
}

fn bench_bulk_encode_u32_large_values(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 1024];
	let values: Vec<u32> = (0..1024).map(|i| 1000000000 + i as u32).collect();

	c.bench_function("bulk_encode_u32_large_values", |b| {
		b.iter(|| bulk_encode(&mut buf, &values))
	});
}

fn bench_bulk_decode_u32_large_values(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 1024];
	let values: Vec<u32> = (0..1024).map(|i| 1000000000 + i as u32).collect();

	let encoded_len = bulk_encode(&mut buf, &values).unwrap();
	let mut decoded_values = [0u32; 1024];

	c.bench_function("bulk_decode_u32_large_values", |b| {
		b.iter(|| bulk_decode(&buf[..encoded_len], &mut decoded_values))
	});
}

criterion_group!(
	benches,
	bench_single_encode_u32,
	bench_single_decode_u32,
	bench_bulk_encode_u32_small,
	bench_bulk_decode_u32_small,
	bench_bulk_encode_u32_medium,
	bench_bulk_decode_u32_medium,
	bench_bulk_encode_u32_large,
	bench_bulk_decode_u32_large,
	bench_bulk_encode_u32_mixed,
	bench_bulk_decode_u32_mixed,
	bench_bulk_encode_u32_small_values,
	bench_bulk_decode_u32_small_values,
	bench_bulk_encode_u32_large_values,
	bench_bulk_decode_u32_large_values
);
criterion_main!(benches);
