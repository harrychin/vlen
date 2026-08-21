#![cfg(not(target_arch = "wasm32"))]
//! Comparison against other integer encodings.
//!
//! Methodology: every codec is driven through its fastest public
//! in-memory API, over identical values and reused buffers, in the
//! same process, so the comparison measures the codecs rather than
//! I/O machinery or allocation:
//!
//! - `leb128`: LEB128 via the `integer-encoding` crate
//! - `prost`: the protobuf varint (LEB128) implementation from `prost`
//! - `vint64`: the other prefix-varint crate (length in the first byte)
//! - `stream_vbyte`: group varint, scalar kernels (its x86_64 SSE4.1
//!   decoder is faster than what this benchmark shows on that arch)
//! - `fixed`: raw little-endian `u32` words, as an upper throughput
//!   bound that spends four bytes per value
//!
//! Because vlen's array API is infallible while the other crates'
//! entry points validate their input, the single-value benchmarks
//! also include `vlen_checked` — the slice-based `Encode`/`Decode`
//! path that performs the same validation work the other crates do.
//!
//! `prost` and `vint64` are 64-bit codecs; they are fed the same
//! values widened to `u64`. The bulk rows use each crate's natural
//! bulk usage: vlen's dedicated bulk functions, stream-vbyte's batch
//! API, and per-value loops for the codecs that ship no bulk API.
//!
//! Distributions: `small` is uniform one-byte values, `mixed` cycles
//! all size classes in a period the branch predictor can learn, and
//! `random` draws sizes unpredictably.

use criterion::{Criterion, criterion_group, criterion_main};
use integer_encoding::VarInt;
use std::hint::black_box;
use stream_vbyte::decode::decode as svb_decode;
use stream_vbyte::encode::encode as svb_encode;
use stream_vbyte::scalar::Scalar;
use vlen::{
	Decode, Encode, bulk_decode_u32, bulk_encode_u32, decode_u32, encode_u32,
};

const N: usize = 1024;

fn values(kind: &str) -> Vec<u32> {
	(0..N as u32)
		.map(|i| match kind {
			"small" => i % 0x80,
			"random" => {
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
			_ => match i % 4 {
				0 => i,
				1 => 1000 + i,
				2 => 1_000_000 + i,
				_ => 1_000_000_000 + i,
			},
		})
		.collect()
}

fn bench_single_encode(c: &mut Criterion) {
	let mut group = c.benchmark_group("single_encode");
	let value = 12_345_678u32;

	let mut buf = [0u8; 5];
	group.bench_function("vlen", |b| {
		b.iter(|| encode_u32(black_box(&mut buf), black_box(value)))
	});

	let mut buf = [0u8; 5];
	group.bench_function("vlen_checked", |b| {
		b.iter(|| black_box(value).encode(black_box(&mut buf[..])).unwrap())
	});

	let mut buf = [0u8; 5];
	group.bench_function("leb128", |b| {
		b.iter(|| black_box(value).encode_var(black_box(&mut buf)))
	});

	let mut buf = Vec::with_capacity(10);
	group.bench_function("prost", |b| {
		b.iter(|| {
			buf.clear();
			prost::encoding::encode_varint(
				black_box(value as u64),
				black_box(&mut buf),
			);
			buf.len()
		})
	});

	group.bench_function("vint64", |b| {
		b.iter(|| vint64::encode(black_box(value as u64)).as_ref().len())
	});

	let mut buf = [0u8; 4];
	group.bench_function("fixed", |b| {
		b.iter(|| buf = black_box(value).to_le_bytes())
	});

	group.finish();
}

fn bench_single_decode(c: &mut Criterion) {
	let mut group = c.benchmark_group("single_decode");
	let value = 12_345_678u32;

	let mut buf = [0u8; 5];
	let _ = encode_u32(&mut buf, value);
	group.bench_function("vlen", |b| b.iter(|| decode_u32(black_box(&buf))));

	group.bench_function("vlen_checked", |b| {
		b.iter(|| u32::decode(black_box(&buf[..])).unwrap())
	});

	let mut buf = [0u8; 5];
	let _ = value.encode_var(&mut buf);
	group.bench_function("leb128", |b| {
		b.iter(|| u32::decode_var(black_box(&buf)).unwrap())
	});

	let mut buf = Vec::new();
	prost::encoding::encode_varint(value as u64, &mut buf);
	group.bench_function("prost", |b| {
		b.iter(|| {
			let mut slice = black_box(&buf[..]);
			prost::encoding::decode_varint(&mut slice).unwrap()
		})
	});

	let encoded = vint64::encode(value as u64);
	group.bench_function("vint64", |b| {
		b.iter(|| {
			let mut slice = black_box(encoded.as_ref());
			vint64::decode(&mut slice).unwrap()
		})
	});

	let buf = value.to_le_bytes();
	group.bench_function("fixed", |b| {
		b.iter(|| u32::from_le_bytes(*black_box(&buf)))
	});

	group.finish();
}

fn bench_bulk_encode(c: &mut Criterion) {
	for kind in ["small", "mixed", "random"] {
		let mut group = c.benchmark_group(format!("bulk_encode/{kind}"));
		let values = values(kind);
		let wide: Vec<u64> = values.iter().map(|&v| v as u64).collect();
		let mut buf = vec![0u8; N * 5];

		group.bench_function("vlen", |b| {
			b.iter(|| {
				bulk_encode_u32(black_box(&mut buf), black_box(&values))
					.unwrap()
			})
		});

		group.bench_function("leb128", |b| {
			b.iter(|| {
				let mut offset = 0;
				for &value in black_box(&values) {
					offset += value.encode_var(&mut buf[offset..]);
				}
				offset
			})
		});

		let mut vec_buf = Vec::with_capacity(N * 10);
		group.bench_function("prost", |b| {
			b.iter(|| {
				vec_buf.clear();
				for &value in black_box(&wide) {
					prost::encoding::encode_varint(value, &mut vec_buf);
				}
				vec_buf.len()
			})
		});

		group.bench_function("vint64", |b| {
			b.iter(|| {
				let mut offset = 0;
				for &value in black_box(&wide) {
					let encoded = vint64::encode(value);
					let bytes = encoded.as_ref();
					buf[offset..offset + bytes.len()].copy_from_slice(bytes);
					offset += bytes.len();
				}
				offset
			})
		});

		group.bench_function("stream_vbyte", |b| {
			b.iter(|| {
				svb_encode::<Scalar>(black_box(&values), black_box(&mut buf))
			})
		});

		group.bench_function("fixed", |b| {
			b.iter(|| {
				for (dst, &value) in
					buf.chunks_exact_mut(4).zip(black_box(&values))
				{
					dst.copy_from_slice(&value.to_le_bytes());
				}
				N * 4
			})
		});

		group.finish();
	}
}

fn bench_bulk_decode(c: &mut Criterion) {
	for kind in ["small", "mixed", "random"] {
		let mut group = c.benchmark_group(format!("bulk_decode/{kind}"));
		let values = values(kind);
		let wide: Vec<u64> = values.iter().map(|&v| v as u64).collect();
		let mut decoded = vec![0u32; N];
		let mut decoded_wide = vec![0u64; N];

		let mut vlen_buf = vec![0u8; N * 5];
		let vlen_len = bulk_encode_u32(&mut vlen_buf, &values).unwrap();
		let vlen_encoded = &vlen_buf[..vlen_len];
		group.bench_function("vlen", |b| {
			b.iter(|| {
				bulk_decode_u32(
					black_box(vlen_encoded),
					black_box(&mut decoded),
				)
				.unwrap()
			})
		});

		let mut leb_buf = vec![0u8; N * 5];
		let mut leb_len = 0;
		for &value in &values {
			leb_len += value.encode_var(&mut leb_buf[leb_len..]);
		}
		let leb_encoded = &leb_buf[..leb_len];
		group.bench_function("leb128", |b| {
			b.iter(|| {
				let mut offset = 0;
				for slot in decoded.iter_mut() {
					let (value, len) =
						u32::decode_var(black_box(&leb_encoded[offset..]))
							.unwrap();
					*slot = value;
					offset += len;
				}
				offset
			})
		});

		let mut prost_buf = Vec::new();
		for &value in &wide {
			prost::encoding::encode_varint(value, &mut prost_buf);
		}
		group.bench_function("prost", |b| {
			b.iter(|| {
				let mut slice = black_box(&prost_buf[..]);
				for slot in decoded_wide.iter_mut() {
					*slot = prost::encoding::decode_varint(&mut slice).unwrap();
				}
				prost_buf.len() - slice.len()
			})
		});

		let mut vint_buf = Vec::new();
		for &value in &wide {
			vint_buf.extend_from_slice(vint64::encode(value).as_ref());
		}
		group.bench_function("vint64", |b| {
			b.iter(|| {
				let mut slice = black_box(&vint_buf[..]);
				for slot in decoded_wide.iter_mut() {
					*slot = vint64::decode(&mut slice).unwrap();
				}
				vint_buf.len() - slice.len()
			})
		});

		let mut svb_buf = vec![0u8; N * 5 + 16];
		let svb_len = svb_encode::<Scalar>(&values, &mut svb_buf);
		let svb_encoded = &svb_buf[..svb_len];
		group.bench_function("stream_vbyte", |b| {
			b.iter(|| {
				svb_decode::<Scalar>(
					black_box(svb_encoded),
					N,
					black_box(&mut decoded),
				)
			})
		});

		let fixed_buf: Vec<u8> =
			values.iter().flat_map(|v| v.to_le_bytes()).collect();
		group.bench_function("fixed", |b| {
			b.iter(|| {
				for (slot, src) in decoded
					.iter_mut()
					.zip(black_box(&fixed_buf).chunks_exact(4))
				{
					*slot = u32::from_le_bytes(src.try_into().unwrap());
				}
				N * 4
			})
		});

		group.finish();
	}
}

criterion_group!(
	benches,
	bench_single_encode,
	bench_single_decode,
	bench_bulk_encode,
	bench_bulk_decode,
);
criterion_main!(benches);
