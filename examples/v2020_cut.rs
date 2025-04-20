//
// Copyright (c) 2025 Nathan Fiedler
//
use clap::{arg, command, value_parser, Arg};
use fastcdc::v2020::*;
use memmap2::Mmap;
use std::fs::File;

fn main() {
    let matches = command!("Example of using v2020 chunker cut() function.")
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
    let min_size: usize = avg_size as usize / 4;
    let max_size: usize = avg_size as usize * 4;

    // A bunch of extra setup that FastCDC would normally do for us. This is a
    // lot of work compared to the v2020 example, but the point is to make sure
    // the cut() function remains usable despite other API changes. The output
    // of this example should be identical to the v2020 example.
    let bits = logarithm2(avg_size);
    let level = Normalization::Level1;
    let normalization = level.bits();
    let mask_s = MASKS[(bits + normalization) as usize];
    let mask_l = MASKS[(bits - normalization) as usize];
    let mask_s_ls = mask_s << 1;
    let mask_l_ls = mask_l << 1;
    let mut processed: usize = 0;
    let mut remaining: usize = mmap.len();
    while remaining > 0 {
        let end = processed + remaining;
        let (hash, count) = cut(
            &mmap[processed..end],
            min_size,
            avg_size as usize,
            max_size,
            mask_s,
            mask_l,
            mask_s_ls,
            mask_l_ls,
        );
        let cutpoint = processed + count;
        if cutpoint == 0 {
            break;
        } else {
            let offset = processed;
            let length = cutpoint - offset;
            processed += length;
            remaining -= length;
            println!("hash={} offset={} size={}", hash, offset, length);
        }
    }
}
