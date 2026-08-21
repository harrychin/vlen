# Changelog

## 0.4.3

### Changed

- The cold short-buffer paths of the checked codec copy with plain
  byte loops instead of variable-length `copy_from_slice`, so minimal
  builds no longer link `compiler_builtins` memcpy or any panic
  machinery. A Cortex-M binary using checked `u32` encode + decode
  measures ~500 bytes of code (down from ~1.25 KB) at
  `opt-level = "z"` with fat LTO and `panic = "abort"` - the checked
  codec now has zero dependencies beyond its own instructions.

## 0.4.2

### Changed

- Each prefix-varint encode form is now built as one little-endian
  word and stored whole instead of byte-by-byte. Wire format
  unchanged; bulk encoding of mixed-size streams is ~23% faster
  (halving the gap to stream-vbyte, whose control-stream format
  retains the remaining ~14% edge on interleaved sizes), and random
  streams gain ~8%.

## 0.4.1

### Added

- Sixteen-byte run windows for two- and four-byte decode: eight
  two-byte or four four-byte encodings per step in portable safe
  Rust, roughly 20% faster than the previous eight-byte windows.
- An opt-in `simd` feature (also part of `full`) with audited native
  kernels for the run lane reassemblies: NEON (aarch64), SSE2
  (x86_64), and simd128 (wasm32 with `+simd128`), covering one-,
  two-, and four-byte decode runs - about 15-19% on those paths.
  Every instruction set used is baseline for its target (simd128 is
  compile-time gated), so there is no runtime detection; default
  builds still contain no unsafe code. The full suite runs on all
  three architectures in CI, kernels on and off.
- Two- and four-byte encode runs now serialize a lane array whose
  little-endian bytes are exactly the wire encoding - safe code the
  autovectorizer turns into SIMD on every platform, 41% and 30%
  faster respectively, and the four-byte path no longer needs
  scratch bytes.

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
