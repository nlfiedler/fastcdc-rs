//
// Copyright (c) 2019 Nathan Fiedler
//
use std::env;
use std::fs::File;
use std::io::Cursor;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use aes_ctr::Aes256Ctr;
use aes_ctr::stream_cipher::generic_array::GenericArray;
use aes_ctr::stream_cipher::{
    NewStreamCipher, SyncStreamCipher
};
use byteorder::{BigEndian, ReadBytesExt};

// 31-bit integers are immune from signed 32-bit integer overflow, which the
// current implementation relies on for hashing.
const MAX_VALUE: u32 = 2_147_483_648;

fn generate() -> String {
    // Cleverly make "random" numbers by ciphering all zeros using a key and
    // nonce (a.k.a. initialization vector) of all zeroes. This is effectively
    // noise, but it is predictable noise, so the results are always the same.
    // Either we do this at build time or we have a file containing "magic"
    // numbers that no one knows how to generate when needed.
    let mut table = [0u8; 1024];
    let key = GenericArray::from([0u8; 32]);
    let nonce = GenericArray::from([0u8; 16]);
    let mut cipher = Aes256Ctr::new(&key, &nonce);
    cipher.apply_keystream(&mut table[..]);
    let mut result = String::new();
    // The formatting is not pretty, but it compiles.
    result.push_str("const TABLE: [u32; 256] = [\n");
    let mut rdr = Cursor::new(&table[..]);
    for index in 1..257 {
        let mut num: u32 = rdr.read_u32::<BigEndian>().unwrap();
        num = num % MAX_VALUE;
        result.push_str(&format!("{},", num));
        if index % 6 == 0 {
            result.push('\n');
        } else {
            result.push(' ');
        }
    }
    result.truncate(result.len() - 2);
    result.push_str("\n];\n");
    result
}

fn main() {
    // If the generated "hash" table is missing, generate it now.
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let table_path_str = out_dir.join("table.rs");
    if !Path::new(&table_path_str).exists() {
        let table = generate();
        let mut file = File::create(&table_path_str).expect("could not create table.rs file");
        file.write(table.to_string().as_bytes()).unwrap();
    }
}
