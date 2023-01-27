//
// Copyright (c) 2023 Nathan Fiedler
//

//! This crate implements multiple versions of the FastCDC content defined
//! chunking algorithm in pure Rust. A critical aspect of the behavior of this
//! algorithm is that it returns exactly the same results for the same input.
//!
//! To learn more about content defined chunking and its applications, see
//! [FastCDC: a Fast and Efficient Content-Defined Chunking Approach for Data
//! Deduplication](https://www.usenix.org/system/files/conference/atc16/atc16-paper-xia.pdf)
//! from 2016, as well as the subsequent improvements described in the
//! [paper](https://ieeexplore.ieee.org/document/9055082) from 2020
//!
//! ## Migration from pre-3.0
//!
//! If you were using a release of this crate from before the 3.0 release, you
//! will need to make a small adjustment to continue using the same
//! implemetation as before.
//!
//! Before the 3.0 release:
//!
//! ```no_run
//! # use fastcdc::ronomon as fastcdc;
//! # use std::fs;
//! # let contents = fs::read("test/fixtures/SekienAkashita.jpg").unwrap();
//! let chunker = fastcdc::FastCDC::new(&contents, 8192, 16384, 32768);
//! ```
//!
//! After the 3.0 release:
//!
//! ```no_run
//! # use std::fs;
//! # let contents = fs::read("test/fixtures/SekienAkashita.jpg").unwrap();
//! let chunker = fastcdc::ronomon::FastCDC::new(&contents, 8192, 16384, 32768);
//! ```
//!
//! The cut points produced will be identical to previous releases as the
//! `ronomon` implementation was never changed in that manner. Note, however,
//! that the other implementations _will_ produce different results.
//!
//! ## Implementations
//!
//! This crate had started as a translation of a variation of FastCDC
//! implemented in the
//! [ronomon/deduplication](https://github.com/ronomon/deduplication)
//! repository, written by Joran Dirk Greef. That variation makes several
//! changes to the original algorithm, primarily to accomodate JavaScript. The
//! Rust version of this variation is found in the `ronomon` module in this
//! crate.
//!
//! For a canonical implementation of the algorithm as described in the 2016
//! paper, see the `v2016` crate.
//!
//! For a canonical implementation of the algorithm as described in the 2020
//! paper, see the `v2020` crate. This implementation produces identical cut
//! points as the 2016 version, but does so a bit faster.
//!
//! If you are using this crate for the first time, the `v2020` implementation
//! would be the most approprite. It uses 64-bit hash values and tends to be
//! faster than both the `ronomon` and `v2016` versions.
//!
//! ## Examples
//!
//! A short example of using the fast chunker is shown below:
//!
//! ```no_run
//! use std::fs;
//! use fastcdc::v2020;
//! let contents = fs::read("test/fixtures/SekienAkashita.jpg").unwrap();
//! let chunker = v2020::FastCDC::new(&contents, 4096, 16384, 65535);
//! for entry in chunker {
//!     println!("offset={} size={}", entry.offset, entry.length);
//! }
//! ```
//!
//! The example above is using normalized chunking level 1 as described in
//! section 3.5 of the 2020 paper. To use a different level of chunking
//! normalization, replace `new` with `with_level` as shown below:
//!
//! ```no_run
//! use std::fs;
//! use fastcdc::v2020::{FastCDC, Normalization};
//! let contents = fs::read("test/fixtures/SekienAkashita.jpg").unwrap();
//! let chunker = FastCDC::with_level(&contents, 8192, 16384, 32768, Normalization::Level3);
//! for entry in chunker {
//!     println!("offset={} size={}", entry.offset, entry.length);
//! }
//! ```
//!
//! Notice that the minimum and maximum chunk sizes were changed in the example
//! using the maximum normalized chunking level. This is due to the behavior of
//! normalized chunking in which the generated chunks tend to be closer to the
//! expected chunk size. It is not necessary to change the min/max values, just
//! something of which to be aware. With lower levels of normalized chunking,
//! the size of the generated chunks will vary more. See the documentation of
//! the `Normalization` enum for more detail as well as the FastCDC paper.
//!
//! ## Minimum and Maximum
//!
//! The values you choose for the minimum and maximum chunk sizes will depend on
//! the input data to some extent, as well as the normalization level described
//! above. Depending on your application, you may want to have a wide range of
//! chunk sizes in order to improve the overall deduplication ratio.
//!
//! Note that changing the minimum chunk size will almost certainly result in
//! different cut points. It is best to pick a minimum chunk size for your
//! application that can remain relevant indefinitely, lest you produce
//! different sets of chunks for the same data.
//!
//! Similarly, setting the maximum chunk size to be too small may result in cut
//! points that were determined by the maximum size rather than the data itself.
//! Ideally you want cut points that are determined by the input data. However,
//! this is application dependent and your situation may be different.
//!
//! ## Large Data
//!
//! If processing very large files, the streaming version of the chunkers in the
//! `v2016` and `v2020` modules may be a suitable approach. They both allocate a
//! byte vector equal to the maximum chunk size, draining and resizing the
//! vector as chunks are found. However, using a crate such as `memmap2` can be
//! significantly faster than the streaming chunkers. See the examples in the
//! `examples` directory for how to use the streaming versions as-is, versus the
//! non-streaming chunkers which read from a memory-mapped file.

pub mod ronomon;
pub mod v2016;
pub mod v2020;
