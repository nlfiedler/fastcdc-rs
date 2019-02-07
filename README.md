# FastCDC

This crate implements the "FastCDC" content defined chunking algorithm in pure
Rust. A critical aspect of its behavior is that it returns exactly the same
results for the same input. To learn more about content defined chunking and its
applications, see the reference material linked below.

## Requirements

* [Rust](https://www.rust-lang.org) stable (2018 edition)

## Building and Testing

```shell
$ cargo clean
$ cargo build
$ cargo test
```

## Example Usage

An example can be found in the `examples` directory of the source repository,
which demonstrates reading files of arbitrary size into a memory-mapped buffer
and passing them through the chunker (and computing the SHA256 hash digest of
each chunk).

The unit tests also have some short examples of using the chunker, of which this
code snippet is an example:

```rust
let read_result = fs::read("test/fixtures/SekienAkashita.jpg");
assert!(read_result.is_ok());
let contents = read_result.unwrap();
let chunker = FastCDC::new(&contents, 16384, 32768, 65536);
let results: Vec<Chunk> = chunker.collect();
assert_eq!(results.len(), 3);
assert_eq!(results[0].offset, 0);
assert_eq!(results[0].length, 32857);
assert_eq!(results[1].offset, 32857);
assert_eq!(results[1].length, 16408);
assert_eq!(results[2].offset, 49265);
assert_eq!(results[2].length, 60201);
```

## Reference Material

The algorithm is as described in "FastCDC: a Fast and Efficient Content-Defined
Chunking Approach for Data Deduplication"; see the
[paper](https://www.usenix.org/system/files/conference/atc16/atc16-paper-xia.pdf),
and
[presentation](https://www.usenix.org/sites/default/files/conference/protected-files/atc16_slides_xia.pdf)
for details.

## Prior Art

This crate is little more than a rewrite of the implementation by Joran Dirk
Greef (see the ronomon link below), in Rust, and greatly simplified in usage.
One significant difference is that the chunker in this crate does _not_
calculate a hash digest of the chunks.

* [ronomon/deduplication](https://github.com/ronomon/deduplication)
    + C++ and JavaScript implementation on which this code is based.
* [rdedup_cdc at docs.rs](https://docs.rs/crate/rdedup-cdc/0.1.0/source/src/fastcdc.rs)
    + An alternative implementation of FastCDC to the one in this crate.
* [jrobhoward/quickcdc](https://github.com/jrobhoward/quickcdc)
    + Similar but slightly earlier algorithm by some of the same researchers.
