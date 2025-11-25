use criterion::{criterion_group, criterion_main, Criterion};
use integer_encoding::{VarIntReader, VarIntWriter};
use std::hint::black_box;
use std::io::Cursor;
use vlen::{bulk_decode, bulk_encode, decode_u32, encode_u32};

fn bench_vlen_encode(c: &mut Criterion) {
	let mut buf = [0u8; 5];
	c.bench_function("vlen_encode", |b| {
		b.iter(|| {
			let v = black_box(12345678u32);
			encode_u32(&mut buf, v)
		})
	});
}

fn bench_vlen_decode(c: &mut Criterion) {
	let mut buf = [0u8; 5];
	let _len = encode_u32(&mut buf, 12345678u32);
	c.bench_function("vlen_decode", |b| b.iter(|| decode_u32(&buf)));
}

fn bench_leb128_encode(c: &mut Criterion) {
	let mut buf = [0u8; 5];
	c.bench_function("leb128_encode", |b| {
		b.iter(|| {
			let v = black_box(12345678u32);
			let mut cursor = Cursor::new(&mut buf[..]);
			cursor.write_varint(v).unwrap();
			cursor.position() as usize
		})
	});
}

fn bench_leb128_decode(c: &mut Criterion) {
	let mut buf = [0u8; 5];
	let mut cursor = Cursor::new(&mut buf[..]);
	cursor.write_varint(12345678u32).unwrap();
	let len = cursor.position() as usize;
	c.bench_function("leb128_decode", |b| {
		b.iter(|| {
			let mut cursor = Cursor::new(&buf[..len]);
			cursor.read_varint::<u32>().unwrap()
		})
	});
}

fn bench_vlen_bulk_encode(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 1024];
	let values: Vec<u32> = (0..1024)
		.map(|i| match i % 4 {
			0 => i as u32,
			1 => 1000 + i as u32,
			2 => 1000000 + i as u32,
			_ => 1000000000 + i as u32,
		})
		.collect();

	c.bench_function("vlen_bulk_encode", |b| {
		b.iter(|| bulk_encode(&mut buf, &values))
	});
}

fn bench_vlen_bulk_decode(c: &mut Criterion) {
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

	c.bench_function("vlen_bulk_decode", |b| {
		b.iter(|| bulk_decode(&buf[..encoded_len], &mut decoded_values))
	});
}

fn bench_leb128_bulk_encode(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 1024];
	let values: Vec<u32> = (0..1024)
		.map(|i| match i % 4 {
			0 => i as u32,
			1 => 1000 + i as u32,
			2 => 1000000 + i as u32,
			_ => 1000000000 + i as u32,
		})
		.collect();

	c.bench_function("leb128_bulk_encode", |b| {
		b.iter(|| {
			let mut cursor = Cursor::new(&mut buf[..]);
			for &value in &values {
				cursor.write_varint(value).unwrap();
			}
			cursor.position() as usize
		})
	});
}

fn bench_leb128_bulk_decode(c: &mut Criterion) {
	let mut buf = [0u8; 5 * 1024];
	let values: Vec<u32> = (0..1024)
		.map(|i| match i % 4 {
			0 => i as u32,
			1 => 1000 + i as u32,
			2 => 1000000 + i as u32,
			_ => 1000000000 + i as u32,
		})
		.collect();

	let mut cursor = Cursor::new(&mut buf[..]);
	for &value in &values {
		cursor.write_varint(value).unwrap();
	}
	let encoded_len = cursor.position() as usize;
	let mut decoded_values = [0u32; 1024];

	c.bench_function("leb128_bulk_decode", |b| {
		b.iter(|| {
			let mut cursor = Cursor::new(&buf[..encoded_len]);
			for i in 0..1024 {
				decoded_values[i] = cursor.read_varint::<u32>().unwrap();
			}
		})
	});
}

criterion_group!(
	benches,
	bench_vlen_encode,
	bench_vlen_decode,
	bench_leb128_encode,
	bench_leb128_decode,
	bench_vlen_bulk_encode,
	bench_vlen_bulk_decode,
	bench_leb128_bulk_encode,
	bench_leb128_bulk_decode
);
criterion_main!(benches);
