# vlen

**The fastest-decoding self-delimiting varint for Rust.** Integers and
floats up to 128 bits, smaller values in fewer bytes, zero
dependencies, no unsafe code by default, `no_std`.

```rust
use vlen::{Decode, Encode};

let mut buf = [0u8; 5];
let len = 12345u32.encode(&mut buf)?;          // 2 bytes
let (value, _) = u32::decode(&buf[..len])?;    // 12345
# Ok::<(), vlen::Error>(())
```

## Choosing an API

| Need | Use |
|------|-----|
| One checked value | `Encode`/`Decode` or `encode`/`decode` |
| Canonical first value | `decode_canonical` |
| Exact whole input | `decode_exact`; use `decode_strict` when it must also be canonical |
| A mixed-type message | `Writer` and `Reader`; add `read_canonical` and `finish` for strict fields and framing |
| A homogeneous batch | `bulk_encode`/`bulk_decode`, or the specialized `u32`, `u64`, `i32`, and `i64` variants |
| Lazy stream decoding | `decode_iter`, or a specialized iterator such as `decode_iter_u32` |
| An owned buffer (`alloc`) | `encode_to_vec`, `encode_append`, and the bulk `Vec` helpers |
| Compile-time or trusted fixed arrays | `encode_u32`/`decode_u32` and their typed counterparts |

## Why vlen

**It decodes faster than every varint we could find to compare
against — including on adversarial input.** The length lives in the
first byte instead of continuation bits spread across the value, so
decoding is one predicted branch:

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/harrychin/vlen/main/assets/benchmarks-dark.svg">
  <img alt="Bulk decode of 1,024 u32 values: vlen is fastest across small, mixed, and random distributions against LEB128, prost, vint64, and stream-vbyte" src="https://raw.githubusercontent.com/harrychin/vlen/main/assets/benchmarks-light.svg">
</picture>

Encode and single-value results from the same run, with honest losses
included:

| Benchmark (1,024 u32) | vlen | LEB128 | prost | vint64 | stream-vbyte |
|-----------------------|-----:|-------:|------:|-------:|-------------:|
| bulk encode, small values | **0.14 µs** | 0.77 µs | 0.90 µs | 1.93 µs | 0.51 µs |
| bulk encode, mixed sizes  | **0.82 µs** | 1.48 µs | 1.86 µs | 2.40 µs | 0.98 µs |
| bulk encode, random sizes | 1.63 µs | 1.78 µs | 2.70 µs | 2.99 µs | **1.08 µs** |
| single decode (4-byte value) | **0.58 ns** | 2.01 ns | 0.80 ns | 2.36 ns | – |
| single encode (4-byte value) | 1.50 ns | 1.86 ns | 2.44 ns | **0.81 ns** | – |

Methodology: every codec through its fastest public in-memory API over
identical data, from one run of `benches/comparison.rs`. The bulk rows
and the chart all use vlen's validating API. The single-value rows use
its infallible array API; through the validating slice API — the same
work the other crates always do — vlen measures 0.77 ns decode /
1.58 ns encode, level with prost's validating decode (0.80 ns).
stream-vbyte runs its scalar kernels (its SSE4.1 decoder is faster on
x86_64) and is a control-stream format rather than a self-delimiting
varint; `prost` and `vint64` are 64-bit codecs fed the same values
widened to `u64`.

Compression matches LEB128 byte-for-byte below 2^28 — where most
varint data lives — and caps at 9 bytes for `u64`, where LEB128 needs
up to 10.

**Strict when the protocol needs one representation.** The ordinary
decoder deliberately accepts over-long encodings, which lets a format
reserve a fixed-width slot before its value is known. For signed or hashed
data, or any field that must consume its entire slice,
`decode_canonical`, `decode_exact`, and `decode_strict` add allocation-
free validation without changing the permissive codec:

```rust
let reserved = [0x85, 0x00]; // the value 5 in an over-long slot
assert_eq!(vlen::decode::<u32>(&reserved)?, (5, 2));
assert!(matches!(
    vlen::decode_strict::<u32>(&reserved),
    Err(vlen::StrictError::NonCanonical { .. })
));
# Ok::<(), vlen::Error>(())
```

**It is safe to point at untrusted bytes.** Default builds contain no
unsafe code (`#![deny(unsafe_code)]`); the opt-in `simd` feature adds
two small, audited NEON/SSE2 kernels whose soundness holds by
construction (baseline target features, array-derived pointers). The
checked API returns typed errors — never panics, never desynchronizes
on truncated input, invalid prefixes, or out-of-range values — and
decoding needs only the bytes a value actually occupies.

**It runs everywhere, at compile time too.** The core is dependency-
free `no_std`. On Cortex-M (`opt-level = "z"`, fat LTO), the unchecked
array API compiles to ~200 bytes and the fully validating codec to
~500 — with no panic paths and no memcpy dependency — where the
comparable `vint64` crate measures ~1.2 KB in the same harness. Every
array-based codec function is also `const fn`:

```rust
const LEN: usize = {
    let mut buf = [0u8; 5];
    vlen::encode_u32(&mut buf, 12345)
};
```

**One wire format across all widths.** A value encoded as `u16`
produces the same bytes as `u32`, `u64`, or `u128`, and decodes at any
width that can hold it — no cross-type surprises when a field grows.

**Bulk operations that exploit your data's shape.** The specialized
bulk functions for `u32`, `u64`, `i32`, and `i64` detect runs of
similarly-sized values and move them without per-value length
arithmetic — up to 4x faster than the per-value loop on small-value
streams, and the signed variants are built for delta-encoded data
(smooth delta streams decode ~1.8x faster than the generic loop) — in
portable safe Rust on every architecture. Streaming callers get the
same treatment: the run-accelerated iterators (`decode_iter_u32` and
friends) iterate small-value streams ~4.6x faster than the generic
`decode_iter`, which handles any type:

```rust
use vlen::{bulk_encode, decode_iter};

let values = [1u64, 250, 70_000, u64::MAX];
let mut buf = [0u8; 36];
let len = bulk_encode(&mut buf, &values)?;

let decoded: Result<Vec<u64>, vlen::Error> =
    decode_iter(&buf[..len]).collect();
assert_eq!(decoded?, values);
# Ok::<(), vlen::Error>(())
```

**Ergonomics that scale from one value to a protocol.** Every
integer width is supported — including `usize` for length prefixes,
with a platform-independent wire format — and the `Writer`/`Reader`
cursors handle mixed-type messages without offset bookkeeping:

```rust
let mut buf = [0u8; 16];
let mut writer = vlen::Writer::new(&mut buf);
writer.write(7u32)?;
writer.write(-42i64)?;
let len = writer.finish();

let mut reader = vlen::Reader::new(&buf[..len]);
assert_eq!(reader.read_canonical::<u32>()?, 7);
assert_eq!(reader.read_canonical::<i64>()?, -42);
reader.finish()?;
# Ok::<(), vlen::StrictError>(())
```

**Serde that respects your format.** With the `serde` feature,
annotate plain fields with `#[serde(with = "vlen::serde::u32")]` (or
use the `Vlen*` wrapper types): binary formats (postcard, bincode,
...) get the raw encoded bytes and human-readable formats (JSON, ...)
get base64 — allocation-free either way, hostile input rejected with
errors.

## Features

| Feature | Adds |
|---------|------|
| `alloc` | `Vec` conveniences: `encode_to_vec`/`encode_append`, `bulk_encode_to_vec`/`bulk_encode_append`, and `bulk_decode_values` |
| `serde` | `Vlen*` wrapper types (allocation-free, `no_std`) |
| `simd`  | Native NEON/SSE2/wasm-simd128 kernels for the bulk run fast paths (~15-19% faster one-, two-, and four-byte runs; a handful of audited load/store unsafe blocks) |
| `full`  | Everything above |

MSRV: **1.85**. Tested in CI on x86_64, aarch64, big-endian s390x,
and wasm32, stable and MSRV, with clippy, rustfmt, fuzz smoke tests,
semver checks, and `no_std` builds gating every change.

## When something else fits better

If your workload is purely columnar bulk `u32` compression — no
streaming, no self-delimiting values — a control-stream format like
`stream-vbyte` encodes unpredictably interleaved sizes faster (its
lengths live in a separate control stream, so per-value size changes
cost it nothing). vlen is built for the general case: self-delimiting
streams you can read value by value.

## Learn more

- [examples/wal.rs](examples/wal.rs) — a runnable write-ahead log:
  framing, delta encoding, torn-tail crash recovery, and the bulk
  paths in ~150 lines (`cargo run --release --example wal --features alloc`)
- [API documentation](https://docs.rs/vlen)
- [DESIGN.md](DESIGN.md) — wire format specification and performance
  design notes
- [CHANGELOG.md](CHANGELOG.md) — release notes and migration guides

## License

MPL-2.0 — see [LICENSE](LICENSE). Based on the original `vu128`
implementation by John Millikin (attribution in
[LICENSE-VU128.txt](LICENSE-VU128.txt)), with performance improvements
and enhancements by Harrison Chin.
