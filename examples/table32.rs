//
// Copyright (c) 2023 Nathan Fiedler
//
use aes::cipher::{generic_array::GenericArray, KeyIvInit, StreamCipher};
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

type Aes256Ctr32BE = ctr::Ctr32BE<aes::Aes256>;

const MAX_VALUE: u32 = 2_147_483_648;

///
/// Produce a table of 256 predictable "random" 32-bit integers with the
/// high-bit cleared to produce 31-bit integers "to avoid 64 bit integers for
/// the sake of the JavaScript reference implementation" -- Joran Greef. See
/// https://github.com/ronomon/deduplication for a longer explanation.
///
fn generate() -> String {
    // Cleverly make "random" numbers by ciphering all zeros using a key and
    // nonce (a.k.a. initialization vector) of all zeroes. This is effectively
    // noise, but it is predictable noise, so the results are always the same.
    let mut table = [0u8; 1024];
    let key = GenericArray::from([0u8; 32]);
    let nonce = GenericArray::from([0u8; 16]);
    let mut cipher = Aes256Ctr32BE::new(&key, &nonce);
    cipher.apply_keystream(&mut table);
    let mut result = String::new();
    // the formatting is not pretty, but it compiles
    result.push_str("const TABLE: [u32; 256] = [\n");
    let mut rdr = Cursor::new(&table[..]);
    for index in 1..257 {
        let mut num: u32 = rdr.read_u32::<BigEndian>().unwrap();
        num = num % MAX_VALUE;
        result.push_str(&format!(" {:#010x},", num));
        if index % 8 == 0 {
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
    println!("{}", table);
}
