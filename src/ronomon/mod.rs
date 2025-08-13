//
// Copyright (c) 2020 Nathan Fiedler
//

//! This module implements a variation of the FastCDC algorithm using
//! 31-integers and right shifts instead of left shifts.
//!
//! The explanation below is copied from
//! [ronomon/deduplication](https://github.com/ronomon/deduplication) since this
//! module is little more than a translation of that implementation:
//!
//! > The following optimizations and variations on FastCDC are involved in the
//! > chunking algorithm:
//! > * 31 bit integers to avoid 64 bit integers for the sake of the Javascript
//! >   reference implementation.
//! > * A right shift instead of a left shift to remove the need for an
//! >   additional modulus operator, which would otherwise have been necessary
//! >   to prevent overflow.
//! > * Masks are no longer zero-padded since a right shift is used instead of a
//! >   left shift.
//! > * A more adaptive threshold based on a combination of average and minimum
//! >   chunk size (rather than just average chunk size) to decide the pivot
//! >   point at which to switch masks. A larger minimum chunk size now switches
//! >   from the strict mask to the eager mask earlier.
//! > * Masks use 1 bit of chunk size normalization instead of 2 bits of chunk
//! >   size normalization.

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
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Chunk {
    /// Hash found at this location.
    pub hash: u32,
    /// Starting byte position within the original content.
    pub offset: usize,
    /// Length of the chunk in bytes.
    pub length: usize,
}

///
/// The FastCDC chunker implementation by Joran Dirk Greef.
///
/// Use `new` to construct a new instance, and then iterate over the `Chunk`s
/// via the `Iterator` trait.
///
/// This example reads a file into memory and splits it into chunks that are
/// somewhere between 16KB and 64KB, preferring something around 32KB.
///
/// ```no_run
/// let contents = std::fs::read("test/fixtures/SekienAkashita.jpg").unwrap();
/// let chunker = fastcdc::ronomon::FastCDC::new(&contents, 16384, 32768, 65536);
/// for entry in chunker {
///     println!("offset={} size={}", entry.offset, entry.length);
/// }
/// ```
///
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FastCDC<'a> {
    source: &'a [u8],
    bytes_processed: usize,
    bytes_remaining: usize,
    min_size: usize,
    avg_size: usize,
    max_size: usize,
    mask_s: u32,
    mask_l: u32,
    eof: bool,
}

impl<'a> FastCDC<'a> {
    ///
    /// Construct a new `FastCDC` that will process the given slice of bytes.
    ///
    /// The `min_size` specifies the preferred minimum chunk size, likewise for
    /// `max_size`; the `avg_size` is what the FastCDC paper refers to as the
    /// desired "normal size" of the chunks;
    ///
    pub fn new(source: &'a [u8], min_size: usize, avg_size: usize, max_size: usize) -> Self {
        FastCDC::with_eof(source, min_size, avg_size, max_size, true)
    }

    ///
    /// Construct a new `FastCDC` that will process multiple blocks of bytes.
    ///
    /// If `eof` is `false`, then the `source` contains a non-terminal block of
    /// bytes, meaning that there will be more data available in a subsequent
    /// call. If `eof` is `true` then the `source` is expected to contain the
    /// final block of data.
    ///
    pub fn with_eof(
        source: &'a [u8],
        min_size: usize,
        avg_size: usize,
        max_size: usize,
        eof: bool,
    ) -> Self {
        assert!(min_size >= MINIMUM_MIN);
        assert!(min_size <= MINIMUM_MAX);
        assert!(avg_size >= AVERAGE_MIN);
        assert!(avg_size <= AVERAGE_MAX);
        assert!(max_size >= MAXIMUM_MIN);
        assert!(max_size <= MAXIMUM_MAX);
        let bits = (avg_size as u32).ilog2();
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
            eof,
        }
    }

    /// Returns the size of the next chunk.
    fn cut(&self, mut source_offset: usize, mut source_size: usize) -> (u32, usize) {
        if source_size <= self.min_size {
            if !self.eof { (0, 0) } else { (0, source_size) }
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
                    return (hash, source_offset - source_start);
                }
            }
            // Fall back to using the "easier" chunking judgement to find chunks
            // that run larger than the desired normal size.
            while source_offset < source_len2 {
                let index = self.source[source_offset] as usize;
                source_offset += 1;
                hash = (hash >> 1) + TABLE[index];
                if (hash & self.mask_l) == 0 {
                    return (hash, source_offset - source_start);
                }
            }
            // If source is not the last buffer, we may yet find a larger chunk.
            // If sourceSize === maximum, we will not find a larger chunk and should emit.
            if !self.eof && source_size < self.max_size {
                (hash, 0)
            } else {
                // All else fails, return the whole chunk. This will happen with
                // pathological data, such as all zeroes.
                (hash, source_size)
            }
        }
    }
}

impl Iterator for FastCDC<'_> {
    type Item = Chunk;

    fn next(&mut self) -> Option<Chunk> {
        if self.bytes_remaining == 0 {
            None
        } else {
            let (chunk_hash, chunk_size) = self.cut(self.bytes_processed, self.bytes_remaining);
            if chunk_size == 0 {
                None
            } else {
                let chunk_start = self.bytes_processed;
                self.bytes_processed += chunk_size;
                self.bytes_remaining -= chunk_size;
                Some(Chunk {
                    hash: chunk_hash,
                    offset: chunk_start,
                    length: chunk_size,
                })
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // NOTE: This intentionally returns the upper bound for both `size_hint` values, as the upper bound
        // doesn't actually seem to get used by `std` and using the actual lower bound is practically
        // guaranteed to require a second capacity growth.
        let upper_bound = self.bytes_remaining / self.min_size;
        (upper_bound, Some(upper_bound))
    }
}

///
/// Find the middle of the desired chunk size, or what the FastCDC paper refers
/// to as the "normal size".
///
fn center_size(average: usize, minimum: usize, source_size: usize) -> usize {
    let mut offset: usize = minimum + minimum.div_ceil(2);
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
// The program to produce this table is named table32.rs in examples.
//
#[rustfmt::skip]
const TABLE: [u32; 256] = [
    0x5c95c078, 0x22408989, 0x2d48a214, 0x12842087, 0x530f8afb, 0x474536b9, 0x2963b4f1, 0x44cb738b,
    0x4ea7403d, 0x4d606b6e, 0x074ec5d3, 0x3af39d18, 0x726003ca, 0x37a62a74, 0x51a2f58e, 0x7506358e,
    0x5d4ab128, 0x4d4ae17b, 0x41e85924, 0x470c36f7, 0x4741cbe1, 0x01bb7f30, 0x617c1de3, 0x2b0c3a1f,
    0x50c48f73, 0x21a82d37, 0x6095ace0, 0x419167a0, 0x3caf49b0, 0x40cea62d, 0x66bc1c66, 0x545e1dad,
    0x2bfa77cd, 0x6e85da24, 0x5fb0bdc5, 0x652cfc29, 0x3a0ae1ab, 0x2837e0f3, 0x6387b70e, 0x13176012,
    0x4362c2bb, 0x66d8f4b1, 0x37fce834, 0x2c9cd386, 0x21144296, 0x627268a8, 0x650df537, 0x2805d579,
    0x3b21ebbd, 0x7357ed34, 0x3f58b583, 0x7150ddca, 0x7362225e, 0x620a6070, 0x2c5ef529, 0x7b522466,
    0x768b78c0, 0x4b54e51e, 0x75fa07e5, 0x06a35fc6, 0x30b71024, 0x1c8626e1, 0x296ad578, 0x28d7be2e,
    0x1490a05a, 0x7cee43bd, 0x698b56e3, 0x09dc0126, 0x4ed6df6e, 0x02c1bfc7, 0x2a59ad53, 0x29c0e434,
    0x7d6c5278, 0x507940a7, 0x5ef6ba93, 0x68b6af1e, 0x46537276, 0x611bc766, 0x155c587d, 0x301ba847,
    0x2cc9dda7, 0x0a438e2c, 0x0a69d514, 0x744c72d3, 0x4f326b9b, 0x7ef34286, 0x4a0ef8a7, 0x6ae06ebe,
    0x669c5372, 0x12402dcb, 0x5feae99d, 0x76c7f4a7, 0x6abdb79c, 0x0dfaa038, 0x20e2282c, 0x730ed48b,
    0x069dac2f, 0x168ecf3e, 0x2610e61f, 0x2c512c8e, 0x15fb8c06, 0x5e62bc76, 0x69555135, 0x0adb864c,
    0x4268f914, 0x349ab3aa, 0x20edfdb2, 0x51727981, 0x37b4b3d8, 0x5dd17522, 0x6b2cbfe4, 0x5c47cf9f,
    0x30fa1ccd, 0x23dedb56, 0x13d1f50a, 0x64eddee7, 0x0820b0f7, 0x46e07308, 0x1e2d1dfd, 0x17b06c32,
    0x250036d8, 0x284dbf34, 0x68292ee0, 0x362ec87c, 0x087cb1eb, 0x76b46720, 0x104130db, 0x71966387,
    0x482dc43f, 0x2388ef25, 0x524144e1, 0x44bd834e, 0x448e7da3, 0x3fa6eaf9, 0x3cda215c, 0x3a500cf3,
    0x395cb432, 0x5195129f, 0x43945f87, 0x51862ca4, 0x56ea8ff1, 0x201034dc, 0x4d328ff5, 0x7d73a909,
    0x6234d379, 0x64cfbf9c, 0x36f6589a, 0x0a2ce98a, 0x5fe4d971, 0x03bc15c5, 0x44021d33, 0x16c1932b,
    0x37503614, 0x1acaf69d, 0x3f03b779, 0x49e61a03, 0x1f52d7ea, 0x1c6ddd5c, 0x062218ce, 0x07e7a11a,
    0x1905757a, 0x7ce00a53, 0x49f44f29, 0x4bcc70b5, 0x39feea55, 0x5242cee8, 0x3ce56b85, 0x00b81672,
    0x46beeccc, 0x3ca0ad56, 0x2396cee8, 0x78547f40, 0x6b08089b, 0x66a56751, 0x781e7e46, 0x1e2cf856,
    0x3bc13591, 0x494a4202, 0x520494d7, 0x2d87459a, 0x757555b6, 0x42284cc1, 0x1f478507, 0x75c95dff,
    0x35ff8dd7, 0x4e4757ed, 0x2e11f88c, 0x5e1b5048, 0x420e6699, 0x226b0695, 0x4d1679b4, 0x5a22646f,
    0x161d1131, 0x125c68d9, 0x1313e32e, 0x4aa85724, 0x21dc7ec1, 0x4ffa29fe, 0x72968382, 0x1ca8eef3,
    0x3f3b1c28, 0x39c2fb6c, 0x6d76493f, 0x7a22a62e, 0x789b1c2a, 0x16e0cb53, 0x7deceeeb, 0x0dc7e1c6,
    0x5c75bf3d, 0x52218333, 0x106de4d6, 0x7dc64422, 0x65590ff4, 0x2c02ec30, 0x64a9ac67, 0x59cab2e9,
    0x4a21d2f3, 0x0f616e57, 0x23b54ee8, 0x02730aaa, 0x2f3c634d, 0x7117fc6c, 0x01ac6f05, 0x5a9ed20c,
    0x158c4e2a, 0x42b699f0, 0x0c7c14b3, 0x02bd9641, 0x15ad56fc, 0x1c722f60, 0x7da1af91, 0x23e0dbcb,
    0x0e93e12b, 0x64b2791d, 0x440d2476, 0x588ea8dd, 0x4665a658, 0x7446c418, 0x1877a774, 0x5626407e,
    0x7f63bd46, 0x32d2dbd8, 0x3c790f4a, 0x772b7239, 0x6f8b2826, 0x677ff609, 0x0dc82c11, 0x23ffe354,
    0x2eac53a6, 0x16139e09, 0x0afd0dbc, 0x2a4d4237, 0x56a368c7, 0x234325e4, 0x2dce9187, 0x32e8ea7e
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
            assert_eq!(entry.hash, 3106636015);
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
        assert_eq!(results[0].hash, 1527472128);
        assert_eq!(results[0].offset, 0);
        assert_eq!(results[0].length, 22366);
        assert_eq!(results[1].hash, 1174757376);
        assert_eq!(results[1].offset, 22366);
        assert_eq!(results[1].length, 8282);
        assert_eq!(results[2].hash, 2687197184);
        assert_eq!(results[2].offset, 30648);
        assert_eq!(results[2].length, 16303);
        assert_eq!(results[3].hash, 1210105856);
        assert_eq!(results[3].offset, 46951);
        assert_eq!(results[3].length, 18696);
        assert_eq!(results[4].hash, 2984739645);
        assert_eq!(results[4].offset, 65647);
        assert_eq!(results[4].length, 32768);
        assert_eq!(results[5].hash, 1121740051);
        assert_eq!(results[5].offset, 98415);
        assert_eq!(results[5].length, 11051);
    }

    #[test]
    fn test_sekien_16k_chunks_streaming() {
        let filepath = "test/fixtures/SekienAkashita.jpg";
        let read_result = fs::read(filepath);
        assert!(read_result.is_ok());
        let contents = read_result.unwrap();

        // Just as with the non-streaming test, we expect to have the same
        // number of chunks, with the same offsets and sizes, every time.
        let chunk_offsets = [0, 22366, 30648, 46951, 65647, 98415];
        let chunk_sizes = [22366, 8282, 16303, 18696, 32768, 11051];

        // The size of the buffer that we will be using for streaming the
        // content. It should be greater than or equal to the upper bound on the
        // chunk size.
        const BUF_SIZE: usize = 32768;

        // Get the size of the file to detect when we have reached the last
        // block of data to be processed by the chunker.
        let attr = fs::metadata(filepath).unwrap();
        let file_size = attr.len();
        let mut file_pos = 0;
        let mut chunk_index = 0;

        // We expect to encounter the chunks in the following groups based on
        // the buffer size we selected.
        for group_size in &[2, 1, 1, 1, 1] {
            let upper_bound = file_pos + BUF_SIZE;
            let (eof, slice) = if upper_bound >= file_size as usize {
                (true, &contents[file_pos..])
            } else {
                (false, &contents[file_pos..upper_bound])
            };
            let chunker = FastCDC::with_eof(slice, 8192, 16384, 32768, eof);
            let results: Vec<Chunk> = chunker.collect();
            assert_eq!(results.len(), *group_size);
            for idx in 0..*group_size {
                assert_eq!(results[idx].offset + file_pos, chunk_offsets[chunk_index]);
                assert_eq!(results[idx].length, chunk_sizes[chunk_index]);
                chunk_index += 1;
            }
            // advance the file pointer after using it for comparing offsets
            for result in results {
                file_pos += result.length;
            }
        }
        // assert that we processed every byte of the file
        assert_eq!(file_pos as u64, file_size);
    }

    #[test]
    fn test_sekien_32k_chunks() {
        let read_result = fs::read("test/fixtures/SekienAkashita.jpg");
        assert!(read_result.is_ok());
        let contents = read_result.unwrap();
        let chunker = FastCDC::new(&contents, 16384, 32768, 65536);
        let results: Vec<Chunk> = chunker.collect();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].hash, 2772598784);
        assert_eq!(results[0].offset, 0);
        assert_eq!(results[0].length, 32857);
        assert_eq!(results[1].hash, 1651589120);
        assert_eq!(results[1].offset, 32857);
        assert_eq!(results[1].length, 16408);
        assert_eq!(results[2].hash, 1121740051);
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
        assert_eq!(results[0].hash, 2772598784);
        assert_eq!(results[0].offset, 0);
        assert_eq!(results[0].length, 32857);
        assert_eq!(results[1].hash, 1121740051);
        assert_eq!(results[1].offset, 32857);
        assert_eq!(results[1].length, 76609);
    }
}
