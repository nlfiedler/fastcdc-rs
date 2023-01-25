//
// Copyright (c) 2023 Nathan Fiedler
//
use byteorder::{BigEndian, ReadBytesExt};
use md5::{Digest, Md5};
use std::io::Cursor;

///
/// Produce a table of 256 predictable "random" 64-bit integers shifted left 1
/// bit. This uses the same approach as the C reference implementation,
/// producing an MD5 hash of seed values increasing from 0 to 255.
///
/// C implementation: https://github.com/wxiacode/FastCDC-c
///
fn main() {
    let mut table = String::new();
    table.push_str("const TABLE: [u64; 256] = [\n");
    let mut seed = [0u8; 64];
    for index in 0..=255 {
        seed.fill(index);
        let mut hasher = Md5::new();
        hasher.update(&seed);
        let digest = hasher.finalize();
        let mut rdr = Cursor::new(&digest[..]);
        let mut num: u64 = rdr.read_u64::<BigEndian>().unwrap();
        num = num << 1;
        table.push_str(&format!(" {:#018x},", num));
        if index % 4 == 3 {
            table.push('\n');
        }
    }
    // remove the trailing comma (and final newline)
    table.truncate(table.len() - 2);
    table.push_str("\n];\n");
    print!("{}", table);
}
