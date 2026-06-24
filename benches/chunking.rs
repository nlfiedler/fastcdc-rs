//! Reproducible chunking benchmarks for FastCDC.
//! 
//! Code and strategy borrowed from perf-bench-harness-and-notes branch in
//! https://github.com/russellromney/fastcdc-rs fork
//!
//! All inputs are generated deterministically from a fixed seed (no temp
//! files, no network, nothing to clean up). Throughput is reported by
//! criterion in bytes/sec; divide by 2^20 for MiB/s.
//!
//! These are intended for local before/after comparison of a change. The
//! recommended workflow uses criterion's built-in baselines:
//!
//!     git checkout master
//!     cargo bench -- --save-baseline before
//!     git checkout <pr-branch>
//!     cargo bench -- --baseline before
//!
//! The second run prints the percent change and a significance verdict per
//! benchmark. Run on a quiet machine; treat deltas under ~10% as noise.
//!
//! Run: `cargo bench`            (all groups)
//!      `cargo bench -- v2020`   (filter by group/bench name)
//!
//! Groups:
//!   v2020_paths  — iterator vs cut()-loop vs StreamCDC, same input
//!   content      — iterator across content types (entropy sensitivity)
//!   avg_size     — iterator across 16KiB / 1MiB / 2MiB average chunk sizes
//!   small        — sub-min and tiny inputs (per-call overhead)
//!   versions     — v2016 vs v2020 vs ronomon, same input

use std::hint::black_box;
use std::io::Cursor;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use fastcdc::{ronomon, v2016, v2020};

// ---------------------------------------------------------------------------
// Deterministic data generation
// ---------------------------------------------------------------------------

/// SplitMix64 — tiny, deterministic, good enough for benchmark fill.
struct SplitMix64(u64);
impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

/// High-entropy (incompressible) bytes. Worst case for CDC: the hash rarely
/// satisfies the mask, so the scan runs long between cut points.
fn gen_random(len: usize, seed: u64) -> Vec<u8> {
    let mut rng = SplitMix64(seed);
    let mut out = Vec::with_capacity(len);
    while out.len() + 8 <= len {
        out.extend_from_slice(&rng.next_u64().to_le_bytes());
    }
    while out.len() < len {
        out.push(rng.next_u64() as u8);
    }
    out
}

/// Low-entropy text-ish bytes: a small word pool with spaces/newlines. More
/// like source code or logs — frequent cut points, lots of short scans.
fn gen_text(len: usize, seed: u64) -> Vec<u8> {
    const WORDS: &[&str] = &[
        "the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog", "lorem",
        "ipsum", "dolor", "sit", "amet", "fn", "let", "mut", "return", "struct",
        "impl", "self", "match", "async", "await", "value", "offset", "length",
    ];
    let mut rng = SplitMix64(seed);
    let mut out = Vec::with_capacity(len + 16);
    let mut col = 0;
    while out.len() < len {
        let w = WORDS[(rng.next_u64() as usize) % WORDS.len()];
        out.extend_from_slice(w.as_bytes());
        col += w.len();
        if col > 64 {
            out.push(b'\n');
            col = 0;
        } else {
            out.push(b' ');
        }
    }
    out.truncate(len);
    out
}

/// All zeros. Pathological: no cut point is ever found by content, so every
/// chunk is forced to max_size. Exercises the full inner scan to the limit.
fn gen_zeros(len: usize) -> Vec<u8> {
    vec![0u8; len]
}

/// Mixed: alternating ~64 KiB runs of random and text. Approximates a real
/// archive of binary blobs interleaved with text.
fn gen_mixed(len: usize, seed: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut s = seed;
    let mut toggle = false;
    while out.len() < len {
        let take = (64 * 1024).min(len - out.len());
        let block = if toggle {
            gen_text(take, s)
        } else {
            gen_random(take, s)
        };
        out.extend_from_slice(&block);
        toggle = !toggle;
        s = s.wrapping_add(0x1234_5678_9abc_def0);
    }
    out
}

// ---------------------------------------------------------------------------
// Driver helpers — each consumes the whole input and returns a black-box-able
// accumulator so the optimizer cannot delete the work.
// ---------------------------------------------------------------------------

fn run_v2020_iter(data: &[u8], min: usize, avg: usize, max: usize) -> usize {
    let chunker = v2020::FastCDC::new(data, min, avg, max);
    let mut acc = 0usize;
    for c in chunker {
        acc ^= c.length ^ (c.hash as usize);
    }
    acc
}

fn run_v2020_cut_loop(data: &[u8], min: usize, avg: usize, max: usize) -> usize {
    let chunker = v2020::FastCDC::new(data, min, avg, max);
    let mut pos = 0usize;
    let mut rem = data.len();
    let mut acc = 0usize;
    while rem > 0 {
        let (hash, cutpoint) = chunker.cut(pos, rem);
        if cutpoint == 0 {
            break;
        }
        let len = cutpoint - pos;
        acc ^= len ^ (hash as usize);
        pos += len;
        rem -= len;
    }
    acc
}

fn run_v2020_stream(data: &[u8], min: usize, avg: usize, max: usize) -> usize {
    let chunker = v2020::StreamCDC::new(Cursor::new(data), min, avg, max);
    let mut acc = 0usize;
    for r in chunker {
        let c = r.expect("stream chunk");
        acc ^= c.length ^ (c.hash as usize);
    }
    acc
}

fn run_v2016_iter(data: &[u8], min: usize, avg: usize, max: usize) -> usize {
    let chunker = v2016::FastCDC::new(data, min, avg, max);
    let mut acc = 0usize;
    for c in chunker {
        acc ^= c.length ^ (c.hash as usize);
    }
    acc
}

fn run_ronomon_iter(data: &[u8], min: usize, avg: usize, max: usize) -> usize {
    let chunker = ronomon::FastCDC::new(data, min, avg, max);
    let mut acc = 0usize;
    for c in chunker {
        acc ^= c.length ^ (c.hash as usize);
    }
    acc
}

// avg -> (min, avg, max) using the crate's example convention (min=avg/4, max=avg*4).
fn sizes(avg: usize) -> (usize, usize, usize) {
    (avg / 4, avg, avg * 4)
}

// ---------------------------------------------------------------------------
// Benchmark groups
// ---------------------------------------------------------------------------

const MIB: usize = 1024 * 1024;

fn bench_v2020_paths(c: &mut Criterion) {
    let data = gen_random(8 * MIB, 0xC127_u64);
    let (min, avg, max) = sizes(16 * 1024);
    let mut g = c.benchmark_group("v2020_paths");
    g.throughput(Throughput::Bytes(data.len() as u64));
    g.bench_function("iterator", |b| {
        b.iter(|| black_box(run_v2020_iter(black_box(&data), min, avg, max)))
    });
    g.bench_function("cut_loop", |b| {
        b.iter(|| black_box(run_v2020_cut_loop(black_box(&data), min, avg, max)))
    });
    g.bench_function("stream", |b| {
        b.iter(|| black_box(run_v2020_stream(black_box(&data), min, avg, max)))
    });
    g.finish();
}

fn bench_content(c: &mut Criterion) {
    let len = 8 * MIB;
    let inputs: [(&str, Vec<u8>); 4] = [
        ("random", gen_random(len, 1)),
        ("text", gen_text(len, 2)),
        ("zeros", gen_zeros(len)),
        ("mixed", gen_mixed(len, 3)),
    ];
    let (min, avg, max) = sizes(16 * 1024);
    let mut g = c.benchmark_group("content");
    g.throughput(Throughput::Bytes(len as u64));
    for (name, data) in &inputs {
        g.bench_function(*name, |b| {
            b.iter(|| black_box(run_v2020_iter(black_box(data), min, avg, max)))
        });
    }
    g.finish();
}

fn bench_avg_size(c: &mut Criterion) {
    let data = gen_random(32 * MIB, 7);
    let mut g = c.benchmark_group("avg_size");
    g.throughput(Throughput::Bytes(data.len() as u64));
    for &avg in &[16 * 1024usize, MIB, 2 * MIB] {
        let (min, avg, max) = sizes(avg);
        let label = format!("{}KiB", avg / 1024);
        g.bench_function(&label, |b| {
            b.iter(|| black_box(run_v2020_iter(black_box(&data), min, avg, max)))
        });
    }
    g.finish();
}

fn bench_small(c: &mut Criterion) {
    // Sub-min: shorter than min_size, returns a single (0, len) chunk.
    let submin = gen_random(100, 11);
    // A handful of average-sized chunks — measures per-chunk overhead.
    let few = gen_random(64 * 1024, 12);
    let (min, avg, max) = sizes(16 * 1024);
    let mut g = c.benchmark_group("small");
    g.throughput(Throughput::Bytes(submin.len() as u64));
    g.bench_function("submin_100B", |b| {
        b.iter(|| black_box(run_v2020_iter(black_box(&submin), min, avg, max)))
    });
    g.throughput(Throughput::Bytes(few.len() as u64));
    g.bench_function("few_64KiB", |b| {
        b.iter(|| black_box(run_v2020_iter(black_box(&few), min, avg, max)))
    });
    g.finish();
}

fn bench_versions(c: &mut Criterion) {
    let data = gen_random(8 * MIB, 21);
    let (min, avg, max) = sizes(16 * 1024);
    let mut g = c.benchmark_group("versions");
    g.throughput(Throughput::Bytes(data.len() as u64));
    g.bench_function("v2020", |b| {
        b.iter(|| black_box(run_v2020_iter(black_box(&data), min, avg, max)))
    });
    g.bench_function("v2016", |b| {
        b.iter(|| black_box(run_v2016_iter(black_box(&data), min, avg, max)))
    });
    g.bench_function("ronomon", |b| {
        b.iter(|| black_box(run_ronomon_iter(black_box(&data), min, avg, max)))
    });
    g.finish();
}

criterion_group!(
    benches,
    bench_v2020_paths,
    bench_content,
    bench_avg_size,
    bench_small,
    bench_versions,
);
criterion_main!(benches);
