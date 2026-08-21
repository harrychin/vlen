//! Benchmarks for the bulk encode/decode paths.
//!
//! Distributions: `small` exercises the SWAR one-byte fast path,
//! `mixed` cycles through all encoded sizes in a periodic pattern the
//! branch predictor can learn, `random` draws sizes unpredictably (the
//! worst case for branchy decoders), and `large` defeats the fast path
//! entirely.

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use vlen::{bulk_decode, bulk_decode_u32, bulk_encode, bulk_encode_u32};

const N: usize = 1024;

fn values(kind: &str) -> Vec<u32> {
	(0..N as u32)
		.map(|i| match kind {
			"small" => i % 0x80,
			"mixed" => match i % 4 {
				0 => i,
				1 => 1000 + i,
				2 => 1_000_000 + i,
				_ => 1_000_000_000 + i,
			},
			"random" => {
				// Deterministic xorshift so the size sequence is
				// unpredictable to the branch predictor but stable
				// across runs.
				let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0xDEAD_BEEF;
				x ^= x << 13;
				x ^= x >> 17;
				x ^= x << 5;
				match x % 4 {
					0 => x % 0x80,
					1 => 0x80 + x % 0x3F80,
					2 => 0x4000 + x % 0x1F_C000,
					_ => 0x20_0000 + x % 0xFE0_0000,
				}
			},
			_ => 0x1000_0000 + i,
		})
		.collect()
}

fn bench_bulk(c: &mut Criterion) {
	for kind in ["small", "mixed", "random", "large"] {
		let values = values(kind);
		let mut buf = vec![0u8; N * 5];

		c.bench_function(&format!("bulk_encode_u32/{kind}"), |b| {
			b.iter(|| {
				bulk_encode_u32(black_box(&mut buf), black_box(&values))
					.unwrap()
			})
		});
		c.bench_function(&format!("bulk_encode_generic/{kind}"), |b| {
			b.iter(|| {
				bulk_encode(black_box(&mut buf), black_box(&values)).unwrap()
			})
		});

		let encoded_len = bulk_encode(&mut buf, &values).unwrap();
		let encoded = &buf[..encoded_len];
		let mut decoded = vec![0u32; N];

		c.bench_function(&format!("bulk_decode_u32/{kind}"), |b| {
			b.iter(|| {
				bulk_decode_u32(black_box(encoded), black_box(&mut decoded))
					.unwrap()
			})
		});
		c.bench_function(&format!("bulk_decode_generic/{kind}"), |b| {
			b.iter(|| {
				bulk_decode(black_box(encoded), black_box(&mut decoded))
					.unwrap()
			})
		});
	}
}

criterion_group!(benches, bench_bulk);
criterion_main!(benches);
