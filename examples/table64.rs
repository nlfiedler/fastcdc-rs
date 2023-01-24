//
// Copyright (c) 2023 Nathan Fiedler
//
use byteorder::{BigEndian, ReadBytesExt};
use md5::{Digest, Md5};
use std::io::Cursor;

///
/// Produce a table of 256 predictable "random" 64-bit integers. This uses the
/// same approach as the C reference implementation, producing an MD5 hash of
/// seed values increasing from 0 to 255.
///
/// C implementation:
/// https://github.com/Borelset/destor/blob/master/src/chunking/fascdc_chunking.c
///
fn generate() -> String {
    let mut result = String::new();
    result.push_str("const TABLE: [u64; 256] = [\n");
    let mut seed = [0u8; 64];
    for index in 0..=255 {
        seed.fill(index);
        let mut hasher = Md5::new();
        hasher.update(&seed);
        let table = hasher.finalize();
        let mut rdr = Cursor::new(&table[..]);
        let num: u64 = rdr.read_u64::<BigEndian>().unwrap();
        result.push_str(&format!(" {:#018x},", num));
        if index % 4 == 3 {
            result.push('\n');
        }
    }
    // remove the trailing comma (and final newline)
    result.truncate(result.len() - 2);
    result.push_str("\n];\n");
    result
}

fn main() {
    let table = generate();
    print!("{}", table);
}
