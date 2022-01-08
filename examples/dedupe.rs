//
// Copyright (c) 2020 Nathan Fiedler
//
use clap::{App, Arg};
use crypto_hash::{hex_digest, Algorithm};
use fastcdc::*;
use memmap::MmapOptions;
use std::fs::File;
use std::str::FromStr;

fn main() {
    fn is_integer(v: &str) -> Result<(), String> {
        if u64::from_str(&v).is_ok() {
            return Ok(());
        }
        Err(String::from(
            "The size must be a valid unsigned 64-bit integer.",
        ))
    }
    let matches = App::new("Example of using fastcdc crate.")
        .about("Splits a (large) file and computes checksums.")
        .arg(
            Arg::new("size")
                .short('s')
                .long("size")
                .value_name("SIZE")
                .help("The desired average size of the chunks.")
                .takes_value(true)
                .validator(is_integer),
        )
        .arg(
            Arg::new("INPUT")
                .help("Sets the input file to use")
                .required(true)
                .index(1),
        )
        .get_matches();
    let size = matches.value_of("size").unwrap_or("131072");
    let avg_size = u64::from_str(size).unwrap() as usize;
    let filename = matches.value_of("INPUT").unwrap();
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
