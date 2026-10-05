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
reserved in a buffer before the value to be written is known;
`encode_padded` (or `Writer::reserve` and `Writer::fill`) writes a
value into such a slot using exactly its width.
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

Encoding has no chain through memory either: a value's output offset
waits only on the previous length's addition. So the specialized bulk
encoders encode windows of mixed sizes branch-free (below), and only
single-value and generic encoding stay branchy.

### Bulk run detection

The specialized bulk functions detect runs of equal-length encodings.
Within a run, value boundaries are known in advance, which removes the
per-value length arithmetic entirely:

- One-byte runs move eight values per step (a continuation-bit mask on
  a `u64` word); two-byte runs reassemble four values inside 16-bit
  lanes at once.
- Three- to five-byte encode runs emit one class-known full-width
  store per value, and `u64` six- to nine-byte encode runs a prefix
  byte plus one eight-byte store; four-byte decode runs use 32-bit
  lanes, and binary-length-prefix decode runs (five to nine bytes) use
  pairs of plain masked loads.
- Three-byte decode runs stay on the branchy scalar path, which
  measured faster than their SWAR lane math, unless the SSSE3 kernel
  below is compiled in.
- The window checks are gated so that streams with no runs pay only a
  compare or two per eight values.
- Encode windows that are not runs go branch-free: a table indexed by
  each value's leading zeros gives its layout (prefix bits, low-bit
  mask, shift, and length), the word `prefix | (v & low) | ((v >>
  shift) << 8)` is stored whole at the running offset, and the offset
  advances by the length. Interleaved sizes then cost no
  mispredictions: random-size `u32` streams encode 1.37x faster at
  x86-64-v1 and 1.59x at v3 than with the per-value fallback. BMI2's
  `pdep` builds the same word in one instruction and measured ~20%
  faster at v3, but it is microcoded and very slow on AMD Zen 1 and 2,
  which also qualify as x86-64-v3, so the table form is used.

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

#### x86 beyond SSE2

Builds that target x86-64-v2 or newer (`-C target-cpu=x86-64-v2`,
`x86-64-v3`, `native`) also get an SSSE3 kernel for three-byte decode
runs: one `pshufb` per sixteen bytes spreads each encoding into its
own 32-bit lane, a single compare checks all eight prefixes, and two
shifts rebuild the values. It is selected by `cfg(target_feature)`, so
there is still no runtime detection — a binary built for it requires
it — and an x86-64-v1 build compiles exactly the code it did before.
Measured on the same VM, three-byte runs decode 4.6x faster at
x86-64-v2 and 3.2-3.7x at x86-64-v3, with other distributions
unchanged at v2. At v3, specialized decoding of interleaved sizes
measured 9-23% slower with the kernel present, from code layout
rather than work done (the kernel never runs there); the generic
decoder is the faster choice for such data either way.

Wider kernels were measured and not kept. With eight-value windows, an
AVX2 kernel can only help four-byte runs (eight per 32-byte step
instead of four per sixteen): inlined, it decoded them 1.8x faster but
slowed interleaved streams by about 24%; kept out of line, the call
cost erased the gain. AVX-512 has no lane width left to fill in an
eight-value window. Both levels still build and pass the full suite,
and CI tests x86-64-v2, v3, and (where the runner has AVX-512) v4.
LLVM's own autovectorization of the encode paths picks up SSE4.1 and
AVX2 without explicit kernels.

### Specialized vs generic bulk functions

Indicative numbers for 1,024 values (x86_64, 4-vCPU cloud VM; each
pair timed in one binary, best of 40 batches — measure on your own
hardware). "All n-byte" draws values uniformly across that size class:

| Distribution                    | specialized vs generic encode | specialized vs generic decode |
|---------------------------------|------------------------------:|------------------------------:|
| all one-byte                    | **~1.7x faster**              | **~2.5x faster**              |
| all two-byte                    | **~4x faster**                | **~2.5x faster**              |
| all three-byte                  | **~1.4x faster**              | about even                    |
| all four-byte                   | **~2x faster**                | **~2.3x faster**              |
| all five-byte                   | **~2.4x faster**              | **~1.3x faster**              |
| `u64`, all six- to nine-byte    | **~2-2.4x faster**            | **~1.3x faster**              |
| mixed / random sizes            | ~1.1-1.2x faster              | ~1.3-1.4x slower              |
| `u64`, random six- to nine-byte | ~1.15x slower                 | ~1.2x slower                  |

Prefer the specialized functions whenever the data has runs of
similarly-sized values; only interleaved sizes favor the generic
decoders. On the encode side the branch-free mixed windows closed that
gap; on the decode side it remains, because a decoder cannot know
where the next value starts without reading the current one's prefix.
Ways of closing it that were measured and dropped, because each bought
mixed-data speed with run speed: a branch-free pre-test of every
decode run class (random sizes ~8% faster, runs 12-19% slower), and,
before the branch-free windows, one table-driven encode gate for all
classes (runs 21-42% slower) and a min/max test over the window (runs
1.4-2.5x slower). The `alloc` `Vec` helpers take the specialized paths for
`u32`, `u64`, `i32`, and `i64`.

The criterion benches in `benches/bulk.rs` are sensitive to code
layout on the generic side: the same generic encode has measured
anywhere from 1.3 to 2.2 µs there as unrelated code changed, so
compare both sides within one build.

### Build configuration

vlen is sensitive to inlining. If encode/decode shows up in your
profiles, build with `lto = "thin"` (or `"fat"`) and consider
`codegen-units = 1` in your release profile; `-C target-cpu=native`
helps the SWAR bulk paths.
