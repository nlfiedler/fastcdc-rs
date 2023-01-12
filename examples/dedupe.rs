//
// Copyright (c) 2023 Nathan Fiedler
//
use clap::{arg, command, value_parser, Arg};
use crypto_hash::{hex_digest, Algorithm};
use fastcdc::*;
use memmap::MmapOptions;
use std::fs::File;

fn main() {
    let matches = command!("Example of using fastcdc crate.")
        .about("Splits a (large) file and computes checksums.")
        .arg(
            arg!(
                -s --size <SIZE> "The desired average size of the chunks."
            )
            .value_parser(value_parser!(u64)),
        )
        .arg(
            Arg::new("INPUT")
                .help("Sets the input file to use")
                .required(true)
                .index(1),
        )
        .get_matches();
    let size = matches.get_one::<u64>("size").unwrap_or(&131072);
    let avg_size = *size as usize;
    let filename = matches.get_one::<String>("INPUT").unwrap();
    let file = File::open(filename).expect("cannot open file!");
    let mmap = unsafe { MmapOptions::new().map(&file).expect("cannot create mmap?") };
    let min_size = avg_size / 2;
    let max_size = avg_size * 2;
    let chunker = FastCDC::new(&mmap[..], min_size, avg_size, max_size);
    for entry in chunker {
        let end = entry.offset + entry.length;
        let digest = hex_digest(Algorithm::SHA256, &mmap[entry.offset..end]);
        println!(
            "hash={} offset={} size={}",
            digest, entry.offset, entry.length
        );
    }
}
