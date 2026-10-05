use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use vlen::{
	Encode, Error, bulk_decode_values, bulk_encode_append, encode_append,
};

#[derive(Clone, Copy)]
struct DownstreamBytes {
	bytes: [u8; 18],
	len: u8,
}

impl Encode for DownstreamBytes {
	const MAX_ENCODED_SIZE: usize = 18;

	fn encoded_size(self) -> usize {
		self.len as usize
	}

	fn encode(self, buf: &mut [u8]) -> vlen::Result<usize> {
		let needed = self.encoded_size();
		let available = buf.len();
		let dst = buf
			.get_mut(..needed)
			.ok_or(Error::BufferTooSmall { needed, available })?;
		dst.copy_from_slice(&self.bytes[..needed]);
		Ok(needed)
	}
}

fn bench_encode_append(c: &mut Criterion) {
	let mut small = Vec::with_capacity(17);
	c.bench_function("encode_append/u32_small", |b| {
		b.iter(|| {
			small.clear();
			encode_append(&mut small, black_box(5u32));
			black_box(small.as_slice());
		})
	});

	let mut wide = Vec::with_capacity(17);
	c.bench_function("encode_append/u32_wide", |b| {
		b.iter(|| {
			wide.clear();
			encode_append(&mut wide, black_box(u32::MAX));
			black_box(wide.as_slice());
		})
	});

	let mut widest = Vec::with_capacity(17);
	c.bench_function("encode_append/u128_wide", |b| {
		b.iter(|| {
			widest.clear();
			encode_append(&mut widest, black_box(u128::MAX));
			black_box(widest.as_slice());
		})
	});

	let mut downstream = Vec::with_capacity(18);
	c.bench_function("encode_append/downstream_18", |b| {
		b.iter(|| {
			downstream.clear();
			encode_append(
				&mut downstream,
				black_box(DownstreamBytes {
					bytes: *b"eighteen-byte-data",
					len: 18,
				}),
			);
			black_box(downstream.as_slice());
		})
	});
}

/// Deterministic xorshift, as in the bulk benches.
fn xorshift(i: u32) -> u32 {
	let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0xDEAD_BEEF;
	x ^= x << 13;
	x ^= x >> 17;
	x ^= x << 5;
	x
}

fn bench_bulk_vec(c: &mut Criterion) {
	fn run<T: Encode + vlen::Decode>(
		c: &mut Criterion,
		name: &str,
		values: &[T],
	) {
		let mut out =
			Vec::with_capacity(values.len() * <T as Encode>::MAX_ENCODED_SIZE);
		c.bench_function(&format!("bulk_encode_append/{name}"), |b| {
			b.iter(|| {
				out.clear();
				bulk_encode_append(&mut out, black_box(values));
				black_box(out.as_slice());
			})
		});
		c.bench_function(&format!("bulk_encode_to_vec/{name}"), |b| {
			b.iter(|| vlen::bulk_encode_to_vec(black_box(values)))
		});
		let encoded = vlen::bulk_encode_to_vec(values);
		c.bench_function(&format!("bulk_decode_values/{name}"), |b| {
			b.iter(|| bulk_decode_values::<T>(black_box(&encoded)).unwrap())
		});
	}

	let n = 1024u32;
	let small: Vec<u32> = (0..n).map(|i| i % 0x80).collect();
	let two_byte: Vec<u32> =
		(0..n).map(|i| 0x80 + xorshift(i) % 0x3F80).collect();
	let random: Vec<u32> = (0..n)
		.map(|i| {
			let x = xorshift(i);
			match x % 4 {
				0 => x % 0x80,
				1 => 0x80 + x % 0x3F80,
				2 => 0x4000 + x % 0x1F_C000,
				_ => 0x20_0000 + x % 0xFE0_0000,
			}
		})
		.collect();
	let deltas: Vec<i64> = (0..n as i64)
		.map(|i| (xorshift(i as u32) % 63) as i64 - 31)
		.collect();
	let nanos: Vec<u64> = (0..n as u64)
		.map(|i| 1_700_000_000_000_000_000 + i * 1_000_003)
		.collect();
	run(c, "u32_small", &small);
	run(c, "u32_two_byte", &two_byte);
	run(c, "u32_random", &random);
	run(c, "i64_deltas", &deltas);
	run(c, "u64_nanos", &nanos);
}

criterion_group!(benches, bench_encode_append, bench_bulk_vec);
criterion_main!(benches);
