//
// Copyright (c) 2025 Nathan Fiedler
//

use super::*;

#[cfg(all(feature = "futures", not(feature = "tokio")))]
use futures::{
    io::{AsyncRead, AsyncReadExt},
    stream::Stream,
};

#[cfg(all(feature = "tokio", not(feature = "futures")))]
use tokio_stream::Stream;

#[cfg(all(feature = "tokio", not(feature = "futures")))]
use tokio::io::{AsyncRead, AsyncReadExt};

#[cfg(all(feature = "tokio", not(feature = "futures")))]
use async_stream::try_stream;

///
/// An async-streamable version of the FastCDC chunker implementation from 2020
/// with streaming support.
///
/// Use `new` to construct an instance, and then [`as_stream`](AsyncStreamCDC::as_stream)
/// to produce an async [Stream] of the chunks.
///
/// Both `futures` and `tokio`-based [AsyncRead] inputs are supported via
/// feature flags. But, if necessary you can also use the
/// [`async_compat`](https://docs.rs/async-compat/latest/async_compat/) crate to
/// adapt your inputs as circumstances may require.
///
/// Note that this struct allocates a [`Vec<u8>`] of `max_size` bytes to act as a
/// buffer when reading from the source and finding chunk boundaries.
///
/// ```no_run
/// # use std::fs::File;
/// # use fastcdc::v2020::AsyncStreamCDC;
/// # #[cfg(all(feature = "futures", not(feature = "tokio")))]
/// # use futures::stream::StreamExt;
/// # #[cfg(all(feature = "tokio", not(feature = "futures")))]
/// # use tokio_stream::StreamExt;
///
/// async fn run() {
///     let source = std::fs::read("test/fixtures/SekienAkashita.jpg").unwrap();
///     let mut chunker = AsyncStreamCDC::new(source.as_ref(), 4096, 16384, 65535);
///     let stream = chunker.as_stream();
///
///     let chunks = stream.collect::<Vec<_>>().await;
///
///     for result in chunks {
///         let chunk = result.unwrap();
///         println!("offset={} length={}", chunk.offset, chunk.length);
///     }
/// }
/// ```
///
pub struct AsyncStreamCDC<R> {
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
    mask_s_ls: u64,
    mask_l_ls: u64,
    gear: Box<[u64; 256]>,
    gear_ls: Box<[u64; 256]>,
}

impl<R: AsyncRead + Unpin> AsyncStreamCDC<R> {
    ///
    /// Construct a [`AsyncStreamCDC`] that will process bytes from the given source.
    ///
    /// Uses chunk size normalization level 1 by default.
    ///
    pub fn new(source: R, min_size: u32, avg_size: u32, max_size: u32) -> Self {
        Self::with_level(source, min_size, avg_size, max_size, Normalization::Level1)
    }

    ///
    /// Create a new [`AsyncStreamCDC`] with the given normalization level.
    ///
    pub fn with_level(
        source: R,
        min_size: u32,
        avg_size: u32,
        max_size: u32,
        level: Normalization,
    ) -> Self {
        Self::with_level_and_seed(source, min_size, avg_size, max_size, level, 0)
    }

    ///
    /// Create a new [`AsyncStreamCDC`] with the given normalization level and
    /// seed to be XOR'd with the values in the gear tables.
    ///
    pub fn with_level_and_seed(
        source: R,
        min_size: u32,
        avg_size: u32,
        max_size: u32,
        level: Normalization,
        seed: u64,
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
        let (gear, gear_ls) = get_gear_with_seed(seed);
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
            mask_s_ls: mask_s << 1,
            mask_l_ls: mask_l << 1,
            gear,
            gear_ls,
        }
    }

    /// Fill the buffer with data from the source, returning the number of bytes
    /// read (zero if end of source has been reached).
    async fn fill_buffer(&mut self) -> Result<usize, Error> {
        // this code originally copied from asuran crate
        if self.eof {
            Ok(0)
        } else {
            let mut all_bytes_read = 0;
            while !self.eof && self.length < self.capacity {
                let bytes_read = self.source.read(&mut self.buffer[self.length..]).await?;
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
    async fn read_chunk(&mut self) -> Result<ChunkData, Error> {
        self.fill_buffer().await?;
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
                self.mask_s_ls,
                self.mask_l_ls,
                &self.gear,
                &self.gear_ls,
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

    #[cfg(all(feature = "tokio", not(feature = "futures")))]
    pub fn as_stream(&mut self) -> impl Stream<Item = Result<ChunkData, Error>> + '_ {
        try_stream! {
            loop {
                match self.read_chunk().await {
                    Ok(chunk) => yield chunk,
                    Err(Error::Empty) => {
                        break;
                    }
                    error @ Err(_) => {
                        error?;
                    }
                }
            }
        }
    }

    #[cfg(all(feature = "futures", not(feature = "tokio")))]
    pub fn as_stream(&mut self) -> impl Stream<Item = Result<ChunkData, Error>> + '_ {
        futures::stream::unfold(self, |this| async {
            let chunk = this.read_chunk().await;
            if let Err(Error::Empty) = chunk {
                None
            } else {
                Some((chunk, this))
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::v2020::MASKS;

    use super::AsyncStreamCDC;

    #[test]
    #[should_panic]
    fn test_minimum_too_low() {
        let array = [0u8; 1024];
        AsyncStreamCDC::new(array.as_slice(), 63, 256, 1024);
    }

    #[test]
    #[should_panic]
    fn test_minimum_too_high() {
        let array = [0u8; 1024];
        AsyncStreamCDC::new(array.as_slice(), 67_108_867, 256, 1024);
    }

    #[test]
    #[should_panic]
    fn test_average_too_low() {
        let array = [0u8; 1024];
        AsyncStreamCDC::new(array.as_slice(), 64, 255, 1024);
    }

    #[test]
    #[should_panic]
    fn test_average_too_high() {
        let array = [0u8; 1024];
        AsyncStreamCDC::new(array.as_slice(), 64, 268_435_457, 1024);
    }

    #[test]
    #[should_panic]
    fn test_maximum_too_low() {
        let array = [0u8; 1024];
        AsyncStreamCDC::new(array.as_slice(), 64, 256, 1023);
    }

    #[test]
    #[should_panic]
    fn test_maximum_too_high() {
        let array = [0u8; 1024];
        AsyncStreamCDC::new(array.as_slice(), 64, 256, 1_073_741_825);
    }

    #[test]
    fn test_masks() {
        let source = [0u8; 1024];
        let chunker = AsyncStreamCDC::new(source.as_slice(), 64, 256, 1024);
        assert_eq!(chunker.mask_l, MASKS[7]);
        assert_eq!(chunker.mask_s, MASKS[9]);
        let chunker = AsyncStreamCDC::new(source.as_slice(), 8192, 16384, 32768);
        assert_eq!(chunker.mask_l, MASKS[13]);
        assert_eq!(chunker.mask_s, MASKS[15]);
        let chunker = AsyncStreamCDC::new(source.as_slice(), 1_048_576, 4_194_304, 16_777_216);
        assert_eq!(chunker.mask_l, MASKS[21]);
        assert_eq!(chunker.mask_s, MASKS[23]);
    }

    struct ExpectedChunk {
        hash: u64,
        offset: u64,
        length: usize,
        digest: String,
    }

    use md5::{Digest, Md5};

    #[cfg(all(feature = "futures", not(feature = "tokio")))]
    use futures::stream::StreamExt;
    #[cfg(all(feature = "tokio", not(feature = "futures")))]
    use tokio_stream::StreamExt;

    #[cfg_attr(all(feature = "tokio", not(feature = "futures")), tokio::test)]
    #[cfg_attr(all(feature = "futures", not(feature = "tokio")), futures_test::test)]
    async fn test_iter_sekien_16k_chunks() {
        let read_result = std::fs::read("test/fixtures/SekienAkashita.jpg");
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
                hash: 8197189939299398838,
                offset: 21325,
                length: 17140,
                digest: "badfb0757fe081c20336902e7131f768".into(),
            },
            ExpectedChunk {
                hash: 13019990849178155730,
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
        let mut chunker = AsyncStreamCDC::new(contents.as_ref(), 4096, 16384, 65535);
        let stream = chunker.as_stream();

        let chunks = stream.collect::<Vec<_>>().await;

        let mut index = 0;

        for chunk in chunks {
            let chunk = chunk.unwrap();
            assert_eq!(chunk.hash, expected_chunks[index].hash);
            assert_eq!(chunk.offset, expected_chunks[index].offset);
            assert_eq!(chunk.length, expected_chunks[index].length);
            let mut hasher = Md5::new();
            hasher
                .update(&contents[(chunk.offset as usize)..(chunk.offset as usize) + chunk.length]);
            let table = hasher.finalize();
            let digest = format!("{:x}", table);
            assert_eq!(digest, expected_chunks[index].digest);
            index += 1;
        }
        assert_eq!(index, 5);
    }
}
