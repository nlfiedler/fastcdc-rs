# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```shell
# Run tests for a specific module or single test by name
cargo test v2020
cargo test test_cut_sekien_16k_chunks

# Run tests with the async feature flags (mutually exclusive, see below)
cargo test --features tokio
cargo test --features futures

# Run an example
cargo run --example v2020 -- --size 16384 test/fixtures/SekienAkashita.jpg
cargo run --example stream2020 -- --size 16384 test/fixtures/SekienAkashita.jpg
cargo run --example async2020 --features tokio -- --size 16384 test/fixtures/SekienAkashita.jpg
```

To regenerate the GEAR hash tables, use the `regenerate-gear-tables` skill.

## Architecture

This crate provides three independent implementations of the FastCDC content defined chunking algorithm, each in its own module under `src/`:

- **`ronomon`** — A variation of FastCDC from the [ronomon/deduplication](https://github.com/ronomon/deduplication) JS/C++ implementation. Uses 31-bit integers and right shifts. This is the legacy implementation; pre-3.0 users should migrate to this module. Returns `u32` hashes.

- **`v2016`** — Implementation from the 2016 FastCDC paper. Uses 64-bit Gear hashes, sub-minimum chunk cut-point skipping, and normalized chunking. Non-streaming only (`FastCDC`) plus a streaming variant (`StreamCDC`).

- **`v2020`** — Implementation from the 2020 paper. Same cut points as `v2016` but ~20% faster due to "rolling two bytes each time." Recommended for new users. Provides:
  - `FastCDC` — in-memory chunker, implements `Iterator<Item = Chunk>`
  - `StreamCDC` — reads from `Read`, implements `Iterator<Item = Result<ChunkData, Error>>`
  - `AsyncStreamCDC` — reads from `AsyncRead`, enabled by `tokio` or `futures` feature flags; produces a `Stream` via `.as_stream()`

### Gear tables

The GEAR hash tables in `v2016` and `v2020` are identical 256-entry `[u64; 256]` arrays computed from MD5 digests of byte values 0–255. The `v2020` module also maintains a left-shifted twin (`GEAR_LS`) for the "rolling two bytes" optimization.

### Normalization

`v2016` and `v2020` expose a `Normalization` enum (Level0–Level3). Level1 is the default. Higher levels produce chunks closer to the target average size by using different mask bit widths (`mask_s` for the first half, `mask_l` for the second half). The mask values come from the `MASKS` constant array indexed by `avg_size.log2().round() ± normalization_bits` (the private `logarithm2` helper) — a *rounded* log2, not `usize::ilog2()`'s floor, so a non-power-of-two `avg_size` like `12288` picks the same bucket as `16384` rather than `8192`.

### Seeding

`v2020` supports an optional `seed: u64` via `with_level_and_seed`. A non-zero seed is XOR'd into the GEAR and GEAR_LS tables to shift chunk boundaries, useful for preventing deduplication inference attacks. Zero seed borrows the static tables without allocation.

### Feature flags

`tokio` and `futures` both enable `AsyncStreamCDC` (backed by their respective `AsyncRead` traits), but are mutually exclusive in the conditional compilation guards (`#[cfg(all(feature = "tokio", not(feature = "futures")))]`) — enabling both does not give you a choice at runtime.

### Test fixture

All tests use `test/fixtures/SekienAkashita.jpg`. The expected chunk hashes and lengths are hardcoded in tests — any algorithm change that alters cut points will break these tests by design, as deterministic output is a core guarantee of the crate.
