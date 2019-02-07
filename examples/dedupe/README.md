# FastCDC Example: dedupe

This example demonstrates the use of the `fastcdc` crate by accepting a file on
the command line, finding the content dependent chunk boundaries, computing the
SHA256 hash digest of each chunk, and printing the results.

## Requirements

* [Rust](https://www.rust-lang.org) stable (2018 edition)

## Running

Build and run the optimized version so the chunker performs a bit faster.

```shell
$ cargo run --release -- --help
```

When given a file as an argument, the demo code will split the file into chunks
roughly between 64KB and 256KB in size and output the SHA256 checksum of each
chunk. While the results may differ slightly from the
[ronomon/deduplication](https://github.com/ronomon/deduplication) demo code,
they are mostly the same, and consistent from one run to the next.

The demo will work on files of any size, theoretically, and was tested on files
consisting of multiple gigabytes, with a desired chunk size of 4MB.
