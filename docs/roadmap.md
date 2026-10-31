# PathMuster roadmap

Status: **planning document for the proposed design.** No code exists yet. This
roadmap turns [docs/architecture.md](architecture.md) into an implementation
order. It does not change the design; where the two disagree, the architecture
wins and this file should be corrected.

## Contents

1. [How to read this roadmap](#1-how-to-read-this-roadmap)
2. [Milestones at a glance](#2-milestones-at-a-glance)
3. [Dependencies and implementation order](#3-dependencies-and-implementation-order)
4. [Quality gates that apply to every change](#4-quality-gates-that-apply-to-every-change)
5. [M1: Portable-path checker](#5-m1-portable-path-checker)
6. [M2: Manifest inputs and JSON reports](#6-m2-manifest-inputs-and-json-reports)
7. [M3: USD audit of composition arcs](#7-m3-usd-audit-of-composition-arcs)
8. [M4: USD asset values, variants and clips](#8-m4-usd-asset-values-variants-and-clips)
9. [M5: Dependency closure](#9-m5-dependency-closure)
10. [M6: Hardening and 0.1 release](#10-m6-hardening-and-01-release)
11. [Risks](#11-risks)
12. [Deferred work (not committed)](#12-deferred-work-not-committed)
13. [Clarifications adopted by the tasks](#13-clarifications-adopted-by-the-tasks)
14. [Open decisions](#14-open-decisions)

---

## 1. How to read this roadmap

- **Task IDs** (`T01` to `T35`) are planning identifiers. Each is written to
  become one GitHub issue; the IDs will be replaced by issue numbers once the
  issues exist.
- **Estimates** are engineer-hours for one engineer who knows Rust, as a range.
  They include writing the tests and documentation the task requires, and the
  rework a normal review round causes. They exclude waiting time. The low end
  assumes nothing surprising; the high end covers one significant surprise. A
  milestone's range is the sum of its tasks, so the real total is more likely to
  land in the middle than at either end.
- **Calendar time** is given at about 25 productive hours a week, which is a
  planning figure, not a promise.
- **Each milestone ends in something that can be used or demonstrated**, and
  each later milestone only adds to it.
- Architecture requirement IDs (F1 to F13, N1 to N9) and open questions (Q1 to
  Q12) are cited where a task satisfies or settles them.

## 2. Milestones at a glance

| Milestone | Outcome | Tasks | Estimate (h) | About |
|---|---|---|---|---|
| **M1** Portable-path checker | `pathmuster check-paths` on path lists: all three target profiles, collisions, policy, exit codes, CI on three operating systems | T01 to T09 (9) | 49 to 74 | 2 to 3 weeks |
| **M2** Manifest inputs and JSON | `sha256sum`, BagIt and OCFL inputs; versioned JSON reports | T10 to T14 (5) | 23 to 35 | 1 to 1.5 weeks |
| **M3** USD audit of composition arcs | `pathmuster check` on `.usda`: sublayers, references and payloads resolved with evidence; unsupported input fails closed with exit 4 | T15 to T24 (10) | 92 to 132 | 4 to 5 weeks |
| **M4** Asset values, variants, clips | `asset` attributes, time samples, variants and value clips recognised, so typical scenes can complete an audit | T25 to T28 (4) | 30 to 47 | 1.5 to 2 weeks |
| **M5** Dependency closure | `pathmuster closure` writes a `sha256sum` manifest that stock tools verify | T29 to T30 (2) | 23 to 36 | 1 to 1.5 weeks |
| **M6** Hardening and release | Fuzzing, OpenUSD oracle job, performance and limits checks, complete docs, release preparation | T31 to T35 (5) | 35 to 54 | 1.5 to 2 weeks |
| **Total to 0.1.0** | | **35** | **252 to 378** (about 300 likely) | about 11 to 15 weeks |

The architecture's acceptance criteria for the initial slice (section 14) are
met at the end of M5. Version 0.1.0 is cut after M6, because a tool that claims
"no panics on untrusted input" should be fuzzed before it is published.

## 3. Dependencies and implementation order

```text
T01 workspace + CI
 ├─ T02 findings ─┬─ T03 component rules ─┐
 │                ├─ T04 path-level rules ┤
 │                ├─ T05 collisions ──────┼─ T09 check-paths ── T13 input inference
 │                ├─ T07 policy/outcome ──┤       │
 │                └─ T08 text report ─────┘       └─ T14 JSON report
 └─ T06 list readers ──┬─ T10 sha256sum ─┐
                       ├─ T11 BagIt ─────┼─ T13
                       └─ T12 OCFL ──────┘

T01 ─ T15 path syntax ─┬─────────────────────────┐
T01 ─ T16 RelPath ─ T17 resolver ─ T20 USD resolution
T01,T02 ─ T18 USD lexer ─ T19 arcs ─┴─ T21 audit engine ─ T22 fail-closed ─ T23 check ─ T24 fixtures
                                                                    │
T19 ─ T25 asset values ─┬─ T26 variants ─┐                          │
                        └─ T27 clips ────┴─ T28 full fixture + 3-OS CI
T21,T10 ─ T29 hashing + writer ─ T30 closure command (needs T23)
M6: T31 fuzz · T32 oracle (needs T28) · T33 performance (needs T30) · T34 docs · T35 release
```

Order inside a milestone follows the task numbers unless the dependency list on
the task says otherwise. Two tasks with no dependency on each other can be done
in parallel by two people; the plan does not assume it.

Why this order:

1. **The path rules come first** because they are pure, carry most of the unit
   tests, and are useful without any project format (architecture section 5).
   They also fix the finding model, policy and report types that everything
   else reuses.
2. **Fail-closed is built in before breadth.** In M3 the USD reader recognises
   only sublayers, references and payloads; an asset path anywhere else is an
   `unsupported-construct` gap and the audit is `incomplete` (exit 4). So M3 is
   safe to use on any `.usda` file even though it understands little. M4 then
   turns known gaps into recognised constructs one at a time.
3. **Closure comes after the audit** because it reuses the audit's in-memory
   closure set; only hashing and the manifest writer are new.
4. **Fuzzing, the oracle job and performance work come last** because they need
   the parsers and the closure to exist, but they gate the release.

## 4. Quality gates that apply to every change

Every task's pull request must keep these green. The first task (T01) builds
them; later tasks must not weaken them.

| Gate | Command or check |
|---|---|
| Format | `cargo fmt --all --check` |
| Lints | `cargo clippy --all-targets -- -D warnings`; in library non-test code also deny `clippy::unwrap_used`, `clippy::expect_used`, `clippy::indexing_slicing` |
| Tests | `cargo test --workspace` on Linux, macOS and Windows runners |
| MSRV | build and test on Rust 1.85, the declared `rust-version` |
| Dependencies | `cargo deny check` (licences, advisories, duplicates, sources) |
| Unsafe | `#![forbid(unsafe_code)]` in both crates |
| Docs | `#![deny(missing_docs)]` on the library; `cargo doc --no-deps` with warnings denied |
| Findings catalogue | a test fails if any `FindingCode` has no entry in `docs/findings.md` (added in T02) |
| Repository portability | CI runs `pathmuster check-paths` over the repository's own file list (added in T09) |
| Determinism | output-affecting changes carry a test showing identical output for shuffled input or directory order |

Each task also states its own tests. A task is done when its acceptance
criteria hold, the gates above pass, and its documentation is in the same
change.

---

## 5. M1: Portable-path checker

**Goal.** A first slice that is useful with no project format at all: check
whether a list of file names, such as `git ls-files -z`, will survive a move to
Windows, macOS or Linux.

**Usable result.**

```text
$ git ls-files -z | pathmuster check-paths --input nul --target windows -
```

prints located findings and exits 0, 1, 2, 3 or 4 as the architecture
specifies. Estimate **49 to 74 h**.

| Task | Title | Estimate (h) | Depends on |
|---|---|---|---|
| T01 | Cargo workspace, CI and quality gates | 6 to 9 | none |
| T02 | Finding model, severities and stable codes | 5 to 8 | T01 |
| T03 | Per-component portability rules | 9 to 13 | T02 |
| T04 | Whole-path rules: length and encoding | 3 to 5 | T02 |
| T05 | Set-level collision detection | 8 to 12 | T02 |
| T06 | Path list readers: lines and NUL | 4 to 6 | T01 |
| T07 | Policy, outcome and gap handling | 4 to 6 | T02 |
| T08 | Text report renderer with output escaping | 3 to 5 | T02 |
| T09 | `check-paths` command | 7 to 10 | T03 to T08 |

**Satisfies:** F5, F6, part of F10 (lists), F11 (text), F12, F13 (the rule for
gap codes), N1, N2, N5, N8, N9.

**Settles:** Q2 (documented approximation, with edge cases listed), Q3 (reserved
names are errors in every form), Q4 (path length is a warning with a root
budget).

**Quality gates specific to M1.**

- Table-driven tests cover every rule on every target, including the examples
  in architecture section 13: `CON`, `con.txt`, `COM¹`, `aux.exr`, `trailing.`,
  `trailing `, `a:b.png`, U+0007, and 255 against 256 units measured in UTF-8
  and in UTF-16.
- Property tests: collision results are the same for any permutation of the
  input; the line and NUL readers return a result or a finding, never panic, on
  arbitrary bytes.
- Case and Unicode edge cases (`ß`, Turkish dotted I, Greek final sigma, NFC
  against NFD) have a test and an entry in `docs/findings.md`.
- The CI matrix is green on Linux, macOS, Windows and the MSRV job.
- Exit-code tests for 0, 1, 2, 3 and 4 through `assert_cmd`.

**Risks.** Rule details drift from what real Windows and macOS do (R4);
approximate case folding misses a collision (R3). Both are bounded by writing
the approximation down and keeping real-filesystem fixtures for CI runners.

---

## 6. M2: Manifest inputs and JSON reports

**Goal.** Finish the standalone half of the tool: read the manifest formats
people already have, and give CI a stable machine-readable report.

**Usable result.** `pathmuster check-paths bag/manifest-sha256.txt --target all`
and `--format json` work. Estimate **23 to 35 h**.

| Task | Title | Estimate (h) | Depends on |
|---|---|---|---|
| T10 | `sha256sum` manifest reader | 5 to 8 | T06 |
| T11 | BagIt payload manifest reader | 3 to 5 | T06 |
| T12 | OCFL inventory reader (head version) | 4 to 6 | T06 |
| T13 | Input format inference and `--input` options | 3 to 4 | T09, T10 to T12 |
| T14 | JSON report, schema version 1 | 8 to 12 | T09 |

**Satisfies:** F10 (all formats), F11 (JSON), N2.

**Quality gates specific to M2.**

- Round-trip property tests: any set of paths written in `sha256sum` escape
  form, and in BagIt percent-encoding, reads back as the same set.
- Reader tests against hand-written files that follow the `sha256sum` format
  (binary marker, backslash-escaped lines, `\r` escape), RFC 8493 manifests, and
  an OCFL 1.1 inventory with more than one version (only `head` is used).
- JSON snapshot tests with `insta`; a determinism test that runs the same input
  with shuffled order and compares bytes; a test that the report holds no
  timestamp or host name.
- `docs/report-format.md` describes schema version 1 and the additive-only rule.

**Risks.** Freezing the report layout before the audit exists (R7). The report
is designed with the reference and closure sections in view, and the schema
promise starts at the 0.1.0 release, not at M2.

---

## 7. M3: USD audit of composition arcs

**Goal.** Audit a `.usda` scene and the text layers it reaches, resolve every
sublayer, reference and payload against a project root with exact-name matching,
and refuse to call anything it did not understand clean.

**Usable result.**

```text
$ pathmuster check shot/scene.usda --target all
```

prints the located findings and an `audit: complete` or `audit: incomplete`
line. Because every asset path outside the supported contexts is a coverage gap,
the result is safe to trust on any text layer: it never reports a pass for input
it could not read. Estimate **92 to 132 h**, the largest milestone.

| Task | Title | Estimate (h) | Depends on |
|---|---|---|---|
| T15 | Path syntax classification | 6 to 9 | T01 |
| T16 | `RelPath` and lexical normalization | 4 to 6 | T01 |
| T17 | Exact-name resolver | 14 to 20 | T16 |
| T18 | `.usda` lexer | 14 to 20 | T01, T02 |
| T19 | USD composition-arc extraction | 12 to 16 | T15, T18 |
| T20 | USD resolution semantics and search paths | 5 to 8 | T15, T17 |
| T21 | Audit engine, traversal and closure model | 16 to 22 | T03, T05, T17, T19, T20 |
| T22 | Fail-closed coverage and the incomplete outcome | 6 to 9 | T07, T19, T21 |
| T23 | `check` command | 8 to 12 | T08, T14, T21, T22 |
| T24 | Core USD fixtures and snapshot tests | 7 to 10 | T23 |

**Satisfies:** F1, F3, F4, F7 (the in-memory closure), F8, F13, N3, N5, N6;
the audit part of F11 and F12.

**Settles:** Q1 (in-house lexer), Q5 (follow in-root symlinks with a warning),
Q7 (a case-mismatched target stays in the closure), Q8 (`#` comments only until
the oracle job confirms the others; any other form is a gap).

**Quality gates specific to M3.**

- Property tests: the lexer never panics on arbitrary bytes; inserting
  whitespace and `#` comments between tokens of a valid layer does not change
  the extracted references; shuffling the directory listing the resolver sees
  does not change the report.
- Resolver tests run on Linux, macOS and Windows CI runners. Fixtures that
  cannot be committed portably (case-colliding pairs, trailing dots, NFC and
  NFD pairs) are generated in a temporary directory and skipped with a message
  where the host cannot create them.
- The two acceptance fixtures from architecture section 14 exist: a core
  fixture with the planted defects the composition-arc subset can express, and
  an unsupported-construct fixture that yields located findings,
  `audit: incomplete` and exit 4 under every policy setting, including
  `--fail-on never`.
- A hostile-input test set: unterminated asset paths and strings, deeply nested
  brackets, a layer cycle, a symlink pointing outside the root, a FIFO named as
  a layer. None may panic or hang; each produces a finding.

**Risks.** The `.usda` grammar is large and the lexer is hand written (R1);
the resolver is the only module that touches the filesystem and is where host
differences show up (R5). Mitigations are fail-closed handling and the shuffled
listing tests.

---

## 8. M4: USD asset values, variants and clips

**Goal.** Recognise the constructs that carry most real textures and caches, so
that a typical text scene can complete an audit instead of ending in
`unsupported-construct`.

**Usable result.** `pathmuster check` understands `asset` and `asset[]`
attributes (default values and time samples), references inside variant sets,
value-clip `assetPaths`, and the metadata contexts. Estimate **30 to 47 h**.

| Task | Title | Estimate (h) | Depends on |
|---|---|---|---|
| T25 | `asset` and `asset[]` attribute extraction | 12 to 18 | T19 |
| T26 | Variant-set extraction | 6 to 10 | T25 |
| T27 | Value clips and unexpanded patterns | 6 to 10 | T25 |
| T28 | Full core fixture and three-OS CI verification | 6 to 9 | T25 to T27 |

**Satisfies:** F2 (complete), F8 (pattern gaps), the rest of F1.

**Settles:** Q9 (`<UDIM>` is reported as `unexpanded-pattern`, a gap).

**Quality gates specific to M4.**

- The core fixture covers one case per classification listed in architecture
  section 13: resolved in-root, missing, POSIX absolute, Windows drive,
  outside-root, case mismatch, search-path-only, crate layer, UDIM pattern,
  variant-only reference, deleted reference, and a cycle between two sublayers.
  Expected reports are `insta` snapshots and are identical on all three runners.
- Every asset path that is still not recognised stays an `unsupported-construct`
  gap. A test lists the constructs deliberately left unsupported and asserts
  that each one yields exit 4.
- Property test: wrapping the same references in a variant set changes `context`
  but not the set of extracted paths.

**Risks.** Context tracking by a small state machine can attach the wrong kind
to an asset path, or miss a form (R1). The fallback is safe: an unrecognised
context is a gap, and the oracle job in M6 cross-checks the rest.

---

## 9. M5: Dependency closure

**Goal.** Write the set of files that must travel with a scene as a manifest
that `sha256sum -c` accepts.

**Usable result.**

```text
$ pathmuster closure shot/scene.usda --output shot.sha256
$ cd shot && sha256sum -c ../shot.sha256
```

Estimate **23 to 36 h**. This milestone completes the architecture's initial
slice (section 14).

| Task | Title | Estimate (h) | Depends on |
|---|---|---|---|
| T29 | Closure hashing and `sha256sum` writer | 11 to 18 | T10, T21 |
| T30 | `closure` command | 12 to 18 | T23, T29 |

**Satisfies:** F7, F9, N1, N2, and the closure parts of F8 and F12.

**Settles:** Q6 (external files make `closure` fail unless `--allow-external`).

**Quality gates specific to M5.**

- An integration test runs the system `sha256sum -c` on a generated closure when
  the tool is present, and is skipped with a message when it is not.
- Names containing a backslash or a newline round-trip through the writer and
  the T10 reader, and the manifest never contains an injected line.
- The manifest is byte-identical across runs and across thread counts.
- The command withholds the manifest, and says why, in each case the
  architecture lists: incomplete audit, incomplete closure, external files. The
  opt-in flags never change an exit 4.
- The command refuses to write into the audited root unless the output path is
  given explicitly (architecture section 11).

**Risks.** A file changes or is swapped for a symlink between the audit and the
hash (R8). Hashing re-checks the file type and reports the file as `unreadable`.

---

## 10. M6: Hardening and 0.1 release

**Goal.** Make the claims in the architecture checkable, and ship.

Estimate **35 to 54 h**.

| Task | Title | Estimate (h) | Depends on |
|---|---|---|---|
| T31 | Fuzz targets and scheduled fuzzing job | 8 to 12 | T18, T10 to T12 |
| T32 | OpenUSD oracle job | 12 to 18 | T28 |
| T33 | Performance and limits verification | 5 to 8 | T30 |
| T34 | Documentation completeness | 6 to 10 | T30 |
| T35 | Release preparation for 0.1.0 | 4 to 6 | T31 to T34 |

**Satisfies:** N5, N7, the "oracle job and fuzzing" items of architecture
sections 13 and 14, N9 (documented codes).

**Settles:** Q8 (comment forms confirmed against OpenUSD), Q10 (the Unicode
version is in the report; the release records it).

**Quality gates specific to M6.**

- Fuzz targets for the USD lexer and each manifest reader run in CI for a fixed
  time on a schedule, with a corpus committed to the repository. Any crash found
  becomes a regression test before the milestone closes.
- The oracle job runs OpenUSD in a container, compares the layer and asset sets
  with the closure on fixtures where both should agree, and is allowed to fail
  without blocking the merge queue. Each disagreement is resolved by fixing
  PathMuster, or recorded as an intended difference in `docs/findings.md`.
- A benchmark run shows reference extraction and resolution for 10,000
  references finishing well under a second on a local disk, excluding hashing
  (N7). Every limit in architecture section 10 has a test showing the
  `limit-exceeded` gap.
- Release checklist: licence chosen and present, `Cargo.toml` metadata, changelog,
  `cargo publish --dry-run` for both packages, `cargo deny` clean, README
  maturity table updated, and `cargo semver-checks` baseline recorded.

**Risks.** The oracle disagrees with the hand-written lexer in ways that need
real fixes (R1), which is the main reason for the upper end of T32. The OpenUSD
Python package or its container image may be hard to pin (R9).

---

## 11. Risks

| ID | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| R1 | The hand-written `.usda` lexer and context tracker miss or mislabel a construct that real scenes use | Medium | High: wrong closure or false "complete" | Fail closed on anything unrecognised; table tests per construct; property tests; oracle job (T32); fuzzing (T31); keep the supported subset small and documented |
| R2 | Real production scenes mostly use binary `.usdc` and `.usdz` layers, which the architecture does not traverse. Each one is an `untraversed-layer` gap, so those audits end `incomplete` (exit 4) and a clean pass is impossible | High for production pipelines | High for adoption | State this plainly in the README and docs. It is correct behaviour, not a bug. Text-authored, generated and test scenes are fully supported. Binary support is the first item of deferred work (section 12) and a decision for the owner (section 14) |
| R3 | Case-folding and normalization approximations miss a collision that a real filesystem has | Low to medium | Medium | Documented approximation (Q2); real-filesystem fixtures on CI runners; switch to full folding only on evidence |
| R4 | Windows and macOS naming rules are more detailed than the table (reserved names, trailing characters, Finder behaviour) | Medium | Medium | Cite the Microsoft naming rules in each test; keep the rule table data-driven so a correction is a one-line change |
| R5 | Resolver results differ by host (listing order, Unicode normalization applied by the OS, symlink support on Windows) | Medium | High: breaks N3 | Resolve through a listing abstraction; shuffled-listing property test; runtime-generated fixtures; three-OS CI from T01 |
| R6 | Dependency MSRV drift: a current release of `clap`, `rayon` or another dependency needs a Rust newer than 1.85 | Medium | Low | MSRV job from T01; pin versions in the lock file; raise MSRV in a minor release with a changelog entry if unavoidable |
| R7 | The JSON schema is fixed too early and has to change | Medium | Medium | Additive-only rule; the schema promise begins at 0.1.0; design the report with audit data in view (T14) |
| R8 | A file changes between audit and hash | Low | Medium | Re-check type at hash time; report `unreadable` instead of following a swapped symlink |
| R9 | The oracle job is flaky or hard to reproduce because OpenUSD packaging changes | Medium | Low | Pin the image and package version; the job is non-blocking |
| R10 | Estimate error. The USD tasks (T18, T19, T21, T25, T32) are the least predictable | High | Medium | Ranges are widest there. Build the lexer and fixtures together; re-estimate M4 and M5 at the end of M3 |
| R11 | Scope creep from deferred work (Godot, BagIt export, SARIF, `.usdz`) | Medium | Medium | Nothing in section 12 is scheduled until 0.1.0 ships |

## 12. Deferred work (not committed)

None of this has tasks, and none of it is in the totals above. Sizes are rough
and only for planning conversations.

| Item | Why deferred | Rough size (h) |
|---|---|---|
| Godot 4 text-project adapter (`project.godot`, `.tscn`, `.tres`, `.import`, `.uid`, UID mapping) | Architecture "second slice", first in order | 50 to 80 |
| BagIt export of the closure | Second slice, after Godot | 14 to 22 |
| SARIF 2.1.0 output | Second slice | 10 to 16 |
| `<UDIM>` tile expansion and value-clip template expansion | Small, but needs user evidence first (Q9) | 8 to 14 |
| `.usdc` crate reading and `.usdz` package inspection, package-relative paths | Large; addresses R2; revisit the `openusd` crate here (Q1) | 50 to 100, high uncertainty |
| Dependencies inside non-USD leaves (MaterialX, OSL) | Needs a format decision per leaf type | 20 to 40 |
| OCFL content paths and all versions; Godot export filters | Low demand expected | 8 to 16 |
| Non-mutating relink suggestions | Useful once findings are trusted | 12 to 20 |
| Configuration file (`pathmuster.toml`) | Flags only until users ask | 8 to 12 |
| Public adapter trait | Wait for two in-tree adapters | 10 to 16 |

## 13. Clarifications adopted by the tasks

The architecture leaves these details open. The tasks adopt the defaults below;
none changes the design, and each should be confirmed or corrected when the
task is reviewed.

| Topic | Default used in the tasks | Where |
|---|---|---|
| Column unit in `file:line:column` | 1-based, counted in Unicode scalar values; the JSON report also carries the byte offset | T18, T08 |
| Exit code when `closure` withholds the manifest | Always 1 for a complete audit, even if policy allowed the finding that caused it. Withholding the manifest is never reported as success | T30 |
| Directory entries that are not valid UTF-8 | They cannot be matched by a reference. They are reported as `not-utf8` for the entry and never silently ignored | T17 |
| Input line longer than the per-line limit | `limit-exceeded` coverage gap, so `check-paths` ends `incomplete` (exit 4) | T06, T07 |
| `manifestAssetPath` inside a `clips` dictionary | Recognised as a `ClipAsset` leaf, to be confirmed by the oracle job; anything else new in that dictionary stays a gap | T27 |
| `.usd` file with a binary header | Detected by magic, treated like `.usdc` | T21 |

## 14. Decisions

| Decision | Needed before | Status |
|---|---|---|
| Licence | The first public release (T35); better before any public push | **Open.** Choose one early so `deny.toml` and the crate metadata can be set once |
| Whether 0.1.0 should wait for binary USD support (R2) | End of M5 | **Decided:** 0.1.0 ships text-only USD. Binary layers end an audit as `incomplete` (exit 4), and the README states this. `.usdz` and `.usdc` support is planned next if early users have binary scenes |
| Whether the oracle job (T32) blocks merges | T32 | **Decided:** the oracle job does not block merges |
