# vlen: High-performance variable-length numeric encoding

`vlen` is an enhanced version of the original `vu128` variable-length
numeric encoding. Numeric types up to 128 bits are supported (integers
and floating-point), with smaller values being encoded using fewer
bytes. Every integer width shares one wire format, so a value encoded
as one type decodes as any wider type.

The compression ratio of `vlen` equals or exceeds the widely used
[VLQ] and [LEB128] encodings, and it decodes faster on modern pipelined
architectures because the encoded length is announced by the first byte
instead of continuation bits spread across the value.

[VLQ]: https://en.wikipedia.org/wiki/Variable-length_quantity
[LEB128]: https://en.wikipedia.org/wiki/LEB128

## Highlights

- **Safe**: no `unsafe` anywhere (`#![deny(unsafe_code)]`), and the
  checked API validates untrusted input with typed errors instead of
  panicking or desynchronizing.
- **Fast**: branch-light single-value codec, plus bulk operations with
  a SWAR fast path for runs of small values.
- **`const fn` everywhere**: every array-based encode/decode function
  works in const contexts.
- **`no_std`**: the core has zero dependencies and builds for embedded
  targets; `serde` support stays `no_std` and allocation-free.

## Usage

### Encoding and decoding

The `Encode` and `Decode` traits work on ordinary slices and validate
everything: buffers only need to fit the value's actual encoded size,
truncated input and invalid prefixes are rejected with typed errors,
and decoding a value that overflows the target type fails cleanly.

```rust
use vlen::{Decode, Encode};

let mut buf = [0u8; 5];
let value = 12345u32;

let len = value.encode(&mut buf)?;
assert_eq!(len, value.encoded_size());

let (decoded, decoded_len) = u32::decode(&buf[..len])?;
assert_eq!(decoded, value);
assert_eq!(decoded_len, len);
# Ok::<(), vlen::Error>(())
```

The array-based functions are the infallible fast core; their array
parameter types guarantee room for any value of the type, and they are
all usable in const contexts:

```rust
const LEN: usize = {
	let mut buf = [0u8; 5];
	vlen::encode_u32(&mut buf, 12345)
};
assert_eq!(LEN, 2);
```

### Bulk operations and streams

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

All bulk functions produce and consume the canonical byte stream —
output is byte-for-byte identical to encoding each value individually,
and the bulk and per-value APIs interoperate freely. The
`u32`-specialized `bulk_encode_u32`/`bulk_decode_u32` add a portable
SWAR fast path that processes runs of one-byte encodings eight at a
time; prefer them when your data is predominantly small values.

Indicative numbers for 1,024 values (Apple M-series, `--quick`
criterion run — measure on your own hardware):

| Distribution | specialized vs generic encode | specialized vs generic decode |
|--------------|------------------------------:|------------------------------:|
| all < 128    | **4.4x faster**               | **5.3x faster**               |
| all 5-byte   | ~1.1x slower                  | **1.5x faster**               |
| mixed sizes  | ~1.2x slower                  | ~1.2x slower                  |
| random sizes | ~1.1x slower                  | ~1.1x slower                  |

### Performance notes

The scalar codec is deliberately branchy rather than branchless: a
decoder's next read position depends on the current value's length, and
letting the branch predictor speculate through that chain overlaps
iterations, which measures substantially faster on modern out-of-order
cores than a branch-free implementation whose arithmetic becomes the
serial critical path (a branch-free variant of this codec benchmarked
about 2.7x slower on unpredictable bulk decodes). Size calculations
(`encoded_size_*`, `encoded_len`) are branch-free, so summing sizes
over a slice vectorizes.

vlen is sensitive to inlining. If encode/decode shows up in your
profiles, build with `lto = "thin"` (or `"fat"`) and consider
`codegen-units = 1` in your release profile; `-C target-cpu=native`
helps the SWAR bulk paths.

### Comparison with other encodings

`benches/comparison.rs` measures vlen against LEB128
(`integer-encoding`), the protobuf varint from `prost`, the `vint64`
prefix varint, `stream-vbyte` group varint (scalar kernels), and raw
fixed-width `u32` words. All codecs run through their in-memory slice
APIs. From the same machine and run:

| Benchmark (1,024 u32) | vlen | leb128 | prost | vint64 | stream-vbyte |
|-----------------------|-----:|-------:|------:|-------:|-------------:|
| bulk encode, mixed    | 1.08 µs | 1.67 µs | 1.93 µs | 3.05 µs | **0.86 µs** |
| bulk decode, mixed    | **1.45 µs** | 2.70 µs | 2.19 µs | 2.67 µs | 1.75 µs |
| bulk encode, small    | **0.15 µs** | 0.88 µs | 1.03 µs | 2.10 µs | 0.63 µs |
| bulk decode, small    | **0.12 µs** | 2.24 µs | 2.06 µs | 2.44 µs | 1.26 µs |
| single encode (4-byte value) | 1.49 ns | 1.99 ns | 2.86 ns | **1.10 ns** | – |
| single decode (4-byte value) | **0.78 ns** | 2.40 ns | 0.84 ns | 2.27 ns | – |

Caveats: `stream-vbyte` is a different format (external count, separate
control stream) with an SSE4.1 decoder that outperforms these scalar
numbers on x86_64, and `prost`/`vint64` are 64-bit codecs fed the same
values widened to `u64`.

### Serde integration

With the `serde` feature, the `Vlen*` wrapper types serialize through
the vlen codec. Binary formats (postcard, bincode, ...) receive the raw
encoded bytes; human-readable formats (JSON, ...) receive base64.
Neither path allocates, and malformed input is rejected with errors.

```rust
use serde::{Deserialize, Serialize};
use vlen::serde::{VlenI64, VlenU32};

#[derive(Serialize, Deserialize)]
struct MyStruct {
	id: VlenU32,
	timestamp: VlenI64,
}
```

## Features

- **`alloc`**: `Vec`-based convenience functions (`encode_to_vec`,
  `bulk_encode_to_vec`, `bulk_decode_values`)
- **`serde`**: serde wrapper types (allocation-free, `no_std`)
- **`full`**: everything above

The minimum supported Rust version is **1.85**.

## Encoding details

Values in the range `[0, 2^7)` are encoded as a single byte with
the same bits as the original value.

Values in the range `[2^7, 2^28)` are encoded as a unary length prefix,
followed by `(length*7)` bits, in little-endian order. This is conceptually
similar to LEB128, but the continuation bits are placed in upper half
of the initial byte. This arrangement is also known as a "prefix varint".

```text
MSB ------------------ LSB

      10101011110011011110  Input value (0xABCDE)
   0101010 1111001 1011110  Zero-padded to a multiple of 7 bits
01010101 11100110 ___11110  Grouped into octets, with 3 continuation bits
01010101 11100110 11011110  Continuation bits `110` added
    0x55     0xE6     0xDE  In hexadecimal

        [0xDE, 0xE6, 0x55]  Encoded output (order is little-endian)
```

Values in the range `[2^28, 2^128)` are encoded as a binary length prefix,
followed by payload bytes, in little-endian order. To differentiate this
format from the format of smaller values, the top 4 bits of the first byte
are set. The length prefix value is the number of payload bytes minus one;
equivalently it is the total length of the encoded value minus two.

```text
MSB ------------------------------------ LSB

               10010001101000101011001111000  Input value (0x12345678)
         00010010 00110100 01010110 01111000  Zero-padded to a multiple of 8 bits
00010010 00110100 01010110 01111000 11110011  Prefix byte is `0xF0 | (4 - 1)`
    0x12     0x34     0x56     0x78     0xF3  In hexadecimal

              [0xF3, 0x78, 0x56, 0x34, 0x12]  Encoded output (order is little-endian)
```

Every integer width uses this same grammar, so the encodings of a value
are identical whether it is encoded as `u16`, `u32`, `u64`, or `u128`.

## Handling of over-long encodings

The `vlen` format permits over-long encodings, which encode a value using
a byte sequence that is unnecessarily long:

- Zero-padding beyond that required to reach a multiple of 7 or 8 bits.
- Using a length prefix byte for a value in the range `[0, 2^7)`.
- Using a binary length prefix byte for a value in the range `[0, 2^28)`.

The `encode_*` functions in this module will not generate such over-long
encodings, but the `decode_*` functions will accept them. This is intended
to allow `vlen` values to be placed in a buffer before the value to be
written is known. Applications that require a single canonical encoding for
any given value should perform appropriate checking in their own code.

## Signed integers and floating-point values

Signed integers and IEEE-754 floating-point values may be encoded with
`vlen` by mapping them to unsigned integers. It is recommended that the
mapping functions be chosen so as to minimize the number of zeroes in the
higher-order bits, which enables better compression.

This library includes helper functions that use Protocol Buffer's ["ZigZag"
encoding] for signed integers and reverse-endian layout for floating-point.

["ZigZag" encoding]: https://protobuf.dev/programming-guides/encoding/#signed-ints

## Breaking changes in 0.4.0

- Errors are a typed [`enum Error`] implementing `core::error::Error`
  instead of `&'static str`; `encoded_size` and the `Vec` constructors
  are infallible.
- `Encode::encode` takes `self` first (`value.encode(&mut buf)`), and
  the checked API works with exactly-sized buffers on both sides.
- **Wire format**: `u16`/`i16` three-byte encodings now use the same
  prefix-varint grammar as the wider types. The previous `0xDE`-prefixed
  raw form conflicted with the shared grammar and made `u16` streams
  unreadable as `u32`/`u64`/`u128`. Two- and one-byte `u16` encodings
  are unchanged.
- The `simd` feature was removed. Its implementation produced
  non-canonical output, decoded incorrectly, and contained
  out-of-bounds accesses. The safe bulk functions (always available)
  replace it; `bulk_encode_u32_safe` is now `bulk_encode_u32`.
- The `const_encode`/`const_decode` modules were removed: the main
  array-based functions are all `const fn` now.
- Binary serde formats receive raw bytes instead of base64 strings.
  JSON output is unchanged.

## License

Licensed under **MPL-2.0** to guarantee future openness - see [LICENSE](LICENSE).

Retains `vu128` (ISC/0BSD) attribution in [LICENSE-VU128.txt](LICENSE-VU128.txt).

## Acknowledgments

This crate is based on the original `vu128` implementation by John Millikin,
with performance improvements and enhancements by Harrison Chin.
