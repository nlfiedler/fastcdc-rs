# Change Log

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/).
This file follows the convention described at
[Keep a Changelog](http://keepachangelog.com/en/1.0.0/).

## [3.2.0] - 2025-04-17
### Added
- omgold: add `with_level_and_seed()` to the v2020 FastCDC implementations.
  This modifies the GEAR hash to prevent attacks that involve inferring
  information about the data based on the result of chunking.

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
