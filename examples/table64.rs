//
// Copyright (c) 2023 Nathan Fiedler
//
use aes::cipher::{generic_array::GenericArray, KeyIvInit, StreamCipher};
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

type Aes256Ctr64BE = ctr::Ctr64BE<aes::Aes256>;

///
/// Produce a table of 256 predictable "random" 64-bit integers. The C reference
/// implementation uses MD5 with seed values of 0 to 255, but that should not
/// matter for the purpose of the algorithm. The only important factor is that
/// we produce consistent values each time.
///
/// C implementation:
/// https://github.com/Borelset/destor/blob/master/src/chunking/fascdc_chunking.c
///
fn generate() -> String {
    // Cleverly make "random" numbers by ciphering all zeros using a key and
    // nonce (a.k.a. initialization vector) of all zeroes. This is effectively
    // noise, but it is predictable noise, so the results are always the same.
    let mut table = [0u8; 2048];
    let key = GenericArray::from([0u8; 32]);
    let nonce = GenericArray::from([0u8; 16]);
    let mut cipher = Aes256Ctr64BE::new(&key, &nonce);
    cipher.apply_keystream(&mut table);
    let mut result = String::new();
    // the formatting is not pretty, but it compiles
    result.push_str("const TABLE: [u64; 256] = [\n");
    let mut rdr = Cursor::new(&table[..]);
    for index in 1..257 {
        let num: u64 = rdr.read_u64::<BigEndian>().unwrap();
        result.push_str(&format!("{:#018x},", num));
        if index % 6 == 0 {
            result.push('\n');
        } else {
            result.push(' ');
        }
    }
    // remove the trailing comma from the last line
    result.truncate(result.len() - 2);
    result.push_str("\n];\n");
    result
}

fn main() {
    let table = generate();
    println!("{}", table);
}
