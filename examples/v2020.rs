//
// Copyright (c) 2023 Nathan Fiedler
//
use clap::{Arg, arg, command, value_parser};
use fastcdc::v2020::*;
use memmap2::Mmap;
use std::fs::File;

fn main() {
    let matches = command!("Example of using v2020 chunker.")
        .about("Finds the content-defined chunk boundaries of a file.")
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
    let mmap = unsafe { Mmap::map(&file).expect("cannot create mmap?") };
    let min_size = avg_size / 4;
    let max_size = avg_size * 4;
    let chunker = FastCDC::new(&mmap[..], min_size, avg_size, max_size);
    for entry in chunker {
        println!(
            "hash={} offset={} size={}",
            entry.hash, entry.offset, entry.length
        );
    }
}
