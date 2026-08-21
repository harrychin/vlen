# Changelog

## 0.4.1

### Added

- Sixteen-byte run windows for two- and four-byte decode: eight
  two-byte or four four-byte encodings per step in portable safe
  Rust, roughly 20% faster than the previous eight-byte windows.
- An opt-in `simd` feature (also part of `full`) with audited NEON
  (aarch64) and SSE2 (x86_64) kernels for those lane reassemblies,
  worth about another 15%. Both instruction sets are baseline for
  their targets, so there is no runtime detection; default builds
  still contain no unsafe code.

- Run-accelerated decoding iterators: `decode_iter_u32`, `decode_iter_u64`,
  `decode_iter_i32`, and `decode_iter_i64` refill an internal window
  through the same run fast paths as the specialized bulk functions,
  then serve values with an index bump. Streams of one-byte values
  iterate ~4.6x faster than the generic `decode_iter`, two-byte
  streams ~3.9x, and even unpredictable mixed streams gain ~1.2x -
  the buffered path never loses. Purely additive; the generic
  `decode_iter` is unchanged.

## 0.4.0

Complete overhaul of correctness, safety, and performance. This is a
breaking release; the notes below cover migration.

### Breaking changes

- Errors are a typed `Error` enum implementing `core::error::Error`
  (`BufferTooSmall`, `InvalidPrefix`, `Overflow`) instead of
  `&'static str`. `encoded_size` and the `Vec` constructors are
  infallible.
- `Encode::encode` takes `self` first (`value.encode(&mut buf)`), and
  the checked API works with exactly-sized buffers on both sides:
  decoding needs only the bytes a value occupies, and encoding fits a
  buffer sized to the actual value.
- **Wire format**: `u16`/`i16` three-byte encodings now use the same
  prefix-varint grammar as the wider types. The previous
  `0xDE`-prefixed raw form conflicted with the shared grammar and made
  `u16` streams unreadable as `u32`/`u64`/`u128`. One- and two-byte
  `u16` encodings are unchanged.
- The `simd` feature was removed. Its implementation produced
  non-canonical output, decoded incorrectly, and contained
  out-of-bounds accesses. The safe bulk functions (always available)
  replace it; `bulk_encode_u32_safe` is now `bulk_encode_u32`.
- The `const_encode`/`const_decode` modules were removed: the main
  array-based functions are all `const fn` now.
- Binary serde formats receive raw encoded bytes instead of base64
  strings (human-readable formats still receive base64). Malformed
  input is rejected with errors instead of panicking.
- Rust 2024 edition; MSRV is 1.85.

### Added

- `bulk_encode`/`bulk_decode` for any supported type, specialized
  `bulk_encode_u32`/`bulk_decode_u32` and `bulk_encode_u64`/
  `bulk_decode_u64` with run-detection fast paths, and a `decode_iter`
  streaming iterator.
- `#![deny(unsafe_code)]`: the crate contains no unsafe code.
- CI: feature matrix on x86_64 and aarch64, clippy and rustfmt gates,
  MSRV check, `no_std` target build, docs.rs configuration build.

## 0.3.0 and earlier

Prior releases added the (since removed) `simd` feature, `const fn`
modules, serde integration, and the initial `vlen` extension of the
original `vu128` codec. Users of the `simd` feature in 0.2.x-0.3.0
should upgrade immediately: its bulk encoder could write out of bounds
through a safe API, and its decoder returned incorrect values.
