# Change Log

All notable changes to this project will be documented in this file.
This project adheres to [Semantic Versioning](http://semver.org/).
This file follows the convention described at
[Keep a Changelog](http://keepachangelog.com/en/1.0.0/).

## [Unreleased]
### Changed
- **Breaking:** moved ronomon FastCDC implementation into `chunker::ronomon` module.
### Added
- Canonical implementation of FastCDC from 2016 paper in `chunker::v2016` module.
- Canonical implementation of FastCDC from 2020 paper in `chunker::v2020` module.

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
