//
// Copyright (c) 2019 Nathan Fiedler
//

//! This crate implements the "FastCDC" content defined chunking algorithm in
//! pure Rust. A critical aspect of its behavior is that it returns exactly the
//! same results for the same input. To learn more about content defined
//! chunking and its applications, see "FastCDC: a Fast and Efficient
//! Content-Defined Chunking Approach for Data Deduplication"
//! ([paper](https://www.usenix.org/system/files/conference/atc16/atc16-paper-xia.pdf)
//! and
//! [presentation](https://www.usenix.org/sites/default/files/conference/protected-files/atc16_slides_xia.pdf))
//!
//! See the `FastCDC` struct for basic usage and an example.
//!
//! For a slightly more involved example, see the `examples` directory in the
//! source repository.

/// Smallest acceptable value for the minimum chunk size.
pub const MINIMUM_MIN: usize = 64;
/// Largest acceptable value for the minimum chunk size.
pub const MINIMUM_MAX: usize = 67_108_864;
/// Smallest acceptable value for the average chunk size.
pub const AVERAGE_MIN: usize = 256;
/// Largest acceptable value for the average chunk size.
pub const AVERAGE_MAX: usize = 268_435_456;
/// Smallest acceptable value for the maximum chunk size.
pub const MAXIMUM_MIN: usize = 1024;
/// Largest acceptable value for the maximum chunk size.
pub const MAXIMUM_MAX: usize = 1_073_741_824;

/// Represents a chunk, returned from the FastCDC iterator.
pub struct Chunk {
    /// Starting byte position within the original content.
    pub offset: usize,
    /// Length of the chunk in bytes.
    pub length: usize,
}

///
/// The FastCDC chunker implementation. Use `new` to construct a new instance,
/// and then iterate over the `Chunk`s via the `Iterator` trait.
///
/// This example reads a file into memory and splits it into chunks that are
/// somewhere between 16KB and 64KB, preferring something around 32KB.
///
/// ```no_run
/// let contents = std::fs::read("test/fixtures/SekienAkashita.jpg").unwrap();
/// let chunker = fastcdc::FastCDC::new(&contents, 16384, 32768, 65536);
/// for entry in chunker {
///     println!("offset={} size={}", entry.offset, entry.length);
/// }
/// ```
///
pub struct FastCDC<'a> {
    source: &'a [u8],
    bytes_processed: usize,
    bytes_remaining: usize,
    min_size: usize,
    avg_size: usize,
    max_size: usize,
    mask_s: u32,
    mask_l: u32,
}

impl<'a> FastCDC<'a> {
    ///
    /// Construct a new `FastCDC` that will process the given slice of bytes.
    /// The `min_size` specifies the preferred minimum chunk size, likewise for
    /// `max_size`; the `avg_size` is what the FastCDC paper refers to as the
    /// desired "normal size" of the chunks.
    ///
    pub fn new(source: &'a [u8], min_size: usize, avg_size: usize, max_size: usize) -> Self {
        assert!(min_size >= MINIMUM_MIN);
        assert!(min_size <= MINIMUM_MAX);
        assert!(avg_size >= AVERAGE_MIN);
        assert!(avg_size <= AVERAGE_MAX);
        assert!(max_size >= MAXIMUM_MIN);
        assert!(max_size <= MAXIMUM_MAX);
        let bits = logarithm2(avg_size as u32);
        let mask_s = mask(bits + 1);
        let mask_l = mask(bits - 1);
        Self {
            source,
            bytes_processed: 0,
            bytes_remaining: source.len(),
            min_size,
            avg_size,
            max_size,
            mask_s,
            mask_l,
        }
    }

    /// Returns the size of the next chunk.
    fn cut(&mut self, mut source_offset: usize, mut source_size: usize) -> usize {
        if source_size <= self.min_size {
            source_size
        } else {
            if source_size > self.max_size {
                source_size = self.max_size;
            }
            let source_start: usize = source_offset;
            let source_len1: usize =
                source_offset + center_size(self.avg_size, self.min_size, source_size);
            let source_len2: usize = source_offset + source_size;
            let mut hash: u32 = 0;
            source_offset += self.min_size;
            // Start by using the "harder" chunking judgement to find chunks
            // that run smaller than the desired normal size.
            while source_offset < source_len1 {
                let index = self.source[source_offset] as usize;
                source_offset += 1;
                hash = (hash >> 1) + TABLE[index];
                if (hash & self.mask_s) == 0 {
                    return source_offset - source_start;
                }
            }
            // Fall back to using the "easier" chunking judgement to find chunks
            // that run larger than the desired normal size.
            while source_offset < source_len2 {
                let index = self.source[source_offset] as usize;
                source_offset += 1;
                hash = (hash >> 1) + TABLE[index];
                if (hash & self.mask_l) == 0 {
                    return source_offset - source_start;
                }
            }
            // All else fails, return the whole chunk. This will happen with
            // pathological data, such as all zeroes.
            source_size
        }
    }
}

impl<'a> Iterator for FastCDC<'a> {
    type Item = Chunk;

    fn next(&mut self) -> Option<Chunk> {
        if self.bytes_remaining == 0 {
            None
        } else {
            let chunk_size = self.cut(self.bytes_processed, self.bytes_remaining);
            let chunk_start = self.bytes_processed;
            self.bytes_processed += chunk_size;
            self.bytes_remaining -= chunk_size;
            Some(Chunk {
                offset: chunk_start,
                length: chunk_size,
            })
        }
    }
}

///
/// Base-2 logarithm function for unsigned 32-bit integers.
///
fn logarithm2(value: u32) -> u32 {
    let fvalue: f64 = f64::from(value);
    let retval: u32 = fvalue.log2().round() as u32;
    retval
}

///
/// Integer division that rounds up instead of down.
///
fn ceil_div(x: usize, y: usize) -> usize {
    (x + y - 1) / y
}

///
/// Find the middle of the desired chunk size, or what the FastCDC paper refers
/// to as the "normal size".
///
fn center_size(average: usize, minimum: usize, source_size: usize) -> usize {
    let mut offset: usize = minimum + ceil_div(minimum, 2);
    if offset > average {
        offset = average;
    }
    let size: usize = average - offset;
    if size > source_size {
        source_size
    } else {
        size
    }
}

///
/// Returns two raised to the `bits` power, minus one. In other words, a bit
/// mask with that many least-significant bits set to 1.
///
fn mask(bits: u32) -> u32 {
    debug_assert!(bits >= 1);
    debug_assert!(bits <= 31);
    2u32.pow(bits) - 1
}

//
// TABLE contains seemingly "random" numbers which are created by ciphering a
// 1024-byte array of all zeros using a 32-byte key and 16-byte nonce (a.k.a.
// initialization vector) of all zeroes. The high bit of each value is cleared
// because 31-bit integers are immune from signed 32-bit integer overflow, which
// the implementation above relies on for hashing.
//
// While this may seem to be effectively noise, it is predictable noise, so the
// results are always the same. That is the most important aspect of the
// content-defined chunking algorithm, consistent results over time.
//
// The original build.rs script was removed in commit f001c11 and shows the
// exact implementation used to generate these "magic" numbers.
//
#[rustfmt::skip]
const TABLE: [u32; 256] = [
    0x5c95_c078, 0x2240_8989, 0x2d48_a214, 0x1284_2087, 0x530f_8afb, 0x4745_36b9,
    0x2963_b4f1, 0x44cb_738b, 0x4ea7_403d, 0x4d60_6b6e, 0x074e_c5d3, 0x3af3_9d18,
    0x7260_03ca, 0x37a6_2a74, 0x51a2_f58e, 0x7506_358e, 0x5d4a_b128, 0x4d4a_e17b,
    0x41e8_5924, 0x470c_36f7, 0x4741_cbe1, 0x01bb_7f30, 0x617c_1de3, 0x2b0c_3a1f,
    0x50c4_8f73, 0x21a8_2d37, 0x6095_ace0, 0x4191_67a0, 0x3caf_49b0, 0x40ce_a62d,
    0x66bc_1c66, 0x545e_1dad, 0x2bfa_77cd, 0x6e85_da24, 0x5fb0_bdc5, 0x652c_fc29,
    0x3a0a_e1ab, 0x2837_e0f3, 0x6387_b70e, 0x1317_6012, 0x4362_c2bb, 0x66d8_f4b1,
    0x37fc_e834, 0x2c9c_d386, 0x2114_4296, 0x6272_68a8, 0x650d_f537, 0x2805_d579,
    0x3b21_ebbd, 0x7357_ed34, 0x3f58_b583, 0x7150_ddca, 0x7362_225e, 0x620a_6070,
    0x2c5e_f529, 0x7b52_2466, 0x768b_78c0, 0x4b54_e51e, 0x75fa_07e5, 0x06a3_5fc6,
    0x30b7_1024, 0x1c86_26e1, 0x296a_d578, 0x28d7_be2e, 0x1490_a05a, 0x7cee_43bd,
    0x698b_56e3, 0x09dc_0126, 0x4ed6_df6e, 0x02c1_bfc7, 0x2a59_ad53, 0x29c0_e434,
    0x7d6c_5278, 0x5079_40a7, 0x5ef6_ba93, 0x68b6_af1e, 0x4653_7276, 0x611b_c766,
    0x155c_587d, 0x301b_a847, 0x2cc9_dda7, 0x0a43_8e2c, 0x0a69_d514, 0x744c_72d3,
    0x4f32_6b9b, 0x7ef3_4286, 0x4a0e_f8a7, 0x6ae0_6ebe, 0x669c_5372, 0x1240_2dcb,
    0x5fea_e99d, 0x76c7_f4a7, 0x6abd_b79c, 0x0dfa_a038, 0x20e2_282c, 0x730e_d48b,
    0x069d_ac2f, 0x168e_cf3e, 0x2610_e61f, 0x2c51_2c8e, 0x15fb_8c06, 0x5e62_bc76,
    0x6955_5135, 0x0adb_864c, 0x4268_f914, 0x349a_b3aa, 0x20ed_fdb2, 0x5172_7981,
    0x37b4_b3d8, 0x5dd1_7522, 0x6b2c_bfe4, 0x5c47_cf9f, 0x30fa_1ccd, 0x23de_db56,
    0x13d1_f50a, 0x64ed_dee7, 0x0820_b0f7, 0x46e0_7308, 0x1e2d_1dfd, 0x17b0_6c32,
    0x2500_36d8, 0x284d_bf34, 0x6829_2ee0, 0x362e_c87c, 0x087c_b1eb, 0x76b4_6720,
    0x1041_30db, 0x7196_6387, 0x482d_c43f, 0x2388_ef25, 0x5241_44e1, 0x44bd_834e,
    0x448e_7da3, 0x3fa6_eaf9, 0x3cda_215c, 0x3a50_0cf3, 0x395c_b432, 0x5195_129f,
    0x4394_5f87, 0x5186_2ca4, 0x56ea_8ff1, 0x2010_34dc, 0x4d32_8ff5, 0x7d73_a909,
    0x6234_d379, 0x64cf_bf9c, 0x36f6_589a, 0x0a2c_e98a, 0x5fe4_d971, 0x03bc_15c5,
    0x4402_1d33, 0x16c1_932b, 0x3750_3614, 0x1aca_f69d, 0x3f03_b779, 0x49e6_1a03,
    0x1f52_d7ea, 0x1c6d_dd5c, 0x0622_18ce, 0x07e7_a11a, 0x1905_757a, 0x7ce0_0a53,
    0x49f4_4f29, 0x4bcc_70b5, 0x39fe_ea55, 0x5242_cee8, 0x3ce5_6b85, 0x00b8_1672,
    0x46be_eccc, 0x3ca0_ad56, 0x2396_cee8, 0x7854_7f40, 0x6b08_089b, 0x66a5_6751,
    0x781e_7e46, 0x1e2c_f856, 0x3bc1_3591, 0x494a_4202, 0x5204_94d7, 0x2d87_459a,
    0x7575_55b6, 0x4228_4cc1, 0x1f47_8507, 0x75c9_5dff, 0x35ff_8dd7, 0x4e47_57ed,
    0x2e11_f88c, 0x5e1b_5048, 0x420e_6699, 0x226b_0695, 0x4d16_79b4, 0x5a22_646f,
    0x161d_1131, 0x125c_68d9, 0x1313_e32e, 0x4aa8_5724, 0x21dc_7ec1, 0x4ffa_29fe,
    0x7296_8382, 0x1ca8_eef3, 0x3f3b_1c28, 0x39c2_fb6c, 0x6d76_493f, 0x7a22_a62e,
    0x789b_1c2a, 0x16e0_cb53, 0x7dec_eeeb, 0x0dc7_e1c6, 0x5c75_bf3d, 0x5221_8333,
    0x106d_e4d6, 0x7dc6_4422, 0x6559_0ff4, 0x2c02_ec30, 0x64a9_ac67, 0x59ca_b2e9,
    0x4a21_d2f3, 0x0f61_6e57, 0x23b5_4ee8, 0x0273_0aaa, 0x2f3c_634d, 0x7117_fc6c,
    0x01ac_6f05, 0x5a9e_d20c, 0x158c_4e2a, 0x42b6_99f0, 0x0c7c_14b3, 0x02bd_9641,
    0x15ad_56fc, 0x1c72_2f60, 0x7da1_af91, 0x23e0_dbcb, 0x0e93_e12b, 0x64b2_791d,
    0x440d_2476, 0x588e_a8dd, 0x4665_a658, 0x7446_c418, 0x1877_a774, 0x5626_407e,
    0x7f63_bd46, 0x32d2_dbd8, 0x3c79_0f4a, 0x772b_7239, 0x6f8b_2826, 0x677f_f609,
    0x0dc8_2c11, 0x23ff_e354, 0x2eac_53a6, 0x1613_9e09, 0x0afd_0dbc, 0x2a4d_4237,
    0x56a3_68c7, 0x2343_25e4, 0x2dce_9187, 0x32e8_ea7e
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_logarithm2() {
        assert_eq!(logarithm2(65537), 16);
        assert_eq!(logarithm2(65536), 16);
        assert_eq!(logarithm2(65535), 16);
        assert_eq!(logarithm2(32769), 15);
        assert_eq!(logarithm2(32768), 15);
        assert_eq!(logarithm2(32767), 15);
        // test implementation assumptions
        assert!(logarithm2(AVERAGE_MIN as u32) >= 8);
        assert!(logarithm2(AVERAGE_MAX as u32) <= 28);
    }

    #[test]
    fn test_ceil_div() {
        assert_eq!(ceil_div(10, 5), 2);
        assert_eq!(ceil_div(11, 5), 3);
        assert_eq!(ceil_div(10, 3), 4);
        assert_eq!(ceil_div(9, 3), 3);
        assert_eq!(ceil_div(6, 2), 3);
        assert_eq!(ceil_div(5, 2), 3);
    }

    #[test]
    fn test_center_size() {
        assert_eq!(center_size(50, 100, 50), 0);
        assert_eq!(center_size(200, 100, 50), 50);
        assert_eq!(center_size(200, 100, 40), 40);
    }

    #[test]
    #[should_panic]
    fn test_mask_low() {
        mask(0);
    }

    #[test]
    #[should_panic]
    fn test_mask_high() {
        mask(32);
    }

    #[test]
    fn test_mask() {
        assert_eq!(mask(24), 16_777_215);
        assert_eq!(mask(16), 65535);
        assert_eq!(mask(10), 1023);
        assert_eq!(mask(8), 255);
    }

    #[test]
    #[should_panic]
    fn test_minimum_too_low() {
        let array = [0u8; 2048];
        FastCDC::new(&array, 63, 256, 1024);
    }

    #[test]
    #[should_panic]
    fn test_minimum_too_high() {
        let array = [0u8; 2048];
        FastCDC::new(&array, 67_108_867, 256, 1024);
    }

    #[test]
    #[should_panic]
    fn test_average_too_low() {
        let array = [0u8; 2048];
        FastCDC::new(&array, 64, 255, 1024);
    }

    #[test]
    #[should_panic]
    fn test_average_too_high() {
        let array = [0u8; 2048];
        FastCDC::new(&array, 64, 268_435_457, 1024);
    }

    #[test]
    #[should_panic]
    fn test_maximum_too_low() {
        let array = [0u8; 2048];
        FastCDC::new(&array, 64, 256, 1023);
    }

    #[test]
    #[should_panic]
    fn test_maximum_too_high() {
        let array = [0u8; 2048];
        FastCDC::new(&array, 64, 256, 1_073_741_825);
    }

    #[test]
    fn test_all_zeros() {
        // for all zeros, always returns chunks of maximum size
        let array = [0u8; 10240];
        let chunker = FastCDC::new(&array, 64, 256, 1024);
        let results: Vec<Chunk> = chunker.collect();
        assert_eq!(results.len(), 10);
        for entry in results {
            assert_eq!(entry.offset % 1024, 0);
            assert_eq!(entry.length, 1024);
        }
    }

    #[test]
    fn test_sekien_16k_chunks() {
        let read_result = fs::read("test/fixtures/SekienAkashita.jpg");
        assert!(read_result.is_ok());
        let contents = read_result.unwrap();
        let chunker = FastCDC::new(&contents, 8192, 16384, 32768);
        let results: Vec<Chunk> = chunker.collect();
        assert_eq!(results.len(), 6);
        assert_eq!(results[0].offset, 0);
        assert_eq!(results[0].length, 22366);
        assert_eq!(results[1].offset, 22366);
        assert_eq!(results[1].length, 8282);
        assert_eq!(results[2].offset, 30648);
        assert_eq!(results[2].length, 16303);
        assert_eq!(results[3].offset, 46951);
        assert_eq!(results[3].length, 18696);
        assert_eq!(results[4].offset, 65647);
        assert_eq!(results[4].length, 32768);
        assert_eq!(results[5].offset, 98415);
        assert_eq!(results[5].length, 11051);
    }

    #[test]
    fn test_sekien_32k_chunks() {
        let read_result = fs::read("test/fixtures/SekienAkashita.jpg");
        assert!(read_result.is_ok());
        let contents = read_result.unwrap();
        let chunker = FastCDC::new(&contents, 16384, 32768, 65536);
        let results: Vec<Chunk> = chunker.collect();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].offset, 0);
        assert_eq!(results[0].length, 32857);
        assert_eq!(results[1].offset, 32857);
        assert_eq!(results[1].length, 16408);
        assert_eq!(results[2].offset, 49265);
        assert_eq!(results[2].length, 60201);
    }

    #[test]
    fn test_sekien_64k_chunks() {
        let read_result = fs::read("test/fixtures/SekienAkashita.jpg");
        assert!(read_result.is_ok());
        let contents = read_result.unwrap();
        let chunker = FastCDC::new(&contents, 32768, 65536, 131_072);
        let results: Vec<Chunk> = chunker.collect();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].offset, 0);
        assert_eq!(results[0].length, 32857);
        assert_eq!(results[1].offset, 32857);
        assert_eq!(results[1].length, 76609);
    }
}
