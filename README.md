# vlen

**The fastest-decoding self-delimiting varint for Rust.** Integers and
floats up to 128 bits, smaller values in fewer bytes, zero
dependencies, zero unsafe code, `no_std`.

```rust
use vlen::{Decode, Encode};

let mut buf = [0u8; 5];
let len = 12345u32.encode(&mut buf)?;          // 2 bytes
let (value, _) = u32::decode(&buf[..len])?;    // 12345
# Ok::<(), vlen::Error>(())
```

## Why vlen

**It decodes faster than every varint we could find to compare
against.** The length lives in the first byte instead of continuation
bits spread across the value, so decoding is one predicted branch. On
1,024-value streams (Apple M-series; `benches/comparison.rs`, all
codecs through their in-memory slice APIs):

|                    | vlen | LEB128 | prost (protobuf) | vint64 |
|--------------------|-----:|-------:|------:|-------:|
| bulk decode, small values | **0.12 µs** | 2.24 µs | 2.06 µs | 2.44 µs |
| bulk decode, mixed sizes  | **1.45 µs** | 2.70 µs | 2.19 µs | 2.67 µs |
| bulk encode, small values | **0.15 µs** | 0.88 µs | 1.03 µs | 2.10 µs |
| single decode      | **0.78 ns** | 2.40 ns | 0.84 ns | 2.27 ns |

Compression matches LEB128 byte-for-byte below 2^28 — where most
varint data lives — and caps at 9 bytes for `u64`, where LEB128 needs
up to 10.

**It is safe to point at untrusted bytes.** The crate contains no
unsafe code (`#![deny(unsafe_code)]`). The checked API returns typed
errors — never panics, never desynchronizes on truncated input,
invalid prefixes, or out-of-range values — and decoding needs only the
bytes a value actually occupies.

**It runs everywhere, at compile time too.** The core is dependency-
free `no_std` (CI builds it for `thumbv7em-none-eabi`), and every
array-based codec function is `const fn`:

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
bulk functions detect runs of similarly-sized values and move them
without per-value length arithmetic — up to 4x faster than the
per-value loop on small-value streams, in portable safe Rust on every
architecture. A `decode_iter` streaming iterator handles streams of
unknown length:

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

**Serde that respects your format.** With the `serde` feature, the
`Vlen*` wrapper types hand binary formats (postcard, bincode, ...) the
raw encoded bytes and human-readable formats (JSON, ...) base64 —
allocation-free either way, hostile input rejected with errors.

## Features

| Feature | Adds |
|---------|------|
| `alloc` | `Vec` conveniences: `encode_to_vec`, `bulk_encode_to_vec`, `bulk_decode_values` |
| `serde` | `Vlen*` wrapper types (allocation-free, `no_std`) |
| `full`  | Everything above |

MSRV: **1.85**. Tested in CI on x86_64 and aarch64, stable and MSRV,
with clippy, rustfmt, and `no_std` builds gating every change.

## When something else fits better

If your workload is purely columnar bulk `u32` compression — no
streaming, no self-delimiting values — a control-stream format like
`stream-vbyte` can encode mixed-size batches faster. vlen is built for
the general case: self-delimiting streams you can read value by value.

## Learn more

- [API documentation](https://docs.rs/vlen)
- [DESIGN.md](DESIGN.md) — wire format specification and performance
  design notes
- [CHANGELOG.md](CHANGELOG.md) — release notes and migration guides

## License

MPL-2.0 — see [LICENSE](LICENSE). Based on the original `vu128`
implementation by John Millikin (attribution in
[LICENSE-VU128.txt](LICENSE-VU128.txt)), with performance improvements
and enhancements by Harrison Chin.
