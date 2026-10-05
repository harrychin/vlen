//! Benchmarks for the bulk encode/decode paths.
//!
//! Distributions: `small` exercises the SWAR one-byte fast path,
//! `mixed` cycles through all encoded sizes in a periodic pattern the
//! branch predictor can learn, `random` draws sizes unpredictably (the
//! worst case for branchy decoders), and `large` defeats the fast path
//! entirely. The `*_uniform` distributions fix one encoded size but
//! draw values uniformly across that size class, so neighboring values
//! share no high bits.

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use vlen::{
	bulk_decode, bulk_decode_i64, bulk_decode_u32, bulk_decode_u64,
	bulk_encode, bulk_encode_i64, bulk_encode_u32, bulk_encode_u64,
};

const N: usize = 1024;

/// Deterministic xorshift so sequences are unpredictable to the branch
/// predictor but stable across runs.
fn xorshift(i: u32) -> u32 {
	let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0xDEAD_BEEF;
	x ^= x << 13;
	x ^= x >> 17;
	x ^= x << 5;
	x
}

fn values(kind: &str) -> Vec<u32> {
	(0..N as u32)
		.map(|i| match kind {
			"small" => i % 0x80,
			"two_byte" => 0x80 + (i * 37) % 0x3F80,
			"three_byte" => 0x4000 + (i * 97) % 0x1F_C000,
			"four_byte" => 0x20_0000 + (i * 997) % 0xFE0_0000,
			"two_byte_uniform" => 0x80 + xorshift(i) % 0x3F80,
			"three_byte_uniform" => 0x4000 + xorshift(i) % 0x1F_C000,
			"four_byte_uniform" => 0x20_0000 + xorshift(i) % 0xFE0_0000,
			"five_byte_uniform" => 0x1000_0000 + xorshift(i) % 0xF000_0000,
			"mixed" => match i % 4 {
				0 => i,
				1 => 1000 + i,
				2 => 1_000_000 + i,
				_ => 1_000_000_000 + i,
			},
			"random" => {
				let x = xorshift(i);
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

fn bench_bulk_i64_deltas(c: &mut Criterion) {
	// Two delta-stream shapes: `smooth` has rare spikes (a steady
	// signal sampled regularly, where runs of small deltas form), and
	// `choppy` interleaves magnitudes per value (hostile to run
	// detection; the documented case for the generic functions).
	for (kind, spike_every) in [("smooth", 64), ("choppy", 4)] {
		let values: Vec<i64> = (0..N as i64)
			.map(|i| {
				let mut x = (i as u32).wrapping_mul(0x9E37_79B9) ^ 0xBEEF;
				x ^= x << 13;
				x ^= x >> 17;
				x ^= x << 5;
				if x % spike_every == 0 {
					(x % 0x40_0000) as i64 - 0x20_0000
				} else {
					(x % 63) as i64 - 31
				}
			})
			.collect();
		let mut buf = vec![0u8; N * 9];

		c.bench_function(&format!("bulk_encode_i64/{kind}"), |b| {
			b.iter(|| {
				bulk_encode_i64(black_box(&mut buf), black_box(&values))
					.unwrap()
			})
		});
		c.bench_function(&format!("bulk_encode_generic_i64/{kind}"), |b| {
			b.iter(|| {
				bulk_encode(black_box(&mut buf), black_box(&values)).unwrap()
			})
		});

		let encoded_len = bulk_encode(&mut buf, &values).unwrap();
		let encoded = &buf[..encoded_len];
		let mut decoded = vec![0i64; N];

		c.bench_function(&format!("bulk_decode_i64/{kind}"), |b| {
			b.iter(|| {
				bulk_decode_i64(black_box(encoded), black_box(&mut decoded))
					.unwrap()
			})
		});
		c.bench_function(&format!("bulk_decode_generic_i64/{kind}"), |b| {
			b.iter(|| {
				bulk_decode(black_box(encoded), black_box(&mut decoded))
					.unwrap()
			})
		});
	}
}

/// Values past `u32::MAX`, in the binary length-prefix classes: uniform
/// within one class, clustered nanosecond timestamps, and sizes drawn
/// unpredictably from all four wide classes.
fn values_u64(kind: &str) -> Vec<u64> {
	(0..N as u32)
		.map(|i| {
			let x = (xorshift(2 * i) as u64) << 32 | xorshift(2 * i + 1) as u64;
			match kind {
				"six_byte_uniform" => (1 << 32) + x % ((1 << 40) - (1 << 32)),
				"eight_byte_uniform" => (1 << 48) + x % ((1 << 56) - (1 << 48)),
				"nine_byte_uniform" => {
					(1 << 56) + x % (1u64 << 56).wrapping_neg()
				},
				"nanos" => 1_700_000_000_000_000_000 + i as u64 * 1_000_003,
				"wide_random" => {
					let class = 4 + (x % 4) as u32;
					(1 << (8 * class)) | (x >> (64 - 8 * class))
				},
				_ => unreachable!("unknown u64 distribution {kind}"),
			}
		})
		.collect()
}

fn bench_bulk_u64(c: &mut Criterion) {
	let narrow = ["small", "two_byte", "two_byte_uniform", "random"]
		.map(|kind| (kind, values(kind).into_iter().map(u64::from).collect()));
	let wide = [
		"six_byte_uniform",
		"eight_byte_uniform",
		"nine_byte_uniform",
		"nanos",
		"wide_random",
	]
	.map(|kind| (kind, values_u64(kind)));
	for (kind, values) in narrow.into_iter().chain(wide) {
		let values: Vec<u64> = values;
		let mut buf = vec![0u8; N * 9];

		c.bench_function(&format!("bulk_encode_u64/{kind}"), |b| {
			b.iter(|| {
				bulk_encode_u64(black_box(&mut buf), black_box(&values))
					.unwrap()
			})
		});
		c.bench_function(&format!("bulk_encode_generic_u64/{kind}"), |b| {
			b.iter(|| {
				bulk_encode(black_box(&mut buf), black_box(&values)).unwrap()
			})
		});

		let encoded_len = bulk_encode(&mut buf, &values).unwrap();
		let encoded = &buf[..encoded_len];
		let mut decoded = vec![0u64; N];

		c.bench_function(&format!("bulk_decode_u64/{kind}"), |b| {
			b.iter(|| {
				bulk_decode_u64(black_box(encoded), black_box(&mut decoded))
					.unwrap()
			})
		});
	}
}

fn bench_bulk(c: &mut Criterion) {
	for kind in [
		"small",
		"two_byte",
		"three_byte",
		"four_byte",
		"two_byte_uniform",
		"three_byte_uniform",
		"four_byte_uniform",
		"five_byte_uniform",
		"mixed",
		"random",
		"large",
	] {
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

criterion_group!(
	benches,
	bench_bulk,
	bench_bulk_u64,
	bench_bulk_i64_deltas,
	bench_iter
);
criterion_main!(benches);

fn bench_iter(c: &mut Criterion) {
	for kind in ["small", "two_byte", "random"] {
		let values = values(kind);
		let mut buf = vec![0u8; N * 5];
		let len = bulk_encode(&mut buf, &values).unwrap();
		let encoded = &buf[..len];
		let mut out = vec![0u32; N];

		c.bench_function(&format!("iter_generic/{kind}"), |b| {
			b.iter(|| {
				vlen::decode_iter::<u32>(black_box(encoded))
					.map(Result::unwrap)
					.fold(0u64, |acc, v| acc.wrapping_add(v as u64))
			})
		});
		c.bench_function(&format!("iter_run/{kind}"), |b| {
			b.iter(|| {
				vlen::decode_iter_u32(black_box(encoded))
					.map(Result::unwrap)
					.fold(0u64, |acc, v| acc.wrapping_add(v as u64))
			})
		});
		c.bench_function(&format!("iter_bulk_slice/{kind}"), |b| {
			b.iter(|| {
				bulk_decode_u32(black_box(encoded), black_box(&mut out))
					.unwrap();
				out.iter().fold(0u64, |acc, &v| acc.wrapping_add(v as u64))
			})
		});
	}
}
