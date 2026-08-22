//
// Copyright (c) 2023 Nathan Fiedler
//
//! Run with:
//!
//! ```shell
//! cargo run --example stream2020 -- --size 16384 test/fixtures/SekienAkashita.jpg
//! ```
use clap::{Arg, arg, command, value_parser};
use fastcdc::v2020::*;
use std::fs::File;

fn main() {
    let matches = command!("Example of using v2020 streaming chunker.")
        .about("Finds the content-defined chunk boundaries of a file.")
        .arg(
            arg!(
                -s --size <SIZE> "The desired average size of the chunks."
            )
            .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("INPUT")
                .help("Sets the input file to use")
                .required(true)
                .index(1),
        )
        .get_matches();
    let size = matches.get_one::<usize>("size").unwrap_or(&131072);
    // v2020 requires even min/avg/max sizes; round down to a multiple of 8 so
    // avg_size, avg_size / 4, and avg_size * 4 all come out even.
    let avg_size = *size & !7;
    let filename = matches.get_one::<String>("INPUT").unwrap();
    let file = File::open(filename).expect("cannot open file!");
    let min_size = avg_size / 4;
    let max_size = avg_size * 4;
    let chunker = StreamCDC::new(file, min_size, avg_size, max_size);
    for result in chunker {
        let entry = result.expect("failed to read chunk");
        println!(
            "hash={} offset={} size={}",
            entry.hash, entry.offset, entry.length
        );
    }
}
