//
// Copyright (c) 2019 Nathan Fiedler
//

include!("table.rs");

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

/// Represents a chunk.
pub struct Chunk {
    /// Starting byte position within the original content.
    pub offset: usize,
    /// Length of the chunk in bytes.
    pub length: usize,
}

pub struct FastCDC<'a> {
    source: &'a [u8],
    bytes_processed: usize,
    bytes_remaining: usize,
    min_size: usize,
    avg_size: usize,
    max_size: usize,
    mask1: u32,
    mask2: u32,
}

impl<'a> FastCDC<'a> {
    pub fn new(source: &'a [u8], min_size: usize, avg_size: usize, max_size: usize) -> Self {
        assert!(min_size >= MINIMUM_MIN);
        assert!(min_size <= MINIMUM_MAX);
        assert!(avg_size >= AVERAGE_MIN);
        assert!(avg_size <= AVERAGE_MAX);
        assert!(max_size >= MAXIMUM_MIN);
        assert!(max_size <= MAXIMUM_MAX);
        let bits = logarithm2(avg_size as u32);
        let mask1 = mask(bits + 1);
        let mask2 = mask(bits - 1);
        Self {
            source,
            bytes_processed: 0,
            bytes_remaining: source.len(),
            min_size,
            avg_size,
            max_size,
            mask1,
            mask2,
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
            while source_offset < source_len1 {
                let index = self.source[source_offset] as usize;
                source_offset += 1;
                hash = (hash >> 1) + TABLE[index];
                if (hash & self.mask1) == 0 {
                    return source_offset - source_start;
                }
            }
            while source_offset < source_len2 {
                let index = self.source[source_offset] as usize;
                source_offset += 1;
                hash = (hash >> 1) + TABLE[index];
                if (hash & self.mask2) == 0 {
                    return source_offset - source_start;
                }
            }
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
/// Division that rounds up where modulus would be zero.
///
fn ceil_div(x: usize, y: usize) -> usize {
    if x % y == 0 {
        ((x / y) + 1)
    } else {
        (x / y)
    }
}

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
        assert_eq!(ceil_div(10, 3), 3);
        assert_eq!(ceil_div(9, 3), 4);
        assert_eq!(ceil_div(6, 2), 4);
        assert_eq!(ceil_div(5, 2), 2);
    }

    #[test]
    fn test_center_size() {
        assert_eq!(center_size(50, 100, 50), 0);
        assert_eq!(center_size(200, 100, 50), 49);
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
