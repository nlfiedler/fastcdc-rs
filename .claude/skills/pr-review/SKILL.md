---
name: pr-review
description: Review a pull request (or the current diff) against fastcdc-rs project standards. Use when asked to review a PR, vet changes before merging to master, or check a branch for the kinds of issues that have slipped through before. Distills reviewer feedback (notably ciehanski on PR #44/#45) into concrete criteria covering silent data loss, per-iteration waste, public-API/semver impact, test rigor, cross-platform CI, and human accountability.
---

# fastcdc-rs PR review

This skill reviews a pull request against the standards this project's maintainers and
active downstream users expect. The criteria below were derived from real reviewer
feedback (ciehanski on PR #44 and #45) plus the determinism guarantees in `CLAUDE.md`.

`fastcdc` is a published crate (currently `4.0.1`) with real downstream consumers, so
the bar is "would a careful human depending on this crate be comfortable merging it into
`master`?" — not "does it compile and pass the existing tests?"

## How to run the review

1. Determine the scope of changes:
   - A specific PR: `gh pr view <N> --json title,body,files,additions,deletions` and
     `gh pr diff <N>`.
   - The current branch: `git diff master...HEAD` (or `git diff` for uncommitted work).
2. Read the changed files in full, not just the diff hunks — context outside the hunk
   often determines whether a change is correct (e.g. constructor invariants that make an
   edge case reachable or not).
3. Walk every criterion below against the change. For each finding, cite
   `file:line` and say *why* it matters to a downstream user, not just *what* it is.
4. Verify, don't assume: run `cargo test`, `cargo test --features tokio`,
   `cargo test --features futures`, `cargo clippy`, and `cargo doc --features futures`.
   Report what you actually ran and its result. If a claim in the PR body is testable
   (e.g. "every byte is emitted", "no perf regression"), test it or say you couldn't.
5. Summarize findings grouped as **Blocking**, **Should-fix**, and **Consider**, then give
   an explicit merge recommendation.

## Review criteria

### 1. No silent data loss or silent failures
The chunking code's whole job is to account for every input byte. A `break`, `return`,
early-exit, or swallowed error that can drop or skip data is a blocking issue.
- Trace every loop exit and error path: can any input byte fail to be emitted?
- An edge case that is "currently unreachable through the public API" is still a latent
  bug. It must `debug_assert!` (or otherwise fail loudly) and degrade safely (emit the
  leftover) rather than silently dropping data. (This is exactly the silent-`break`
  tail-drop ciehanski flagged on #44.)
- Confirm the chunker produces **contiguous, gapless, exact** coverage of the input.

### 2. Per-iteration / per-chunk waste
Hot-path work that could be hoisted out of the loop is a real defect in a perf-critical
crate, not a nitpick.
- Look for conversions, allocations, `try_into`, copies, or table rebuilds happening
  per chunk or per byte that could be done once (the per-chunk `try_into` on the GEAR
  tables ciehanski flagged on #44).
- The core chunking loop is latency-bound on the gear-hash recurrence; be skeptical of
  changes that add work to it, and ask for A/B benchmark evidence (see criterion 6).

### 3. Public API surface and semver
This is a published crate; API changes ripple to every downstream user.
- For any new `pub` item (struct, fn, field, trait, enum variant): is it *intended* to be
  public, or should it be `pub(crate)` / private? Default to the smallest surface that
  works. (ciehanski: "A new public `Chunker` struct is introduced — *should* this struct
  be public to the consuming user?")
- Does the change alter, remove, or rename any existing public API, or change behavior of
  existing public API? If so it's a **breaking change** requiring a **major version bump**
  per semver. Flag the required version change explicitly against the current `Cargo.toml`
  version.
- Adding a public item is at least a minor bump; document the expected version change.
- New public items need rustdoc and ideally a doctest/example.

### 4. Determinism is a contract
Identical cut points across versions are a core guarantee (see `CLAUDE.md`).
- Any change that alters cut points / hashes / chunk boundaries must be intentional,
  called out loudly in the PR, and is itself a breaking change.
- The hardcoded fixture hashes/lengths in tests are the guardrail — they should only
  change deliberately. A PR that edits expected fixture values needs strong justification.
- Cross-check `v2016` and `v2020` produce the same cut points where the docs claim they do.

### 5. Test rigor — tests must actually catch regressions
- New behavior needs tests that assert the *invariant that matters* (e.g. every byte
  emitted, in order, no gaps), not just that the happy path returns something.
- A test is only credible if it fails when the code is broken. Prefer changes where the
  author demonstrably verified this (deliberately break it, watch it fail). When reviewing,
  if a test's failure mode is unclear, mentally (or actually) inject the bug and check the
  test would catch it.
- Be honest about coverage gaps: if a branch can't be reached through the public API and
  therefore isn't covered by a test, say so plainly rather than implying full coverage.
- Cover edge cases: empty input, sub-minimum-size input, all-identical bytes (worst case
  for finding a cut), and the real fixture.
- Run the feature-gated test matrix: default, `tokio`, `futures` (the two async features
  are mutually exclusive — check the `cfg` guards still hold).

### 6. Performance claims need reproducible evidence
- Any "faster" / "no regression" claim should be backed by an interleaved old-vs-new A/B
  measurement that asserts identical output before timing, ideally on more than one
  architecture (e.g. ARM + x86_64). Don't accept single-run wall-clock numbers.
- New perf-sensitive changes are a good prompt to ask whether CI should gain a
  cross-platform test / perf-regression workflow (ciehanski's suggestion), so future PRs
  have data before merge.

### 7. Human accountability and provenance
This was ciehanski's central concern: PRs and comments written end-to-end by an AI agent
with no evident human review, landing in a depended-on crate.
- The review's job is to be the human-quality gate. Surface issues directly and concretely
  so a human can make the merge decision — never imply "looks good, merge it" as a
  substitute for maintainer judgment.
- Flag anything that looks auto-generated and unreviewed: plausible-but-wrong assertions,
  comments/PR text that overstate what was verified, tests that look like coverage but
  don't exercise the claimed invariant.
- Encourage a clear paper trail: PR description states what changed, why, what was tested,
  and the semver impact. Do not vouch for code paths you did not actually exercise.

## Output format

```
## Review of <PR # / branch>

### Blocking
- file:line — issue — why it matters downstream — suggested fix

### Should-fix
- ...

### Consider
- ...

### Verification run
- cargo test: <result>
- cargo test --features tokio / futures: <result>
- cargo clippy / cargo doc: <result>
- (any perf or invariant checks performed)

### Semver impact
- <none / patch / minor / major> — reason

### Recommendation
- <merge / merge after fixes / needs work> — one-line rationale for the human maintainer
```
