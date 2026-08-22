use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use vlen::{Encode, Error, encode_append};

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

criterion_group!(benches, bench_encode_append);
criterion_main!(benches);
