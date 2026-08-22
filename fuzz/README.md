# Fuzzing vlen

The fuzz package keeps libFuzzer and its `std`-only dependencies outside the
published crate. Install the pinned tool used in CI, then run either target:

```sh
cargo install cargo-fuzz --version 0.13.2 --locked
cargo +nightly fuzz run decode_arbitrary
cargo +nightly fuzz run serde_arbitrary
```

The default fuzz build enables `vlen`'s native SIMD feature. Exercise the
portable paths with:

```sh
cargo +nightly fuzz run --no-default-features decode_arbitrary
```

Failures are written below `fuzz/artifacts/<target>/`. Minimize and replay a
finding with:

```sh
cargo +nightly fuzz tmin decode_arbitrary fuzz/artifacts/decode_arbitrary/<input>
cargo +nightly fuzz run decode_arbitrary fuzz/artifacts/decode_arbitrary/<input>
```

`decode_arbitrary` compares checked scalar, generic bulk, specialized bulk,
generic iterator, and run-accelerated iterator behavior over arbitrary byte
streams. `serde_arbitrary` pressures both human-readable and binary visitors.
