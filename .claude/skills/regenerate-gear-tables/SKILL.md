---
name: regenerate-gear-tables
description: Regenerate the GEAR and GEAR_LS hash tables used by v2016/v2020. Use when asked to regenerate, recompute, or verify the GEAR hash tables, or after any change to the MD5-based table generation logic in examples/table64.rs or examples/table64ls.rs.
---

# Regenerate GEAR hash tables

`v2016` and `v2020` both embed a 256-entry `[u64; 256]` GEAR hash table computed from
MD5 digests of byte values 0–255. `v2020` additionally embeds a left-shifted twin,
`GEAR_LS`, used for its "rolling two bytes each time" optimization.

These tables are generated code, not hand-written — regenerate them with the example
programs rather than editing the arrays directly:

```shell
# Regenerate the base GEAR table (64-bit)
cargo run --example table64

# Regenerate the left-shifted GEAR_LS table
cargo run --example table64ls
```

Each program prints the Rust array literal to stdout. Paste the output into the
appropriate `GEAR` / `GEAR_LS` const in `src/v2016/mod.rs` and/or `src/v2020/mod.rs`,
replacing the existing array exactly (same length, same formatting conventions as the
surrounding file).

## Important

- The tables in `v2016` and `v2020` are identical for `GEAR` — only regenerate both if
  you're intentionally changing the generation algorithm; a mismatch between them is a
  bug.
- Changing these tables changes every chunk cut point the crate produces. This breaks
  the hardcoded expected hashes in the test suite (`test/fixtures/SekienAkashita.jpg`)
  **by design** — update those expected values deliberately, don't chase the failures as
  bugs. This is also a semver-relevant break for downstream users who depend on
  deterministic chunking output; call it out clearly if proposing this change.
- After regenerating, run `cargo test`, `cargo test --features tokio`, and
  `cargo test --features futures` to see the full blast radius of the change.
