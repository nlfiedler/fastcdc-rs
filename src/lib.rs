//
// Copyright (c) 2020 Nathan Fiedler
//

//! This crate implements the "FastCDC" content defined chunking algorithm in
//! pure Rust. A critical aspect of its behavior is that it returns exactly the
//! same results for the same input. To learn more about content defined
//! chunking and its applications, see "FastCDC: a Fast and Efficient
//! Content-Defined Chunking Approach for Data Deduplication"
//! ([paper](https://www.usenix.org/system/files/conference/atc16/atc16-paper-xia.pdf)
//! and
//! [presentation](https://www.usenix.org/sites/default/files/conference/protected-files/atc16_slides_xia.pdf))

pub mod chunker;
