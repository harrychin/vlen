//! Comparison against LEB128 (varint) encoding.
//!
//! Both sides use their in-memory slice APIs so the comparison measures
//! the codecs, not I/O machinery.

use criterion::{Criterion, criterion_group, criterion_main};
use integer_encoding::VarInt;
use std::hint::black_box;
use vlen::{bulk_decode, bulk_encode, decode_u32, encode_u32};

fn bench_single_encode(c: &mut Criterion) {
	let mut vlen_buf = [0u8; 5];
	c.bench_function("single_encode/vlen", |b| {
		b.iter(|| encode_u32(black_box(&mut vlen_buf), black_box(12345678u32)))
	});

	let mut leb_buf = [0u8; 5];
	c.bench_function("single_encode/leb128", |b| {
		b.iter(|| black_box(12345678u32).encode_var(black_box(&mut leb_buf)))
	});
}

fn bench_single_decode(c: &mut Criterion) {
	let mut vlen_buf = [0u8; 5];
	let _ = encode_u32(&mut vlen_buf, 12345678u32);
	c.bench_function("single_decode/vlen", |b| {
		b.iter(|| decode_u32(black_box(&vlen_buf)))
	});

	let mut leb_buf = [0u8; 5];
	let _ = 12345678u32.encode_var(&mut leb_buf);
	c.bench_function("single_decode/leb128", |b| {
		b.iter(|| u32::decode_var(black_box(&leb_buf)).unwrap())
	});
}

fn mixed_values() -> Vec<u32> {
	(0..1024u32)
		.map(|i| match i % 4 {
			0 => i,
			1 => 1000 + i,
			2 => 1_000_000 + i,
			_ => 1_000_000_000 + i,
		})
		.collect()
}

fn bench_bulk_encode(c: &mut Criterion) {
	let values = mixed_values();
	let mut buf = vec![0u8; values.len() * 5];

	c.bench_function("bulk_encode/vlen", |b| {
		b.iter(|| bulk_encode(black_box(&mut buf), black_box(&values)).unwrap())
	});

	c.bench_function("bulk_encode/leb128", |b| {
		b.iter(|| {
			let mut offset = 0;
			for &value in black_box(&values) {
				offset += value.encode_var(&mut buf[offset..]);
			}
			offset
		})
	});
}

fn bench_bulk_decode(c: &mut Criterion) {
	let values = mixed_values();

	let mut vlen_buf = vec![0u8; values.len() * 5];
	let vlen_len = bulk_encode(&mut vlen_buf, &values).unwrap();
	let vlen_encoded = &vlen_buf[..vlen_len];
	let mut decoded = vec![0u32; values.len()];

	c.bench_function("bulk_decode/vlen", |b| {
		b.iter(|| {
			bulk_decode(black_box(vlen_encoded), black_box(&mut decoded))
				.unwrap()
		})
	});

	let mut leb_buf = vec![0u8; values.len() * 5];
	let mut leb_len = 0;
	for &value in &values {
		leb_len += value.encode_var(&mut leb_buf[leb_len..]);
	}
	let leb_encoded = &leb_buf[..leb_len];

	c.bench_function("bulk_decode/leb128", |b| {
		b.iter(|| {
			let mut offset = 0;
			for slot in decoded.iter_mut() {
				let (value, len) =
					u32::decode_var(black_box(&leb_encoded[offset..])).unwrap();
				*slot = value;
				offset += len;
			}
			offset
		})
	});
}

criterion_group!(
	benches,
	bench_single_encode,
	bench_single_decode,
	bench_bulk_encode,
	bench_bulk_decode,
);
criterion_main!(benches);
