# vlen design notes

Reference material for the wire format and the engineering decisions
behind the implementation. For API documentation, see
[docs.rs/vlen](https://docs.rs/vlen).

## Wire format

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
are identical whether it is encoded as `u16`, `u32`, `u64`, or `u128`,
and a value encoded at one width decodes at any width that can hold it.

### Over-long encodings

The format permits over-long encodings, which encode a value using a
byte sequence that is unnecessarily long:

- Zero-padding beyond that required to reach a multiple of 7 or 8 bits.
- Using a length prefix byte for a value in the range `[0, 2^7)`.
- Using a binary length prefix byte for a value in the range `[0, 2^28)`.

The `encode_*` functions never generate over-long encodings, but the
`decode_*` functions accept them. This allows a `vlen` slot to be
reserved in a buffer before the value to be written is known.
Applications that require a single canonical encoding for any given
value can use `decode_canonical`; `decode_exact` separately requires
the first value to consume the whole slice, and `decode_strict`
requires both properties. Built-in types validate both encoded length
and canonical prefix form without allocating or re-encoding the value;
downstream codecs that accept same-length aliases define their match
through `Encode::is_canonical_encoding`.

### Signed integers and floating-point values

Signed integers use Protocol Buffers' ["ZigZag" encoding] and
floating-point values use a reverse-endian layout, both chosen to
minimize high-order bits so common values compress well.

["ZigZag" encoding]: https://protobuf.dev/programming-guides/encoding/#signed-ints

## Performance design

### Why the scalar codec is branchy

A decoder's next read position depends on the current value's length,
so decoding is a loop-carried dependency chain. Branch prediction lets
the CPU speculate through that chain and overlap iterations; a
branch-free implementation turns its own arithmetic into the serial
critical path. A branch-free variant of this codec was implemented and
benchmarked at about 2.7x slower on unpredictable bulk decodes, so the
branchy structure is a measured choice, not a default. Size
calculations (`encoded_size_*`, `encoded_len`) have no such chain and
are branch-free, so summing sizes over a slice vectorizes.

### Bulk run detection

The specialized bulk functions detect runs of equal-length encodings.
Within a run, value boundaries are known in advance, which removes the
per-value length arithmetic entirely:

- One-byte runs move eight values per step (a continuation-bit mask on
  a `u64` word); two-byte runs reassemble four values inside 16-bit
  lanes at once.
- Three- to five-byte encode runs emit one class-known full-width
  store per value; four-byte decode runs use 32-bit lanes, and
  binary-length-prefix decode runs (five to nine bytes) use pairs of
  plain masked loads.
- Three-byte decode runs deliberately stay on the branchy scalar path,
  which measured faster than their SWAR lane math.
- The window checks are gated so that streams with no runs pay only a
  compare or two per eight values.

### SIMD: where it helps and where it cannot

With an inline self-delimiting varint, discovering where each value
starts requires reading the previous value's first byte, so wide
shuffles cannot bypass the boundary chain the way they can for formats
with a separate control stream (group varint / stream-vbyte) or
per-byte continuation bits (LEB128). vlen trades that away for the
fastest scalar and streaming decode; the run paths recover batch speed
exactly where boundaries are uniform and therefore known in advance.
If your workload is columnar bulk `u32` compression above all else, a
control-stream format like stream-vbyte is the better tool.

Inside a detected run, however, boundaries are known and real
data-parallelism exists. The opt-in `simd` feature replaces the SWAR
lane reassembly for two- and four-byte decode runs with NEON
(aarch64) and SSE2 (x86_64) kernels, worth about another 15% there.
Both instruction sets are baseline features of their targets, so no
runtime detection is involved; the unsafe surface is two short
functions per architecture whose load/store pointers come from array
references, and the full test suite runs with the feature on and off,
on both architectures, in CI. Default builds remain free of unsafe
code.

### Specialized vs generic bulk functions

Indicative numbers for 1,024 values (Apple M-series, `--quick`
criterion run — measure on your own hardware):

| Distribution    | specialized vs generic encode | specialized vs generic decode |
|-----------------|------------------------------:|------------------------------:|
| all one-byte    | **~4x faster**                | **~4x faster**                |
| all two-byte    | **~1.2x faster**              | **~3.7x faster**              |
| all three-byte  | **~1.5x faster**              | ~1.2x slower                  |
| all four-byte   | **~1.7x faster**              | **~2.4x faster**              |
| all five-byte   | **~3.5x faster**              | **~2x faster**                |
| mixed / random  | ~1.2-1.4x slower              | ~1.1-1.4x slower              |

Prefer the specialized functions whenever the data has runs of
similarly-sized values; only adversarially interleaved sizes favor the
generic functions.

### Build configuration

vlen is sensitive to inlining. If encode/decode shows up in your
profiles, build with `lto = "thin"` (or `"fat"`) and consider
`codegen-units = 1` in your release profile; `-C target-cpu=native`
helps the SWAR bulk paths.
