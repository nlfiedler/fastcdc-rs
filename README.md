# FastCDC [![docs.rs](https://docs.rs/fastcdc/badge.svg)](https://docs.rs/fastcdc) [![Crates.io](https://img.shields.io/crates/v/fastcdc.svg)](https://crates.io/crates/fastcdc) ![Test](https://github.com/nlfiedler/fastcdc-rs/workflows/Test/badge.svg)

This crate contains multiple implementations of the "FastCDC" content defined chunking algorithm orginally described in 2016 by Wen Xia, et al. A critical aspect of its behavior is that it returns exactly the same results for the same input. To learn more about content defined chunking and its applications, see the reference material linked below.

## Requirements

* [Rust](https://www.rust-lang.org) stable (2018 edition)

## Building and Testing

```shell
$ cargo clean
$ cargo build
$ cargo test
```

## Example Usage

Examples can be found in the `examples` directory of the source repository,
which demonstrate reading files of arbitrary size into a memory-mapped buffer
and passing them through the different chunker implementations.

```shell
$ cargo run --example v2016 -- --size 16384 test/fixtures/SekienAkashita.jpg
    Finished dev [unoptimized + debuginfo] target(s) in 0.03s
     Running `target/debug/examples/v2016 --size 16384 test/fixtures/SekienAkashita.jpg`
hash=17968276318003433923 offset=0 size=21325
hash=4098594969649699419 offset=21325 size=17140
hash=15733367461443853673 offset=38465 size=28084
hash=4509236223063678303 offset=66549 size=18217
hash=2504464741100432583 offset=84766 size=24700
```

The unit tests also have some short examples of using the chunkers, of which this
code snippet is an example:

```rust
let read_result = fs::read("test/fixtures/SekienAkashita.jpg");
assert!(read_result.is_ok());
let contents = read_result.unwrap();
let chunker = fastcdc::v2016::FastCDC::new(&contents, 16384, 32768, 65536);
let results: Vec<Chunk> = chunker.collect();
assert_eq!(results.len(), 2);
assert_eq!(results[0].offset, 0);
assert_eq!(results[0].length, 66549);
assert_eq!(results[1].offset, 66549);
assert_eq!(results[1].length, 42917);
```

## Reference Material

The original algorithm is described in [FastCDC: a Fast and Efficient Content-Defined Chunking Approach for Data Deduplication](https://www.usenix.org/system/files/conference/atc16/atc16-paper-xia.pdf), while the improved "rolling two bytes each time" version is detailed in [The Design of Fast Content-Defined Chunking for Data Deduplication Based Storage Systems](https://ieeexplore.ieee.org/document/9055082).

## Other Implementations

* [jrobhoward/quickcdc](https://github.com/jrobhoward/quickcdc)
    + Similar but slightly earlier algorithm by some of the same researchers?
* [rdedup_cdc at docs.rs](https://docs.rs/crate/rdedup-cdc/0.1.0/source/src/fastcdc.rs)
    + Alternative implementation in Rust.
* [ronomon/deduplication](https://github.com/ronomon/deduplication)
    + C++ and JavaScript implementation of a variation of FastCDC.
* [titusz/fastcdc-py](https://github.com/titusz/fastcdc-py)
    + Pure Python port of FastCDC. Compatible with this implementation.
* [wxiacode/destor](https://github.com/wxiacode/destor/blob/master/src/chunking)
    + Canonical algorithm in C with gear table generation and mask values.
* [wxiacode/restic-FastCDC](https://github.com/wxiacode/restic-FastCDC)
    + Alternative implementation in Go with additional mask values.
