//
// Copyright (c) 2023 Nathan Fiedler
//
use clap::{arg, command, value_parser, Arg};
use crypto_hash::{hex_digest, Algorithm};
use fastcdc::chunker::v2016::*;
use memmap::MmapOptions;
use std::fs::File;

fn main() {
    let matches = command!("Example of using fastcdc crate.")
        .about("Splits a (large) file and computes checksums.")
        .arg(
            arg!(
                -s --size <SIZE> "The desired average size of the chunks."
            )
            .value_parser(value_parser!(u32)),
        )
        .arg(
            Arg::new("INPUT")
                .help("Sets the input file to use")
                .required(true)
                .index(1),
        )
        .get_matches();
    let size = matches.get_one::<u32>("size").unwrap_or(&131072);
    let avg_size = *size;
    let filename = matches.get_one::<String>("INPUT").unwrap();
    let file = File::open(filename).expect("cannot open file!");
    let mmap = unsafe { MmapOptions::new().map(&file).expect("cannot create mmap?") };
    let min_size = avg_size / 2;
    let max_size = avg_size * 2;
    let chunker = FastCDC::new(&mmap[..], min_size, avg_size, max_size);
    for entry in chunker {
        let offset: usize = entry.offset as usize;
        let end: usize = offset + entry.length as usize;
        let digest = hex_digest(Algorithm::SHA256, &mmap[offset..end]);
        println!(
            "hash={} offset={} size={}",
            digest, entry.offset, entry.length
        );
    }
}
