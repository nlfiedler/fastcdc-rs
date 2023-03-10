//
// Copyright (c) 2023 Nathan Fiedler
//

//! This module implements the canonical FastCDC algorithm as described in the
//! [paper](https://www.usenix.org/system/files/conference/atc16/atc16-paper-xia.pdf)
//! by Wen Xia, et al., in 2016.
//!
//! The algorithm incorporates a simplified hash judgement using the fast Gear
//! hash, sub-minimum chunk cut-point skipping, and normalized chunking to
//! produce chunks of a more consistent length.
//!
//! There are two ways in which to use the `FastCDC` struct defined in this
//! module. One is to simply invoke `cut()` while managing your own `start` and
//! `remaining` values. The other is to use the struct as an `Iterator` that
//! yields `Chunk` structs which represent the offset and size of the chunks.
//! Note that attempting to use both `cut()` and `Iterator` on the same
//! `FastCDC` instance will yield incorrect results.
//!
//! Note that the `cut()` function returns the 64-bit hash of the chunk, which
//! may be useful in scenarios involving chunk size prediction using historical
//! data, such as in RapidCDC or SuperCDC. This hash value is also given in the
//! `hash` field of the `Chunk` struct. While this value has rather low entropy,
//! it is computationally cost-free and can be put to some use with additional
//! record keeping.
//!
//! The `StreamCDC` implementation is similar to `FastCDC` except that it will
//! read data from a `Read` into an internal buffer of `max_size` and produce
//! `ChunkData` values from the `Iterator`.
use std::fmt;
use std::io::Read;

/// Smallest acceptable value for the minimum chunk size.
pub const MINIMUM_MIN: u32 = 64;
/// Largest acceptable value for the minimum chunk size.
pub const MINIMUM_MAX: u32 = 1_048_576;
/// Smallest acceptable value for the average chunk size.
pub const AVERAGE_MIN: u32 = 256;
/// Largest acceptable value for the average chunk size.
pub const AVERAGE_MAX: u32 = 4_194_304;
/// Smallest acceptable value for the maximum chunk size.
pub const MAXIMUM_MIN: u32 = 1024;
/// Largest acceptable value for the maximum chunk size.
pub const MAXIMUM_MAX: u32 = 16_777_216;

//
// Masks for each of the desired number of bits, where 0 through 5 are unused.
// The values for sizes 64 bytes through 128 kilo-bytes comes from the C
// reference implementation (found in the destor repository) while the extra
// values come from the restic-FastCDC repository. The FastCDC paper claims that
// the deduplication ratio is slightly improved when the mask bits are spread
// relatively evenly, hence these seemingly "magic" values.
//
const MASKS: [u64; 26] = [
    0,                  // padding
    0,                  // padding
    0,                  // padding
    0,                  // padding
    0,                  // padding
    0x0000000001804110, // unused except for NC 3
    0x0000000001803110, // 64B
    0x0000000018035100, // 128B
    0x0000001800035300, // 256B
    0x0000019000353000, // 512B
    0x0000590003530000, // 1KB
    0x0000d90003530000, // 2KB
    0x0000d90103530000, // 4KB
    0x0000d90303530000, // 8KB
    0x0000d90313530000, // 16KB
    0x0000d90f03530000, // 32KB
    0x0000d90303537000, // 64KB
    0x0000d90703537000, // 128KB
    0x0000d90707537000, // 256KB
    0x0000d91707537000, // 512KB
    0x0000d91747537000, // 1MB
    0x0000d91767537000, // 2MB
    0x0000d93767537000, // 4MB
    0x0000d93777537000, // 8MB
    0x0000d93777577000, // 16MB
    0x0000db3777577000, // unused except for NC 3
];

//
// GEAR contains seemingly random numbers which are created by computing the
// MD5 digest of values from 0 to 255, using only the high 8 bytes of the 16
// byte digest. This is the "gear hash" referred to the in FastCDC paper.
//
// The program to produce this table is named table64.rs in examples.
//
#[rustfmt::skip]
const GEAR: [u64; 256] = [
    0x3b5d3c7d207e37dc, 0x784d68ba91123086, 0xcd52880f882e7298, 0xeacf8e4e19fdcca7,
    0xc31f385dfbd1632b, 0x1d5f27001e25abe6, 0x83130bde3c9ad991, 0xc4b225676e9b7649,
    0xaa329b29e08eb499, 0xb67fcbd21e577d58, 0x0027baaada2acf6b, 0xe3ef2d5ac73c2226,
    0x0890f24d6ed312b7, 0xa809e036851d7c7e, 0xf0a6fe5e0013d81b, 0x1d026304452cec14,
    0x03864632648e248f, 0xcdaacf3dcd92b9b4, 0xf5e012e63c187856, 0x8862f9d3821c00b6,
    0xa82f7338750f6f8a, 0x1e583dc6c1cb0b6f, 0x7a3145b69743a7f1, 0xabb20fee404807eb,
    0xb14b3cfe07b83a5d, 0xb9dc27898adb9a0f, 0x3703f5e91baa62be, 0xcf0bb866815f7d98,
    0x3d9867c41ea9dcd3, 0x1be1fa65442bf22c, 0x14300da4c55631d9, 0xe698e9cbc6545c99,
    0x4763107ec64e92a5, 0xc65821fc65696a24, 0x76196c064822f0b7, 0x485be841f3525e01,
    0xf652bc9c85974ff5, 0xcad8352face9e3e9, 0x2a6ed1dceb35e98e, 0xc6f483badc11680f,
    0x3cfd8c17e9cf12f1, 0x89b83c5e2ea56471, 0xae665cfd24e392a9, 0xec33c4e504cb8915,
    0x3fb9b15fc9fe7451, 0xd7fd1fd1945f2195, 0x31ade0853443efd8, 0x255efc9863e1e2d2,
    0x10eab6008d5642cf, 0x46f04863257ac804, 0xa52dc42a789a27d3, 0xdaaadf9ce77af565,
    0x6b479cd53d87febb, 0x6309e2d3f93db72f, 0xc5738ffbaa1ff9d6, 0x6bd57f3f25af7968,
    0x67605486d90d0a4a, 0xe14d0b9663bfbdae, 0xb7bbd8d816eb0414, 0xdef8a4f16b35a116,
    0xe7932d85aaaffed6, 0x08161cbae90cfd48, 0x855507beb294f08b, 0x91234ea6ffd399b2,
    0xad70cf4b2435f302, 0xd289a97565bc2d27, 0x8e558437ffca99de, 0x96d2704b7115c040,
    0x0889bbcdfc660e41, 0x5e0d4e67dc92128d, 0x72a9f8917063ed97, 0x438b69d409e016e3,
    0xdf4fed8a5d8a4397, 0x00f41dcf41d403f7, 0x4814eb038e52603f, 0x9dafbacc58e2d651,
    0xfe2f458e4be170af, 0x4457ec414df6a940, 0x06e62f1451123314, 0xbd1014d173ba92cc,
    0xdef318e25ed57760, 0x9fea0de9dfca8525, 0x459de1e76c20624b, 0xaeec189617e2d666,
    0x126a2c06ab5a83cb, 0xb1321532360f6132, 0x65421503dbb40123, 0x2d67c287ea089ab3,
    0x6c93bff5a56bd6b6, 0x4ffb2036cab6d98d, 0xce7b785b1be7ad4f, 0xedb42ef6189fd163,
    0xdc905288703988f6, 0x365f9c1d2c691884, 0xc640583680d99bfe, 0x3cd4624c07593ec6,
    0x7f1ea8d85d7c5805, 0x014842d480b57149, 0x0b649bcb5a828688, 0xbcd5708ed79b18f0,
    0xe987c862fbd2f2f0, 0x982731671f0cd82c, 0xbaf13e8b16d8c063, 0x8ea3109cbd951bba,
    0xd141045bfb385cad, 0x2acbc1a0af1f7d30, 0xe6444d89df03bfdf, 0xa18cc771b8188ff9,
    0x9834429db01c39bb, 0x214add07fe086a1f, 0x8f07c19b1f6b3ff9, 0x56a297b1bf4ffe55,
    0x94d558e493c54fc7, 0x40bfc24c764552cb, 0x931a706f8a8520cb, 0x32229d322935bd52,
    0x2560d0f5dc4fefaf, 0x9dbcc48355969bb6, 0x0fd81c3985c0b56a, 0xe03817e1560f2bda,
    0xc1bb4f81d892b2d5, 0xb0c4864f4e28d2d7, 0x3ecc49f9d9d6c263, 0x51307e99b52ba65e,
    0x8af2b688da84a752, 0xf5d72523b91b20b6, 0x6d95ff1ff4634806, 0x562f21555458339a,
    0xc0ce47f889336346, 0x487823e5089b40d8, 0xe4727c7ebc6d9592, 0x5a8f7277e94970ba,
    0xfca2f406b1c8bb50, 0x5b1f8a95f1791070, 0xd304af9fc9028605, 0x5440ab7fc930e748,
    0x312d25fbca2ab5a1, 0x10f4a4b234a4d575, 0x90301d55047e7473, 0x3b6372886c61591e,
    0x293402b77c444e06, 0x451f34a4d3e97dd7, 0x3158d814d81bc57b, 0x034942425b9bda69,
    0xe2032ff9e532d9bb, 0x62ae066b8b2179e5, 0x9545e10c2f8d71d8, 0x7ff7483eb2d23fc0,
    0x00945fcebdc98d86, 0x8764bbbe99b26ca2, 0x1b1ec62284c0bfc3, 0x58e0fcc4f0aa362b,
    0x5f4abefa878d458d, 0xfd74ac2f9607c519, 0xa4e3fb37df8cbfa9, 0xbf697e43cac574e5,
    0x86f14a3f68f4cd53, 0x24a23d076f1ce522, 0xe725cd8048868cc8, 0xbf3c729eb2464362,
    0xd8f6cd57b3cc1ed8, 0x6329e52425541577, 0x62aa688ad5ae1ac0, 0x0a242566269bf845,
    0x168b1a4753aca74b, 0xf789afefff2e7e3c, 0x6c3362093b6fccdb, 0x4ce8f50bd28c09b2,
    0x006a2db95ae8aa93, 0x975b0d623c3d1a8c, 0x18605d3935338c5b, 0x5bb6f6136cad3c71,
    0x0f53a20701f8d8a6, 0xab8c5ad2e7e93c67, 0x40b5ac5127acaa29, 0x8c7bf63c2075895f,
    0x78bd9f7e014a805c, 0xb2c9e9f4f9c8c032, 0xefd6049827eb91f3, 0x2be459f482c16fbd,
    0xd92ce0c5745aaa8c, 0x0aaa8fb298d965b9, 0x2b37f92c6c803b15, 0x8c54a5e94e0f0e78,
    0x95f9b6e90c0a3032, 0xe7939faa436c7874, 0xd16bfe8f6a8a40c9, 0x44982b86263fd2fa,
    0xe285fb39f984e583, 0x779a8df72d7619d3, 0xf2d79a8de8d5dd1e, 0xd1037354d66684e2,
    0x004c82a4e668a8e5, 0x31d40a7668b044e6, 0xd70578538bd02c11, 0xdb45431078c5f482,
    0x977121bb7f6a51ad, 0x73d5ccbd34eff8dd, 0xe437a07d356e17cd, 0x47b2782043c95627,
    0x9fb251413e41d49a, 0xccd70b60652513d3, 0x1c95b31e8a1b49b2, 0xcae73dfd1bcb4c1b,
    0x34d98331b1f5b70f, 0x784e39f22338d92f, 0x18613d4a064df420, 0xf1d8dae25f0bcebe,
    0x33f77c15ae855efc, 0x3c88b3b912eb109c, 0x956a2ec96bafeea5, 0x1aa005b5e0ad0e87,
    0x5500d70527c4bb8e, 0xe36c57196421cc44, 0x13c4d286cc36ee39, 0x5654a23d818b2a81,
    0x77b1dc13d161abdc, 0x734f44de5f8d5eb5, 0x60717e174a6c89a2, 0xd47d9649266a211e,
    0x5b13a4322bb69e90, 0xf7669609f8b5fc3c, 0x21e6ac55bedcdac9, 0x9b56b62b61166dea,
    0xf48f66b939797e9c, 0x35f332f9c0e6ae9a, 0xcc733f6a9a878db0, 0x3da161e41cc108c2,
    0xb7d74ae535914d51, 0x4d493b0b11d36469, 0xce264d1dfba9741a, 0xa9d1f2dc7436dc06,
    0x70738016604c2a27, 0x231d36e96e93f3d5, 0x7666881197838d19, 0x4a2a83090aaad40c,
    0xf1e761591668b35d, 0x7363236497f730a7, 0x301080e37379dd4d, 0x502dea2971827042,
    0xc2c5eb858f32625f, 0x786afb9edfafbdff, 0xdaee0d868490b2a4, 0x617366b3268609f6,
    0xae0e35a0fe46173e, 0xd1a07de93e824f11, 0x079b8b115ea4cca8, 0x93a99274558faebb,
    0xfb1e6e22e08a03b3, 0xea635fdba3698dd0, 0xcf53659328503a5c, 0xcde3b31e6fd5d780,
    0x8e3e4221d3614413, 0xef14d0d86bf1a22c, 0xe1d830d3f16c5ddb, 0xaabd2b2a451504e1
];

// Find the next chunk cut point in the source.
fn cut(
    source: &[u8],
    min_size: usize,
    avg_size: usize,
    max_size: usize,
    mask_s: u64,
    mask_l: u64,
) -> (u64, usize) {
    let mut remaining = source.len();
    if remaining <= min_size {
        return (0, remaining);
    }
    let mut center = avg_size;
    if remaining > max_size {
        remaining = max_size;
    } else if remaining < center {
        center = remaining;
    }
    let mut index = min_size;
    // Paraphrasing from the paper: Use the mask with more 1 bits for the
    // hash judgment when the current chunking position is smaller than the
    // desired size, which makes it harder to generate smaller chunks.
    let mut hash: u64 = 0;
    while index < center {
        hash = (hash << 1).wrapping_add(GEAR[source[index] as usize]);
        if (hash & mask_s) == 0 {
            return (hash, index);
        }
        index += 1;
    }
    // Again, paraphrasing: use the mask with fewer 1 bits for the hash
    // judgment when the current chunking position is larger than the
    // desired size, which makes it easier to generate larger chunks.
    let last_pos = remaining;
    while index < last_pos {
        hash = (hash << 1).wrapping_add(GEAR[source[index] as usize]);
        if (hash & mask_l) == 0 {
            return (hash, index);
        }
        index += 1;
    }
    // If all else fails, return the largest chunk. This will happen with
    // pathological data, such as all zeroes.
    (hash, index)
}

///
/// The level for the normalized chunking used by FastCDC and StreamCDC.
///
/// Normalized chunking "generates chunks whose sizes are normalized to a
/// specified region centered at the expected chunk size," as described in
/// section 4.4 of the FastCDC 2016 paper.
///
/// Note that lower levels of normalization will result in a larger range of
/// generated chunk sizes. It may be beneficial to widen the minimum/maximum
/// chunk size values given to the `FastCDC` constructor in that case.
///
/// Note that higher levels of normalization may result in the final chunk of
/// data being smaller than the minimum chunk size, which results in a hash
/// value of zero (`0`) since no calculations are performed for sub-minimum
/// chunks.
///
pub enum Normalization {
    /// No chunk size normalization, produces a wide range of chunk sizes.
    Level0,
    /// Level 1 normalization, in which fewer chunks are outside of the desired range.
    Level1,
    /// Level 2 normalization, where most chunks are of the desired size.
    Level2,
    /// Level 3 normalization, nearly all chunks are the desired size.
    Level3,
}

impl Normalization {
    fn bits(&self) -> u32 {
        match self {
            Normalization::Level0 => 0,
            Normalization::Level1 => 1,
            Normalization::Level2 => 2,
            Normalization::Level3 => 3,
        }
    }
}

///
/// Represents a chunk returned from the FastCDC iterator.
///
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Chunk {
    /// The gear hash value as of the end of the chunk.
    pub hash: u64,
    /// Starting byte position within the source.
    pub offset: usize,
    /// Length of the chunk in bytes.
    pub length: usize,
}

///
/// The FastCDC chunker implementation from 2016.
///
/// Use `new` to construct an instance, and then iterate over the `Chunk`s via
/// the `Iterator` trait.
///
/// This example reads a file into memory and splits it into chunks that are
/// roughly 16 KB in size. The minimum and maximum sizes are the absolute limit
/// on the returned chunk sizes. With this algorithm, it is helpful to be more
/// lenient on the maximum chunk size as the results are highly dependent on the
/// input data. Changing the minimum chunk size will affect the results as the
/// algorithm may find different cut points given it uses the minimum as a
/// starting point (cut-point skipping).
///
/// ```no_run
/// # use std::fs;
/// # use fastcdc::v2016::FastCDC;
/// let contents = fs::read("test/fixtures/SekienAkashita.jpg").unwrap();
/// let chunker = FastCDC::new(&contents, 8192, 16384, 65535);
/// for entry in chunker {
///     println!("offset={} length={}", entry.offset, entry.length);
/// }
/// ```
///
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FastCDC<'a> {
    source: &'a [u8],
    processed: usize,
    remaining: usize,
    min_size: usize,
    avg_size: usize,
    max_size: usize,
    mask_s: u64,
    mask_l: u64,
}

impl<'a> FastCDC<'a> {
    ///
    /// Construct a `FastCDC` that will process the given slice of bytes.
    ///
    /// Uses chunk size normalization level 1 by default.
    ///
    pub fn new(source: &'a [u8], min_size: u32, avg_size: u32, max_size: u32) -> Self {
        FastCDC::with_level(source, min_size, avg_size, max_size, Normalization::Level1)
    }

    ///
    /// Create a new `FastCDC` with the given normalization level.
    ///
    pub fn with_level(
        source: &'a [u8],
        min_size: u32,
        avg_size: u32,
        max_size: u32,
        level: Normalization,
    ) -> Self {
        assert!(min_size >= MINIMUM_MIN);
        assert!(min_size <= MINIMUM_MAX);
        assert!(avg_size >= AVERAGE_MIN);
        assert!(avg_size <= AVERAGE_MAX);
        assert!(max_size >= MAXIMUM_MIN);
        assert!(max_size <= MAXIMUM_MAX);
        let bits = logarithm2(avg_size);
        let normalization = level.bits();
        let mask_s = MASKS[(bits + normalization) as usize];
        let mask_l = MASKS[(bits - normalization) as usize];
        Self {
            source,
            processed: 0,
            remaining: source.len(),
            min_size: min_size as usize,
            avg_size: avg_size as usize,
            max_size: max_size as usize,
            mask_s,
            mask_l,
        }
    }

    ///
    /// Find the next cut point in the data, where `start` is the position from
    /// which to start processing the source data, and `remaining` are the
    /// number of bytes left to be processed.
    ///
    /// The returned 2-tuple consists of the 64-bit hash (fingerprint) and the
    /// byte offset of the end of the chunk.
    ///
    /// There is a special case in which the remaining bytes are less than the
    /// minimum chunk size, at which point this function returns a hash of 0 and
    /// the cut point is the end of the source data.
    ///
    pub fn cut(&self, start: usize, remaining: usize) -> (u64, usize) {
        let end = start + remaining;
        let (hash, count) = cut(
            &self.source[start..end],
            self.min_size,
            self.avg_size,
            self.max_size,
            self.mask_s,
            self.mask_l,
        );
        (hash, start + count)
    }
}

impl<'a> Iterator for FastCDC<'a> {
    type Item = Chunk;

    fn next(&mut self) -> Option<Chunk> {
        if self.remaining == 0 {
            None
        } else {
            let (hash, cutpoint) = self.cut(self.processed, self.remaining);
            if cutpoint == 0 {
                None
            } else {
                let offset = self.processed;
                let length = cutpoint - offset;
                self.processed += length;
                self.remaining -= length;
                Some(Chunk {
                    hash,
                    offset,
                    length,
                })
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // NOTE: This intentionally returns the upper bound for both `size_hint`
        // values, as the upper bound doesn't actually seem to get used by `std`
        // and using the actual lower bound is practically guaranteed to require
        // a second capacity growth.
        let upper_bound = self.source.len() / self.min_size;
        (upper_bound, Some(upper_bound))
    }
}

///
/// The error type returned from the `StreamCDC` iterator.
///
#[derive(Debug)]
pub enum Error {
    /// End of source data reached.
    Empty,
    /// An I/O error occurred.
    IoError(std::io::Error),
    /// Something unexpected happened.
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "chunker error: {self:?}")
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::IoError(error)
    }
}

impl From<Error> for std::io::Error {
    fn from(error: Error) -> Self {
        match error {
            Error::IoError(ioerr) => ioerr,
            Error::Empty => Self::from(std::io::ErrorKind::UnexpectedEof),
            Error::Other(str) => Self::new(std::io::ErrorKind::Other, str),
        }
    }
}

///
/// Represents a chunk returned from the StreamCDC iterator.
///
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ChunkData {
    /// The gear hash value as of the end of the chunk.
    pub hash: u64,
    /// Starting byte position within the source.
    pub offset: u64,
    /// Length of the chunk in bytes.
    pub length: usize,
    /// Source bytes contained in this chunk.
    pub data: Vec<u8>,
}

///
/// The FastCDC chunker implementation from 2016 with streaming support.
///
/// Use `new` to construct an instance, and then iterate over the `ChunkData`s
/// via the `Iterator` trait.
///
/// Note that this struct allocates a `Vec<u8>` of `max_size` bytes to act as a
/// buffer when reading from the source and finding chunk boundaries.
///
/// ```no_run
/// # use std::fs::File;
/// # use fastcdc::v2016::StreamCDC;
/// let source = File::open("test/fixtures/SekienAkashita.jpg").unwrap();
/// let chunker = StreamCDC::new(source, 4096, 16384, 65535);
/// for result in chunker {
///     let chunk = result.unwrap();
///     println!("offset={} length={}", chunk.offset, chunk.length);
/// }
/// ```
///
pub struct StreamCDC<R: Read> {
    /// Buffer of data from source for finding cut points.
    buffer: Vec<u8>,
    /// Maximum capacity of the buffer (always `max_size`).
    capacity: usize,
    /// Number of relevant bytes in the `buffer`.
    length: usize,
    /// Source from which data is read into `buffer`.
    source: R,
    /// Number of bytes read from the source so far.
    processed: u64,
    /// True when the source produces no more data.
    eof: bool,
    min_size: usize,
    avg_size: usize,
    max_size: usize,
    mask_s: u64,
    mask_l: u64,
}

impl<R: Read> StreamCDC<R> {
    ///
    /// Construct a `StreamCDC` that will process bytes from the given source.
    ///
    /// Uses chunk size normalization level 1 by default.
    ///
    pub fn new(source: R, min_size: u32, avg_size: u32, max_size: u32) -> Self {
        StreamCDC::with_level(source, min_size, avg_size, max_size, Normalization::Level1)
    }

    ///
    /// Create a new `StreamCDC` with the given normalization level.
    ///
    pub fn with_level(
        source: R,
        min_size: u32,
        avg_size: u32,
        max_size: u32,
        level: Normalization,
    ) -> Self {
        assert!(min_size >= MINIMUM_MIN);
        assert!(min_size <= MINIMUM_MAX);
        assert!(avg_size >= AVERAGE_MIN);
        assert!(avg_size <= AVERAGE_MAX);
        assert!(max_size >= MAXIMUM_MIN);
        assert!(max_size <= MAXIMUM_MAX);
        let bits = logarithm2(avg_size);
        let normalization = level.bits();
        let mask_s = MASKS[(bits + normalization) as usize];
        let mask_l = MASKS[(bits - normalization) as usize];
        Self {
            buffer: vec![0_u8; max_size as usize],
            capacity: max_size as usize,
            length: 0,
            source,
            eof: false,
            processed: 0,
            min_size: min_size as usize,
            avg_size: avg_size as usize,
            max_size: max_size as usize,
            mask_s,
            mask_l,
        }
    }

    /// Fill the buffer with data from the source, returning the number of bytes
    /// read (zero if end of source has been reached).
    fn fill_buffer(&mut self) -> Result<usize, Error> {
        // this code originally copied from asuran crate
        if self.eof {
            Ok(0)
        } else {
            let mut all_bytes_read = 0;
            while !self.eof && self.length < self.capacity {
                let bytes_read = self.source.read(&mut self.buffer[self.length..])?;
                if bytes_read == 0 {
                    self.eof = true;
                } else {
                    self.length += bytes_read;
                    all_bytes_read += bytes_read;
                }
            }
            Ok(all_bytes_read)
        }
    }

    /// Drains a specified number of bytes from the buffer, then resizes the
    /// buffer back to `capacity` size in preparation for further reads.
    fn drain_bytes(&mut self, count: usize) -> Result<Vec<u8>, Error> {
        // this code originally copied from asuran crate
        if count > self.length {
            Err(Error::Other(format!(
                "drain_bytes() called with count larger than length: {} > {}",
                count, self.length
            )))
        } else {
            let data = self.buffer.drain(..count).collect::<Vec<u8>>();
            self.length -= count;
            self.buffer.resize(self.capacity, 0_u8);
            Ok(data)
        }
    }

    /// Find the next chunk in the source. If the end of the source has been
    /// reached, returns `Error::Empty` as the error.
    fn read_chunk(&mut self) -> Result<ChunkData, Error> {
        self.fill_buffer()?;
        if self.length == 0 {
            Err(Error::Empty)
        } else {
            let (hash, count) = cut(
                &self.buffer[..self.length],
                self.min_size,
                self.avg_size,
                self.max_size,
                self.mask_s,
                self.mask_l,
            );
            if count == 0 {
                Err(Error::Empty)
            } else {
                let offset = self.processed;
                self.processed += count as u64;
                let data = self.drain_bytes(count)?;
                Ok(ChunkData {
                    hash,
                    offset,
                    length: count,
                    data,
                })
            }
        }
    }
}

impl<R: Read> Iterator for StreamCDC<R> {
    type Item = Result<ChunkData, Error>;

    fn next(&mut self) -> Option<Result<ChunkData, Error>> {
        let slice = self.read_chunk();
        if let Err(Error::Empty) = slice {
            None
        } else {
            Some(slice)
        }
    }
}

///
/// Base-2 logarithm function for unsigned 32-bit integers.
///
fn logarithm2(value: u32) -> u32 {
    f64::from(value).log2().round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use md5::{Digest, Md5};
    use std::fs::{self, File};

    #[test]
    fn test_logarithm2() {
        assert_eq!(logarithm2(0), 0);
        assert_eq!(logarithm2(1), 0);
        assert_eq!(logarithm2(2), 1);
        assert_eq!(logarithm2(3), 2);
        assert_eq!(logarithm2(5), 2);
        assert_eq!(logarithm2(6), 3);
        assert_eq!(logarithm2(11), 3);
        assert_eq!(logarithm2(12), 4);
        assert_eq!(logarithm2(19), 4);
        assert_eq!(logarithm2(64), 6);
        assert_eq!(logarithm2(128), 7);
        assert_eq!(logarithm2(256), 8);
        assert_eq!(logarithm2(512), 9);
        assert_eq!(logarithm2(1024), 10);
        assert_eq!(logarithm2(16383), 14);
        assert_eq!(logarithm2(16384), 14);
        assert_eq!(logarithm2(16385), 14);
        assert_eq!(logarithm2(32767), 15);
        assert_eq!(logarithm2(32768), 15);
        assert_eq!(logarithm2(32769), 15);
        assert_eq!(logarithm2(65535), 16);
        assert_eq!(logarithm2(65536), 16);
        assert_eq!(logarithm2(65537), 16);
        assert_eq!(logarithm2(1_048_575), 20);
        assert_eq!(logarithm2(1_048_576), 20);
        assert_eq!(logarithm2(1_048_577), 20);
        assert_eq!(logarithm2(4_194_303), 22);
        assert_eq!(logarithm2(4_194_304), 22);
        assert_eq!(logarithm2(4_194_305), 22);
        assert_eq!(logarithm2(16_777_215), 24);
        assert_eq!(logarithm2(16_777_216), 24);
        assert_eq!(logarithm2(16_777_217), 24);
    }

    #[test]
    #[should_panic]
    fn test_minimum_too_low() {
        let array = [0u8; 1024];
        FastCDC::new(&array, 63, 256, 1024);
    }

    #[test]
    #[should_panic]
    fn test_minimum_too_high() {
        let array = [0u8; 1024];
        FastCDC::new(&array, 67_108_867, 256, 1024);
    }

    #[test]
    #[should_panic]
    fn test_average_too_low() {
        let array = [0u8; 1024];
        FastCDC::new(&array, 64, 255, 1024);
    }

    #[test]
    #[should_panic]
    fn test_average_too_high() {
        let array = [0u8; 1024];
        FastCDC::new(&array, 64, 268_435_457, 1024);
    }

    #[test]
    #[should_panic]
    fn test_maximum_too_low() {
        let array = [0u8; 1024];
        FastCDC::new(&array, 64, 256, 1023);
    }

    #[test]
    #[should_panic]
    fn test_maximum_too_high() {
        let array = [0u8; 1024];
        FastCDC::new(&array, 64, 256, 1_073_741_825);
    }

    #[test]
    fn test_masks() {
        let source = [0u8; 1024];
        let chunker = FastCDC::new(&source, 64, 256, 1024);
        assert_eq!(chunker.mask_l, MASKS[7]);
        assert_eq!(chunker.mask_s, MASKS[9]);
        let chunker = FastCDC::new(&source, 8192, 16384, 32768);
        assert_eq!(chunker.mask_l, MASKS[13]);
        assert_eq!(chunker.mask_s, MASKS[15]);
        let chunker = FastCDC::new(&source, 1_048_576, 4_194_304, 16_777_216);
        assert_eq!(chunker.mask_l, MASKS[21]);
        assert_eq!(chunker.mask_s, MASKS[23]);
    }

    #[test]
    fn test_cut_all_zeros() {
        // for all zeros, always returns chunks of maximum size
        let array = [0u8; 10240];
        let chunker = FastCDC::new(&array, 64, 256, 1024);
        let mut cursor: usize = 0;
        for _ in 0..10 {
            let (hash, pos) = chunker.cut(cursor, 10240 - cursor);
            assert_eq!(hash, 14169102344523991076);
            assert_eq!(pos, cursor + 1024);
            cursor = pos;
        }
        // assert that nothing more should be returned
        let (_, pos) = chunker.cut(cursor, 10240 - cursor);
        assert_eq!(pos, 10240);
    }

    #[test]
    fn test_cut_sekien_16k_chunks() {
        let read_result = fs::read("test/fixtures/SekienAkashita.jpg");
        assert!(read_result.is_ok());
        let contents = read_result.unwrap();
        let chunker = FastCDC::new(&contents, 4096, 16384, 65535);
        let mut cursor: usize = 0;
        let mut remaining: usize = contents.len();
        let expected: Vec<(u64, usize)> = vec![
            (17968276318003433923, 21325),
            (4098594969649699419, 17140),
            (15733367461443853673, 28084),
            (4509236223063678303, 18217),
            (2504464741100432583, 24700),
        ];
        for (e_hash, e_length) in expected.iter() {
            let (hash, pos) = chunker.cut(cursor, remaining);
            assert_eq!(hash, *e_hash);
            assert_eq!(pos, cursor + e_length);
            cursor = pos;
            remaining -= e_length;
        }
        assert_eq!(remaining, 0);
    }

    #[test]
    fn test_cut_sekien_32k_chunks() {
        let read_result = fs::read("test/fixtures/SekienAkashita.jpg");
        assert!(read_result.is_ok());
        let contents = read_result.unwrap();
        let chunker = FastCDC::new(&contents, 8192, 32768, 131072);
        let mut cursor: usize = 0;
        let mut remaining: usize = contents.len();
        let expected: Vec<(u64, usize)> =
            vec![(15733367461443853673, 66549), (2504464741100432583, 42917)];
        for (e_hash, e_length) in expected.iter() {
            let (hash, pos) = chunker.cut(cursor, remaining);
            assert_eq!(hash, *e_hash);
            assert_eq!(pos, cursor + e_length);
            cursor = pos;
            remaining -= e_length;
        }
        assert_eq!(remaining, 0);
    }

    #[test]
    fn test_cut_sekien_64k_chunks() {
        let read_result = fs::read("test/fixtures/SekienAkashita.jpg");
        assert!(read_result.is_ok());
        let contents = read_result.unwrap();
        let chunker = FastCDC::new(&contents, 16384, 65536, 262144);
        let mut cursor: usize = 0;
        let mut remaining: usize = contents.len();
        let expected: Vec<(u64, usize)> = vec![(2504464741100432583, 109466)];
        for (e_hash, e_length) in expected.iter() {
            let (hash, pos) = chunker.cut(cursor, remaining);
            assert_eq!(hash, *e_hash);
            assert_eq!(pos, cursor + e_length);
            cursor = pos;
            remaining -= e_length;
        }
        assert_eq!(remaining, 0);
    }

    struct ExpectedChunk {
        hash: u64,
        offset: u64,
        length: usize,
        digest: String,
    }

    #[test]
    fn test_iter_sekien_16k_chunks() {
        let read_result = fs::read("test/fixtures/SekienAkashita.jpg");
        assert!(read_result.is_ok());
        let contents = read_result.unwrap();
        // The digest values are not needed here, but they serve to validate
        // that the streaming version tested below is returning the correct
        // chunk data on each iteration.
        let expected_chunks = vec![
            ExpectedChunk {
                hash: 17968276318003433923,
                offset: 0,
                length: 21325,
                digest: "2bb52734718194617c957f5e07ee6054".into(),
            },
            ExpectedChunk {
                hash: 4098594969649699419,
                offset: 21325,
                length: 17140,
                digest: "badfb0757fe081c20336902e7131f768".into(),
            },
            ExpectedChunk {
                hash: 15733367461443853673,
                offset: 38465,
                length: 28084,
                digest: "18412d7414de6eb42f638351711f729d".into(),
            },
            ExpectedChunk {
                hash: 4509236223063678303,
                offset: 66549,
                length: 18217,
                digest: "04fe1405fc5f960363bfcd834c056407".into(),
            },
            ExpectedChunk {
                hash: 2504464741100432583,
                offset: 84766,
                length: 24700,
                digest: "1aa7ad95f274d6ba34a983946ebc5af3".into(),
            },
        ];
        let chunker = FastCDC::new(&contents, 4096, 16384, 65535);
        let mut index = 0;
        for chunk in chunker {
            assert_eq!(chunk.hash, expected_chunks[index].hash);
            assert_eq!(chunk.offset, expected_chunks[index].offset as usize);
            assert_eq!(chunk.length, expected_chunks[index].length);
            let mut hasher = Md5::new();
            hasher.update(&contents[chunk.offset..chunk.offset + chunk.length]);
            let table = hasher.finalize();
            let digest = format!("{:x}", table);
            assert_eq!(digest, expected_chunks[index].digest);
            index += 1;
        }
        assert_eq!(index, 5);
    }

    #[test]
    fn test_cut_sekien_16k_nc_0() {
        let read_result = fs::read("test/fixtures/SekienAkashita.jpg");
        assert!(read_result.is_ok());
        let contents = read_result.unwrap();
        let chunker = FastCDC::with_level(&contents, 4096, 16384, 65535, Normalization::Level0);
        let mut cursor: usize = 0;
        let mut remaining: usize = contents.len();
        let expected: Vec<(u64, usize)> = vec![
            (221561130519947581, 6634),
            (15733367461443853673, 59915),
            (10460176299449652894, 25597),
            (6197802202431009942, 5237),
            (2504464741100432583, 12083),
        ];
        for (e_hash, e_length) in expected.iter() {
            let (hash, pos) = chunker.cut(cursor, remaining);
            assert_eq!(hash, *e_hash);
            assert_eq!(pos, cursor + e_length);
            cursor = pos;
            remaining -= e_length;
        }
        assert_eq!(remaining, 0);
    }

    #[test]
    fn test_cut_sekien_16k_nc_3() {
        let read_result = fs::read("test/fixtures/SekienAkashita.jpg");
        assert!(read_result.is_ok());
        let contents = read_result.unwrap();
        let chunker = FastCDC::with_level(&contents, 4096, 16384, 65535, Normalization::Level3);
        let mut cursor: usize = 0;
        let mut remaining: usize = contents.len();
        let expected: Vec<(u64, usize)> = vec![
            (14582375164208481996, 17350),
            (13104072099671895560, 19911),
            (6161241554519610597, 17426),
            (16009206469796846404, 17519),
            (10460176299449652894, 19940),
            (2504464741100432583, 17320),
        ];
        for (e_hash, e_length) in expected.iter() {
            let (hash, pos) = chunker.cut(cursor, remaining);
            assert_eq!(hash, *e_hash);
            assert_eq!(pos, cursor + e_length);
            cursor = pos;
            remaining -= e_length;
        }
        assert_eq!(remaining, 0);
    }

    #[test]
    fn test_error_fmt() {
        let err = Error::Empty;
        assert_eq!(format!("{err}"), "chunker error: Empty");
    }

    #[test]
    fn test_stream_sekien_16k_chunks() {
        let file_result = File::open("test/fixtures/SekienAkashita.jpg");
        assert!(file_result.is_ok());
        let file = file_result.unwrap();
        // The set of expected results should match the non-streaming version.
        let expected_chunks = vec![
            ExpectedChunk {
                hash: 17968276318003433923,
                offset: 0,
                length: 21325,
                digest: "2bb52734718194617c957f5e07ee6054".into(),
            },
            ExpectedChunk {
                hash: 4098594969649699419,
                offset: 21325,
                length: 17140,
                digest: "badfb0757fe081c20336902e7131f768".into(),
            },
            ExpectedChunk {
                hash: 15733367461443853673,
                offset: 38465,
                length: 28084,
                digest: "18412d7414de6eb42f638351711f729d".into(),
            },
            ExpectedChunk {
                hash: 4509236223063678303,
                offset: 66549,
                length: 18217,
                digest: "04fe1405fc5f960363bfcd834c056407".into(),
            },
            ExpectedChunk {
                hash: 2504464741100432583,
                offset: 84766,
                length: 24700,
                digest: "1aa7ad95f274d6ba34a983946ebc5af3".into(),
            },
        ];
        let chunker = StreamCDC::new(file, 4096, 16384, 65535);
        let mut index = 0;
        for result in chunker {
            assert!(result.is_ok());
            let chunk = result.unwrap();
            assert_eq!(chunk.hash, expected_chunks[index].hash);
            assert_eq!(chunk.offset, expected_chunks[index].offset);
            assert_eq!(chunk.length, expected_chunks[index].length);
            let mut hasher = Md5::new();
            hasher.update(&chunk.data);
            let table = hasher.finalize();
            let digest = format!("{:x}", table);
            assert_eq!(digest, expected_chunks[index].digest);
            index += 1;
        }
        assert_eq!(index, 5);
    }
}
