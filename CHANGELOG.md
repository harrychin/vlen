# Changelog

## Unreleased

### Added

- `encode_padded`: writes a value into exactly `N` bytes, padding it to
  an over-long encoding when its canonical form is shorter, so a slot
  reserved before its value is known (a length prefix written after
  its payload) can be filled without hand-rolling the wire format.
  Every decoder for the type accepts the result; canonical decoding
  rejects it unless `N` is the canonical length. Widths beyond what
  the type can decode are a compile-time error.
- `Writer::reserve` and `Writer::fill`, with the `Slot` handle they
  share: reserve a zeroed fixed-width slot in a cursor and fill it
  later.
- With the `simd` feature, an SSSE3 kernel for three-byte decode runs
  in builds targeting x86-64-v2 or newer (`-C target-cpu=x86-64-v2`,
  `x86-64-v3`, `native`): selected at compile time, so x86-64-v1
  builds are unchanged. Three-byte runs decode 4.6x faster at v2 and
  3.2-3.7x at v3; see DESIGN.md for the v3 layout caveat on
  interleaved sizes and for the AVX2 and AVX-512 kernels that were
  measured and not kept.
- CI coverage: Miri over the portable paths and every x86 kernel, the
  suite at x86-64-v2, v3, and (where available) v4, a Cortex-M
  code-size check (`ci/size-check.sh`: no `memcpy` import, no panic
  paths, a byte budget per API surface), and an `encode_arbitrary`
  fuzz target covering the specialized encoders, the `Vec` helpers,
  `encode_padded`, and `Writer` slots.
- `Encode::encode_slice` and (with `alloc`) `Decode::decode_to_vec`:
  provided methods the generic `Vec` helpers go through, so a type
  with a faster bulk codec can supply it. Their defaults are
  `bulk_encode` and collecting `decode_iter`; the built-in `u32`,
  `u64`, `i32`, and `i64` implementations use the run fast paths.

### Changed

- The specialized bulk encoders (`bulk_encode_u32`, `_u64`, `_i32`,
  `_i64`) now detect every two- to five-byte run. The size-class
  check or-reduced the window before comparing, which is only exact
  for power-of-two spans: it rejected ~97% of windows whose values
  are spread across a class, so on such data the specialized
  encoders ran slower than the generic `bulk_encode`. Uniformly
  distributed same-size `u32` streams now encode 2.1-4.4x faster
  (two-byte 1.84 -> 0.49 µs per 1,024 values, five-byte 2.88 ->
  0.65 µs on x86_64); clustered runs are 1.2-1.3x faster at two and
  three bytes and ~10% slower at four. Output bytes are unchanged.
- `bulk_encode_u64` and `bulk_encode_i64` now have encode runs for the
  six- to nine-byte binary length-prefix classes (values from 2^32),
  which decoding already had. Same-size streams of large values -
  nanosecond timestamps, 64-bit IDs and hashes - encode 3.6-4.4x
  faster (nine-byte 3.35 -> 0.79 µs per 1,024 values on x86_64),
  where the specialized encoder previously trailed the generic one
  by ~1.4x; mixed wide sizes gain ~1.2x. Mixed-size `u64` streams of
  values below 2^28 measure ~9% slower, apparently from code layout:
  no window there reaches the new path. `u32` and `i32` are
  unaffected.
- The `alloc` `Vec` helpers now use the run fast paths for `u32`,
  `u64`, `i32`, and `i64`, and `bulk_encode_append` (which
  `bulk_encode_to_vec` now builds on) encodes in bounded chunks into
  worst-case room instead of first summing every value's size.
  Per 1,024 values on x86_64, `bulk_encode_append` is 2.4-5.4x faster
  on runs and 1.27x on random sizes, `bulk_encode_to_vec` 2.2-4.2x
  and 1.15x, and `bulk_decode_values` 1.5-2.8x, including 2.0x on
  random sizes. Types whose `MAX_ENCODED_SIZE` exceeds 4 KiB keep the
  exact sizing pass.

## 0.4.6

### Fixed

- The README benchmark chart now uses absolute image URLs: crates.io
  rewrites a relative `<img src>` but not `<source srcset>`, so the
  dark-mode chart resolved against crates.io itself and the image
  broke there.

## 0.4.5

### Added

- `decode_exact`, `decode_canonical`, and `decode_strict`: additive,
  allocation-free validation for whole-slice consumption and canonical
  encodings, extensible through `Encode::is_canonical_encoding`, with a
  separate non-exhaustive `StrictError` that preserves the v0.4 `Error`
  contract.
- `Reader::read_canonical` and `Reader::finish`: transactional canonical
  field decoding and explicit whole-message consumption checks for mixed-type
  protocols.
- Hostile-stream fuzz targets, big-endian s390x test coverage, and semver
  compatibility checks in CI.
- A runnable write-ahead-log example covering framing, delta encoding,
  torn-tail crash recovery, and the bulk paths.

### Fixed

- Decoding iterators no longer overstate their `size_hint` lower bound when
  malformed input can produce an immediate terminal error.
- `encode_append` now honors conforming downstream `Encode` implementations
  larger than the built-in 17-byte maximum while retaining the stack-buffer
  fast path for built-in types.
- The `u8` and `u16` decoders now handle valid short binary-prefix over-long
  encodings consistently, without reporting that they consumed beyond the
  supplied slice. Those uncommon forms stay on cold paths so ordinary
  prefix-varint decoding retains its prior hot-path performance.
- Canonical decoding now rejects same-length binary-prefix aliases below
  `2^28`, ensuring each built-in value has one accepted canonical byte
  representation.

## 0.4.4

### Added

- `Writer` and `Reader`: sequential cursors over byte slices for
  encoding and decoding mixed-type messages without manual offset
  bookkeeping (`no_std`, zero-cost).
- `Encode`/`Decode` for `u8`, `i8`, `usize`, and `isize`; `u8`/`i8` also
  gain array-based `const fn` codecs. `u8`/`i8` share the `u16` grammar;
  `usize`/`isize` share `u64`/`i64`, so the wire format stays identical
  across platforms (with `Overflow` on narrower targets).
- `encode_append` and `bulk_encode_append`: append encodings to an
  existing `Vec<u8>` without the per-value allocation of
  `encode_to_vec`.
- `#[serde(with = "vlen::serde::u32")]`-style modules for every
  supported type, so plain fields can use vlen encoding without
  wrapper types, plus `VlenU8`/`VlenI8`/`VlenUsize`/`VlenIsize`
  wrappers.

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
