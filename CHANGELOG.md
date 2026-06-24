# Change Log

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/).
This file follows the convention described at
[Keep a Changelog](http://keepachangelog.com/en/1.0.0/).

## [Unreleased]
### Performance
- **`v2020::cut_gear` inner loop: restored array-typed GEAR lookups.** The 4.0.0
  change from `&[u64; 256]` to `&[u64]` reintroduced a `panic_bounds_check` on
  every GEAR table lookup in the hot scan (4 of them per loop iteration).
  `cut_gear` now converts the tables to `&[u64; 256]` once via `try_into`, which
  the compiler can prove in-bounds for a `u8`-derived index. Cut points and
  hashes are unchanged (the existing fixture tests pin them); emitted asm drops
  from 8 to 4 `panic_bounds_check` sites in `cut_gear`, and an interleaved A/B
  measured ~7–14% throughput on random/text/zeros across chunk sizes (M1 Pro and
  a dedicated-CPU x86 VM). The `&[u64]`/`Cow` public signature is unchanged.
### Added
- **`v2020::FastCDC::rechunk`** — re-points an existing `FastCDC` at a new source
  and resets iteration, reusing the already-computed normalization masks and gear
  tables. The cheap way to chunk many in-memory buffers with identical parameters:
  avoids recomputing masks and (for a non-zero seed) re-allocating the gear tables
  on every `FastCDC::new`. The iterator already yields each chunk's offset/length
  without copying, so callers needing the chunk bytes can slice the source. Cut
  points are identical to a freshly constructed `FastCDC`.

## [4.0.1] - 2026-04-26
### Fixed
- Restore rounded-log mask selection. The 4.0.0 cleanup replaced the private
  `logarithm2()` helper (rounded `log2`) with `usize::ilog2()` (floored `log2`),
  which silently changed cut points for any `avg_size` that is not a power of
  two. Power-of-two sizes were unaffected. Cut points now match 3.2.1 again.

## [4.0.0] - 2026-04-11

Many changes suggested by Claude Code that seem worth making despite breaking the API. The changes needed are minor, just changing `u32` to `usize` for the common use case.

### Breaking Changes
**Size parameter types changed from `u32` to `usize`** across all three modules (`v2016`, `v2020`, `ronomon`):
- All public constructors: `new()`, `with_level()`, `with_level_and_seed()` for `FastCDC`, `StreamCDC`, and `AsyncStreamCDC`
- Public constants `MINIMUM_MIN`, `MINIMUM_MAX`, `AVERAGE_MIN`, `AVERAGE_MAX`, `MAXIMUM_MIN`, `MAXIMUM_MAX` are now `usize` instead of `u32`
- `cut_gear()` gear parameters changed from `&[u64; 256]` to `&[u64]`
- `get_gear_with_seed()` return type changed from `(Box<[u64; 256]>, Box<[u64; 256]>)` to `(Cow<'static, [u64]>, Cow<'static, [u64]>)`
- `Error::Display` output format changed (e.g. `"chunker error: Empty"` → `"no more data"`)
- Bounds checks on chunk size parameters changed from `assert!()` to `debug_assert!()`, meaning invalid sizes will no longer panic in release builds
### Minor Changes
- `MASKS` constant in `v2016` is now `pub` (was private)
- `Normalization` enum in both `v2016` and `v2020` now derives `Eq` and `PartialEq`
- `Normalization::bits()` in `v2016` is now `pub` (was private), and got a doc comment in `v2020`
### Bug Fixes & Performance Improvements
- **`size_hint()` corrected** in all three iterators: the lower bound was incorrectly returning the upper bound; now returns `1.min(upper_bound)`, which is semantically correct
- **Buffer extraction optimized** in `StreamCDC` and `AsyncStreamCDC`: replaced `drain(..).collect()` + `resize()` with `extend_from_slice()` + `copy_within()`, avoiding unnecessary reallocation
- **`get_gear_with_seed()` optimized**: when `seed == 0`, the static GEAR tables are borrowed directly via `Cow::Borrowed` instead of heap-allocating a copy
- **`mask()` in `ronomon`** changed from `2u32.pow(bits) - 1` to `(1u32 << bits) - 1` (equivalent but avoids potential debug-mode panics on overflow)

## [3.2.1] - 2025-04-17
### Fixed
- bits0rcerer: make `get_gear_with_seed()` public so it is usable outside of
  the `v2020` module.
- bits0rcerer: pass GEAR tables by reference to avoid copying.
- Restore the original `cut()` function and add `cut_gear()` with the references
  to the GEAR tables.

## [3.2.0] - 2025-04-17
### Added
- omgold: add `with_level_and_seed()` to the v2020 FastCDC implementations.
  This modifies the GEAR hash to prevent attacks that involve inferring
  information about the data based on the result of chunking.
### Changed
- **BREAKING**: the `cut()` function was mistakenly changed, fixed in 3.2.1.

## [3.1.0] - 2023-07-15
### Added
- mzr: add `AsyncStreamCDC` for asynchronous streaming support
- cdata: add features `futures` and `tokio` to enable `AsyncStreamCDC`

## [3.0.3] - 2023-03-29
### Changed
- LokyinZHAO: fix: `size_hint()` returns estimated number of remaining chunks

## [3.0.2] - 2023-03-10
### Changed
- **Breaking:** Removed unnecessary use of `Box` from `StreamCDC` in `v2016` and `v2020`.
- *Should have made this version to be **4.0** but failed to notice the breaking change until after releasing two more updates.*

## [3.0.1] - 2023-02-28
### Added
- nagy: support conversion to `std::io::Error` in streaming chunkers
### Fixed
- ariel-miculas: doc: fix year in `fastcdc::v2020`

## [3.0.0] - 2023-01-26
### Changed
- **Breaking:** moved ronomon FastCDC implementation into `ronomon` module.
  What was `fastcdc::FastCDC::new()` is now `fastcdc::ronomon::FastCDC::new()`.
- flokli: remove `mut` from `&self` in `cut()` as it does not need to be mutable.
### Added
- Canonical implementation of FastCDC from 2016 paper in `v2016` module.
- Canonical implementation of FastCDC from 2020 paper in `v2020` module.
- `Normalization` enum to set the normalized chunking for `v2016` and `v2020` chunkers.
- `StreamCDC`, streaming version of `FastCDC`, in `v2016` and `v2020` modules.

## [2.0.0] - 2023-01-14
### Added
- **Breaking:** dristic: expose hash on chunk struct.

## [1.0.8] - 2023-01-12
### Added
- revert breaking change "expose hash on chunk struct."

## [1.0.7] - 2023-01-11
### Added
- dristic: expose hash on chunk struct.

## [1.0.6] - 2021-01-08
### Added
- rickvanprim: implement `size_hint()` for FastCDC struct.

## [1.0.5] - 2020-07-22
### Added
- Smoozilla: add `with_eof()` constructor for streaming input data.

## [1.0.4] - 2020-07-08
### Added
- aikorsky: add basic derives for `Chunk` and `FastCDC` structs.

## [1.0.3] - 2020-03-18
### Changed
- snsmac: moved the generated table of numbers into the source code,
  significantly speeding up the build.

## [1.0.2] - 2019-08-05
### Fixed
- maobaolong: fixed logic for ceiling division function; results of
  chunking remain unchanged, so no visible difference.

## [1.0.1] - 2019-02-08
### Changed
- Add `build.rs` to generate the `table.rs` at build time, as needed.
- Add more documentation and a brief example of using `FastCDC`.

## [1.0.0] - 2019-02-06
### Added
- Added an example that processes files and computes SHA256 checksums.
- Added API documentation.

## [0.1.0] - 2019-02-05
### Changed
- Initial release
