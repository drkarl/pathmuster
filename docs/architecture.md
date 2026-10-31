# PathMuster architecture

Status: **proposed design, awaiting review.** No code exists yet. Every API, CLI
command, finding code and file layout below is a proposal and may change during
review or implementation.

The crate and binary are named `pathmuster`; the JSON report schema identifier
(`pathmuster-report`) uses the same name.

## Contents

1. [Problem statement and scope](#1-problem-statement-and-scope)
2. [Requirements](#2-requirements)
3. [Public API and CLI](#3-public-api-and-cli)
4. [Domain model and invariants](#4-domain-model-and-invariants)
5. [Modules and responsibility boundaries](#5-modules-and-responsibility-boundaries)
6. [Data flow and processing lifecycle](#6-data-flow-and-processing-lifecycle)
7. [Algorithms and implementation choices](#7-algorithms-and-implementation-choices)
8. [File formats and compatibility](#8-file-formats-and-compatibility)
9. [Errors, findings and reporting](#9-errors-findings-and-reporting)
10. [Performance and resources](#10-performance-and-resources)
11. [Security and robustness](#11-security-and-robustness)
12. [Dependencies and alternatives](#12-dependencies-and-alternatives)
13. [Testing strategy](#13-testing-strategy)
14. [Initial usable implementation slice](#14-initial-usable-implementation-slice)
15. [Deferred functionality and non-goals](#15-deferred-functionality-and-non-goals)
16. [Open technical questions](#16-open-technical-questions)
17. [Rust quality expectations](#17-rust-quality-expectations)

---

## 1. Problem statement and scope

### Problem

Scene descriptions, game projects and similar creative projects refer to other
files: layers, textures, caches, scripts. A project that works on the machine it
was authored on can break when it moves to another computer, operating system,
collaborator or render node, because a reference:

- is an absolute path (`/Users/ana/lib/metal.png`, `C:\Users\ana\brick.png`) that
  exists only on the authoring machine;
- leaves the project root (`../../shared/metal.png`) and is not copied with it;
- uses Windows separators or drive letters and fails on Linux or macOS;
- differs in letter case from the file on disk, which works on a case-insensitive
  filesystem (default Windows and macOS) and fails on Linux;
- names a file whose own name cannot exist on the destination (`aux.exr` or
  `a:b.png` on Windows; `Tex.png` beside `tex.png` on any case-insensitive
  filesystem; the same name in NFC and NFD on APFS).

Existing tools answer parts of this. OpenUSD's
[`UsdUtilsComputeAllDependencies`](https://openusd.org/release/api/dependencies_8h.html)
and `usdzip` list layers, assets and unresolved paths, but a path that resolves on
the current machine is reported like any other dependency, with no statement of
why it may not survive a move. Filename linters such as pre-commit's
[`check-case-conflict`](https://github.com/pre-commit/pre-commit-hooks) and
[`check-illegal-windows-names`](https://github.com/pre-commit/pre-commit-hooks)
work on Git working trees, not on project references or manifest entries.
Archive formats ([BagIt, RFC 8493](https://www.rfc-editor.org/rfc/rfc8493),
[OCFL](https://ocfl.io/1.1/spec/)) recommend portable names but leave the check to
the user.

### What PathMuster does

1. **Reads a project's references** through a small adapter for a documented
   text format, and **resolves** each one against the project root without
   depending on the host filesystem's case behaviour.
2. **Classifies** each reference: resolved inside the root, missing, absolute,
   outside the root, Windows-only syntax, case mismatch, URI or other
   unresolvable form. Each classification carries evidence: the source file,
   line and column, the raw string and what was found on disk.
3. **Checks path portability** of every file name involved against the rules of
   selected target platforms (Windows, macOS, Linux).
4. **Computes the dependency closure**, the set of files that must travel with
   the project, and writes it as a standard `sha256sum` manifest that stock
   tools can verify.
5. **Reports** findings as human-readable text or versioned JSON, with exit
   codes suitable for CI.

The path rules also run on their own over plain path lists, `sha256sum` files,
BagIt payload manifests and OCFL inventories. No project format is needed for
that mode.

### Scope

| In scope (initial) | Next | Out of scope |
|---|---|---|
| Portable-path rule engine | Godot 4 text projects (`project.godot`, `.tscn`, `.tres`, `.import`, `.uid`) | Proprietary or undocumented project formats |
| Standalone path checking of lists and manifests | BagIt export of the closure | Rewriting or relinking references |
| USD text layers (`.usda`, text `.usd`), narrow subset | SARIF output | Fetching URIs or running custom resolvers |
| Closure as `sha256sum` manifest | `.usdz` packages, UDIM expansion | Runtime packers, sandbox capture |
| JSON and text reports, CI exit codes | | GUI |

---

## 2. Requirements

### Functional

| ID | Requirement |
|---|---|
| F1 | Audit a USD text layer and the text layers it reaches through sublayers, references and payloads, against a declared project root. |
| F2 | Extract asset paths from sublayers, references, payloads, `asset` and `asset[]` attribute values (default and time samples), value-clip `assetPaths`, and the same constructs inside variant sets. |
| F3 | Resolve each reference against the filesystem with exact-name matching, independent of host case sensitivity and Unicode normalization. |
| F4 | Classify each reference into exactly one resolution and attach zero or more findings with evidence. |
| F5 | Check every path in the closure, and every path given to `check-paths`, against the rules of each selected target platform. |
| F6 | Detect set-level collisions: names that differ only by case or by Unicode normalization within the same directory, under each target's comparison rules. |
| F7 | Compute the closure: the entry file plus every resolved in-root dependency, deduplicated and sorted, with external dependencies listed separately. |
| F8 | State whether the closure is complete, and list every reason it is not (missing files, untraversed layers, unexpanded patterns, limits hit). |
| F9 | Write the closure as a GNU `sha256sum` text manifest with paths relative to the root. |
| F10 | Read path lists (newline or NUL separated), `sha256sum` files, BagIt `manifest-*.txt` files and OCFL `inventory.json` logical paths for standalone checking. |
| F11 | Emit a human-readable report and a versioned JSON report; select pass/fail with a severity threshold and per-code overrides. |
| F12 | Exit with documented codes distinguishing pass, policy failure, incomplete audit, usage error and audit failure. |
| F13 | Fail closed on unsupported input. Every construct an adapter does not positively recognise is reported as a coverage-gap finding with its location, and an audit with any coverage gap has the result `incomplete`, never pass (section 9.1). |

### Non-functional

| ID | Requirement |
|---|---|
| N1 | **Read-only.** Never modify the audited project. Write only to output paths named by the caller. |
| N2 | **Deterministic.** The same inputs and options give byte-identical JSON and manifest output: stable ordering, no timestamps, no thread-order dependence. |
| N3 | **Host-independent results.** Auditing the same tree on Linux, macOS or Windows gives the same findings, apart from paths outside the root, which are inherently machine-specific. |
| N4 | **Headless and standalone.** Single static-friendly binary; no USD, Godot or Python installation required. |
| N5 | **Robust on untrusted input.** No panics, unbounded recursion or unbounded memory on malformed or hostile files. All limits are configurable and reported when hit. |
| N6 | **No network access.** URIs are classified, never fetched. UNC and device paths are classified, never opened. |
| N7 | **Fast enough for pre-submit checks.** Reference extraction and resolution for a project with 10,000 references completes in well under a second on a local disk, excluding hashing. |
| N8 | **Small public API** that can be stabilized: a handful of entry points, plain data types, `#[non_exhaustive]` enums. |
| N9 | **Stable finding codes.** Codes are documented strings that are not renamed once released. |

---

## 3. Public API and CLI

All signatures below are **proposed**.

### Library

```rust
use pathmuster::{audit, AuditOptions, Outcome, Policy, Target};

// Audit a project entry file. The adapter is chosen from the file's content
// and extension; the root bounds what counts as "inside the project".
let options = AuditOptions::new("shot")          // project root
    .targets(&[Target::Windows, Target::MacOs, Target::Linux])
    .search_paths(["/opt/assets"]);              // optional, USD only
let report = audit("shot/scene.usda", &options)?;

for finding in report.findings() {
    eprintln!("{finding}");                        // file:line:col: severity[code]: message
}

// Incomplete means some input was not fully read (section 9.1).
// It is never a pass, whatever the policy says.
match report.outcome(&Policy::default()) {
    Outcome::Pass => {}
    Outcome::Fail => { /* findings at or above the threshold */ }
    Outcome::Incomplete => {
        for gap in report.coverage_gaps() { /* unsupported-construct, parse-error, ... */ }
    }
}

let closure = report.closure();
if !closure.is_complete() {
    for reason in closure.incomplete_reasons() { /* ... */ }
}

// Hashing happens here, not during the audit.
let mut out = std::fs::File::create("shot.sha256")?;
closure.write_sha256sum(&mut out)?;
```

Standalone path rules, with no project and no filesystem access:

```rust
use pathmuster::rules::{RuleSet, Target};

let rules = RuleSet::for_targets(&[Target::Windows, Target::MacOs, Target::Linux]);
let result = rules.check(["textures/Tex.png", "textures/tex.png", "CON.txt"]);
for finding in result.findings() { /* case-collision, reserved-name */ }
```

Readers for the standalone input formats:

```rust
use pathmuster::inputs::{read_paths, InputFormat};

let paths = read_paths(std::io::stdin().lock(), InputFormat::Sha256sum)?;
let result = rules.check(paths.iter().map(|p| p.path()));
```

Proposed top-level items (the whole intended public surface for 0.1):

| Item | Kind | Purpose |
|---|---|---|
| `audit(entry, &AuditOptions) -> Result<Report, AuditError>` | fn | Run a project audit |
| `AuditOptions` | struct, builder | Root, targets, search paths, limits, symlink policy |
| `Report` | struct | References, findings, closure, summary; `Serialize` |
| `Reference`, `ReferenceKind`, `Target`, `Resolution` | types | See section 4 |
| `Finding`, `FindingCode`, `Severity`, `Evidence` | types | See section 9 |
| `Policy` | struct, builder | Severity overrides and fail threshold; decides pass or fail (section 9) |
| `Outcome` | enum | `Pass`, `Fail`, `Incomplete`; the overall result (section 9.1) |
| `Closure`, `ClosureEntry`, `IncompleteReason` | types | Closure and manifest writer |
| `RelPath` | newtype | Validated root-relative path |
| `rules::{RuleSet, PathCheck}` | module | Pure path rules |
| `inputs::{read_paths, InputFormat, InputPath}` | module | Path list and manifest readers |
| `AuditError`, `InputError` | enums | Fatal errors |

The adapter interface stays crate-private in 0.x. Adapters are selected by an
exported `ProjectFormat` enum (`Usd`, later `Godot`). A public plugin trait is
deferred until two in-tree adapters show what it needs.

### CLI

```text
pathmuster check <ENTRY> [--root DIR] [--target windows,macos,linux|all]
                [--search-path DIR]... [--format text|json] [--output FILE]
                [--fail-on error|warning|never] [--deny CODE]... [--warn CODE]...
                [--allow CODE]... [--no-follow-symlinks] [limits...]

pathmuster closure <ENTRY> [--root DIR] [--output FILE] [--allow-incomplete]
                [--allow-external] [--threads N] [same audit options]

pathmuster check-paths [FILE|-] [--input lines|nul|sha256sum|bagit|ocfl]
                [--target ...] [--format text|json] [--fail-on ...]
```

- `--root` defaults to the entry file's directory for USD and to the directory
  containing `project.godot` for Godot.
- `check` audits and reports. It never hashes.
- `closure` audits, then writes the `sha256sum` manifest to `--output` or stdout
  and the report to stderr. It writes no manifest when the audit or the closure
  is incomplete or the closure has external dependencies, unless the caller
  opts in with `--allow-incomplete` or `--allow-external`. These flags only
  permit writing a partial manifest; they never change the exit code of an
  incomplete audit (4).
- `check-paths` runs only the path rules. With no `--input`, the format is
  inferred from the file name (`manifest-*.txt` → BagIt, `inventory.json` → OCFL,
  `*.sha256`, `SHA256SUMS` → `sha256sum`) and otherwise defaults to `lines`.

Exit codes:

| Code | Meaning |
|---|---|
| 0 | Pass: complete audit; no finding at or above the fail threshold |
| 1 | Fail: complete audit; at least one finding at or above the threshold |
| 2 | Usage error: bad flags, unknown finding code, unreadable option value, or an attempt to remap a coverage-gap code |
| 3 | Audit could not run: entry missing or unreadable, root invalid, unsupported entry format, output not writable |
| 4 | Incomplete: the audit ran, but at least one coverage gap (section 9.1) means part of the input was not fully read. Never a pass. Takes precedence over 1; the report still lists every finding from the parts that were read |

Precedence when several apply: 2, then 3, then 4, then 1, then 0.

Examples (proposed):

```text
$ pathmuster check shot/scene.usda --target all
shot/scene.usda:12:31: error[absolute-path]: `C:\Users\ana\brick.png` is a Windows drive path
shot/scene.usda:14:24: error[outside-root]: `../../shared/metal.png` resolves outside the root
shot/layers/set.usda:8:30: error[case-mismatch]: `textures/Wall.png` matches `textures/wall.png` only case-insensitively (breaks on linux)
shot/layers/set.usda:9:30: error[missing]: `textures/floor.png` does not exist
closure: 3 layers, 4 assets, incomplete (1 missing)
audit: complete (every construct in 3 layers recognised)
4 errors, 0 warnings

$ git ls-files -z | pathmuster check-paths --input nul --target windows -
```

---

## 4. Domain model and invariants

### Core types

```rust
/// A validated path relative to the project root.
pub struct RelPath(String);

pub enum Target { Windows, MacOs, Linux }

pub struct Reference {
    pub id: ReferenceId,            // index into Report::references
    pub source: SourceLocation,     // file (RelPath), line, column, byte span
    pub kind: ReferenceKind,        // Sublayer, Reference, Payload, Asset, ClipAsset, ...
    pub raw: String,                // exactly as written, unescaped by the format's rules
    pub syntax: PathSyntax,         // lexical class of `raw`
    pub resolution: Resolution,     // what resolving `raw` found on disk
    pub context: Option<String>,    // e.g. "variantSet modelingVariant = \"High\""
}

pub enum PathSyntax {
    Relative { anchored: bool },     // "./a", "../a" are anchored; "a/b" is a search path in USD
    PosixAbsolute,                   // "/a/b"
    WindowsDrive,                    // "C:\a", "C:/a"
    WindowsDriveRelative,            // "C:a"
    Unc,                             // "\\server\share\a", "//server/share/a"
    WindowsDevice,                   // "\\?\...", "\\.\..."
    FileUri,                         // "file:///a"
    Uri { scheme: String },          // "http://...", "omniverse://...", "res://" in Godot is handled by the adapter
    HomeRelative,                    // "~/a"
    EnvironmentVariable,             // "$VAR/a", "${VAR}", "%VAR%"
    Expression,                      // USD backtick expression
    Empty,
}

pub enum Resolution {
    InsideRoot { path: RelPath },                 // resolved and inside the root
    OutsideRoot { path: std::path::PathBuf },     // resolved, but not under the root
    Missing { tried: Vec<std::path::PathBuf> },   // well-formed, nothing on disk
    Unresolvable { reason: Unresolvable },        // URI, env var, expression, pattern, device path
}

pub struct Closure {
    entries: Vec<ClosureEntry>,     // in-root files that must travel
    external: Vec<ExternalEntry>,   // resolved files outside the root
    incomplete: Vec<IncompleteReason>,
}
```

`ReferenceKind`, `PathSyntax`, `Unresolvable`, `IncompleteReason` and
`FindingCode` are `#[non_exhaustive]`.

### Invariants

| Type | Invariant | Enforced by |
|---|---|---|
| `RelPath` | UTF-8; `/` separators only; no leading `/`; no empty, `.` or `..` components; no NUL. Non-empty. | Private field; only constructor validates |
| `RelPath` | Byte-wise ordering is the canonical sort order | `Ord` derives from the string |
| `Reference` | Exactly one `Resolution` | Enum |
| `Reference` | `source.file` is a traversed layer inside the root | Constructed by the traversal only |
| `Resolution::InsideRoot` | The path exists, is a regular file (after allowed symlinks), and matches on-disk names exactly or through a recorded `case-mismatch`/`normalization-mismatch` finding | Resolver |
| `Closure.entries` | Sorted, unique, each a regular file inside the root, entry file included | Built from a `BTreeMap<RelPath, _>` |
| `Closure` | `is_complete()` is true iff `incomplete` is empty; when false, the report has a `closure-incomplete` finding | Constructor |
| `Report` | `outcome()` is `Incomplete` iff `coverage_gaps()` is non-empty, whatever the `Policy`; it is `Pass` only for a complete audit | `outcome` checks gaps before policy; `Policy` rejects gap codes |
| `Report` | References ordered by (source file, byte offset); findings ordered by (subject, code) | Final sort before return |
| `Report` | Contains no wall-clock time, host name or thread-dependent order | Review rule, determinism tests |

---

## 5. Modules and responsibility boundaries

A Cargo workspace with two packages:

```text
crates/pathmuster/          library: everything except argument parsing
  src/lib.rs               public re-exports, crate docs
  src/path/                RelPath, PathSyntax classification (pure, no I/O)
  src/rules/               target profiles, per-path checks, collision checks (pure)
  src/resolve/             filesystem resolution, exact-name lookup, directory cache, symlink policy
  src/adapter/mod.rs       crate-private Adapter trait, format detection
  src/adapter/usd/         .usda lexer and asset-path extraction
  src/adapter/godot/       (second slice) project.godot / .tscn / .tres / .import / .uid
  src/audit.rs             traversal worklist, cycle handling, limits, assembling Report
  src/closure.rs           Closure, hashing, sha256sum writer
  src/inputs/              lines, NUL, sha256sum, BagIt manifest, OCFL inventory readers
  src/report/              Report, Finding, Evidence, serde, text rendering
  src/error.rs             AuditError, InputError
crates/pathmuster-cli/      binary `pathmuster`: clap arguments, exit codes, output files
```

Boundaries:

- **`path` and `rules` are pure.** They take strings and return values. No
  filesystem, no environment, no clock. This is the reusable core and the place
  where most tests live.
- **`resolve` is the only module that touches the project filesystem** during
  an audit. `closure` reads file contents only when hashing.
- **Adapters only extract.** An adapter turns the bytes of one file into raw
  references with kind, span and a traversal hint (`Traverse` for layers,
  `Leaf` for assets). Adapters do not resolve, classify or recurse. The adapter
  supplies its resolution style (USD anchored/search-path, Godot `res://`), and
  `resolve` applies it.
- **`inputs` produce path strings**, not `RelPath`s, because the point is to
  find paths that are not valid.
- **The CLI owns policy defaults and process concerns** (exit codes, file
  output, terminal sanitization). The library returns data and never prints.

The library is one crate, not one crate per module. Splitting out the rule engine
(`pathmuster-paths`) is cheap later if someone wants it without the adapters.

---

## 6. Data flow and processing lifecycle

```text
 check / closure                                   check-paths
 ───────────────                                   ───────────
 entry file ──► detect format                      input file / stdin
                   │                                    │
                   ▼                                    ▼
 ┌──────── worklist of layers ◄────────┐          inputs::read_paths
 │  read (size-limited, regular file)  │                │
 │  adapter.extract(bytes) ─► raw refs │                │
 │  for each raw ref:                  │                │
 │    path::classify(raw)              │                │
 │    resolve(classified, layer dir)   │                │
 │    if Traverse and text layer ──────┘                │
 └──────────────┬──────────────────────                 │
                ▼                                       ▼
        collect closure set  ───────────────►  rules::check(paths)
        (entry + in-root targets)               per-path + set-level
                │                                       │
                ▼                                       ▼
        assemble Report (sort, summarize)        PathReport
                │
     ┌──────────┴──────────┐
     ▼                     ▼
 render text/JSON     closure: hash files (bounded pool) ─► sha256sum manifest
     │
 policy ─► exit code
```

Lifecycle of one audit:

1. **Validate options.** Canonicalize the root. Confirm the entry exists, is a
   regular file and lies inside the root. Failure is a fatal `AuditError`.
2. **Detect format** from content first (`#usda` header; `PXR-USDC` magic for
   crate files), then extension.
3. **Traverse** with an explicit worklist (no recursion), a visited set keyed by
   `RelPath` (or canonical path for out-of-root layers, which are recorded but
   not traversed by default), and counters for every limit.
4. **Extract, classify, resolve** each reference. Parse errors and
   unsupported constructs become located coverage-gap findings (section 9.1),
   which make the audit incomplete; the audit continues with the rest of the
   input.
5. **Build the closure** from the entry and every `InsideRoot` target. Record
   `OutsideRoot` targets as external entries.
6. **Run path rules** over the closure's paths and over the raw strings of
   every reference (so `C:\x\aux.png` is reported even though it cannot resolve).
7. **Assemble** the `Report`: sort, deduplicate findings, compute summary counts.
8. **Hash** (only `closure`): stream each closure file through SHA-256 on a
   bounded thread pool, then write the manifest in sorted order.
9. **Decide the outcome**: any coverage gap gives `Incomplete` (exit 4). Only
   for a complete audit does the CLI map severities through overrides and
   compare against `--fail-on` to choose `Pass` (0) or `Fail` (1).

---

## 7. Algorithms and implementation choices

### 7.1 Path syntax classification

A single pass over the raw string, checked in this order: empty; USD expression
(starts with a backtick); URI (`scheme:` per
[RFC 3986](https://www.rfc-editor.org/rfc/rfc3986#section-3.1) with a scheme of
two or more characters, so `C:` is not a scheme); Windows device (`\\?\`,
`\\.\`); UNC (`\\` or `//` followed by a host); Windows drive (`[A-Za-z]:` then a
separator, or drive-relative); POSIX absolute (`/`); home-relative (`~`);
environment variable (`$NAME`, `${NAME}`, `%NAME%`); otherwise relative. A
relative path is anchored when it starts with `./` or `../`. A relative path
containing `\` gets a `backslash-separator` finding; for resolution, `\` is
treated as a separator (the author's intent) and the finding records that only
Windows will honour it.

`file:` URIs are decoded to paths and then classified again, so
`file:///C:/x.png` is reported as a Windows drive path.

### 7.2 Exact-name resolution

The resolver never asks the host filesystem "does `a/B.png` exist?", because the
answer depends on the host's case sensitivity. Instead it walks components from
the base directory:

1. Lexically normalize the relative path (`a/./b/../c` → `a/c`), tracking whether
   the walk ever rises above the root (`outside-root`, even if a later component
   comes back in).
2. For each component, list the parent directory once (cached in a
   `HashMap<PathBuf, Arc<DirListing>>`) and look for an entry whose name is
   byte-identical.
3. If none, look for an entry equal under Unicode NFC normalization
   (`normalization-mismatch`), then under case folding (`case-mismatch`). If
   exactly one matches, continue with the on-disk name and record the finding,
   with the targets where the reference breaks. If several match, record
   `ambiguous-match` and stop.
4. Use `symlink_metadata` on the final entry and each directory. A symlink is
   followed only if its canonical target stays inside the root and
   `--no-follow-symlinks` is not set; it always gets a `symlink` warning, since
   symlinks do not survive many transfer methods. Only regular files are
   accepted as targets; FIFOs, sockets and devices are `not-a-regular-file`.

Absolute and outside-root paths are resolved the same way from their own base,
so a case mismatch is still reported for them.

### 7.3 USD resolution

This follows the documented behaviour of OpenUSD's
[`ArDefaultResolver`](https://openusd.org/release/api/class_ar_default_resolver.html):

- Anchored relative paths resolve against the directory of the layer that
  contains them.
- Search paths (relative, not anchored) are tried against the containing
  layer's directory first, then each `--search-path` in order. A target found
  only through a search path is accepted with a `resolved-via-search-path`
  warning, because the destination may not have the same search paths.
- Absolute paths resolve as themselves and always carry an `absolute-path`
  finding.
- No custom resolver behaviour, no `PXR_AR_DEFAULT_SEARCH_PATH` lookup unless
  the caller passes it explicitly (`--search-path-from-env` is a possible later
  flag), and no package-relative paths (`a.usdz[b.png]`) until `.usdz` support.

### 7.4 USD text extraction

The adapter is a hand-written lexer for the subset of the `.usda` grammar
needed to find asset paths and their syntactic context. It is not a USD parser
and builds no scene graph.

- Tokens: asset paths (`@...@` and `@@@...@@@`, with `\@@@` escape in the
  triple form), strings (single, double, triple-quoted, with escapes), comments
  (`#` to end of line; the comment forms of the reference lexer are matched and
  tested), identifiers, punctuation, numbers.
- A small state machine tracks bracket depth and the most recent metadata or
  property keyword, which gives each asset path its kind:
  - inside the layer's opening `( ... )` after `subLayers` → `Sublayer`;
  - after `references` or `payload`, including `prepend`, `append`, `add` list
    operations → `Reference` / `Payload`; after `delete` → recorded as
    `deleted-reference` (info) and not followed;
  - an attribute value of type `asset` or `asset[]`, including inside
    `timeSamples` → `Asset`;
  - inside a `clips` dictionary under `assetPaths` → `ClipAsset`; a
    `templateAssetPath` string → `unexpanded-pattern`;
  - inside `assetInfo` or `customData` → `Metadata` (reported, not part of the
    closure);
  - an asset path in any other context → kept as `Asset` (so the closure
    errs on the side of including it) **and** reported as
    `unsupported-construct`, because its meaning is not known.
- References inside `variantSet` blocks are extracted for every variant, with
  the variant named in `context`. The closure is the union across variants,
  because any variant may be selected at render time.
- A prim path suffix (`@./a.usda@</World/Set>`) is dropped from the file path
  and kept as context. Internal references (`</World/Set>` alone) produce no
  file reference.
- Asset paths containing `<UDIM>` or other `<...>` tokens are
  `unexpanded-pattern` in the initial slice.

Targets of `Sublayer`, `Reference` and `Payload` are traversed when they are
text layers (`.usda`, or `.usd` with a text header). Crate (`.usdc`, binary
`.usd`) and package (`.usdz`) layers are included in the closure as files but not
traversed, which adds an `untraversed-layer` reason to the closure. Other
extensions (for example `.abc`, `.mtlx`, `.vdb`) are leaves.

**Fail closed.** The reader never skips what it does not understand. Each of
the following is a coverage gap with the file, line and column where it
starts, and makes the audit `Incomplete` (section 9.1):

- a `#usda` version other than `1.x`, or a missing header in a file read as a
  text layer → `unsupported-construct`;
- a token or syntax form the lexer does not recognise, including comment forms
  not yet confirmed (Q8) → `unsupported-construct`;
- an asset path in a context outside the list above → `unsupported-construct`;
- an unterminated string, asset path or bracket → `parse-error`;
- USD expressions → `expression`; `<UDIM>` and clip templates →
  `unexpanded-pattern`; crate and package layers → `untraversed-layer`.

The same rule binds every adapter: Godot input outside its supported subset is
reported the same way.

### 7.5 Portability rules

Each target has a profile. Rules apply per component, per full path and per set.

| Rule | Windows | macOS | Linux |
|---|---|---|---|
| Forbidden characters | `< > : " / \ \| ? *` and U+0000–U+001F ([Win32 naming](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)) | `/`, NUL; `:` warned (Finder shows it as `/`) | `/`, NUL |
| Reserved names | `CON PRN AUX NUL COM0-9 LPT0-9` and superscript digit forms, with or without extension, case-insensitive | — | — |
| Trailing `.` or space | error | — | — |
| Component length | 255 UTF-16 units | 255 UTF-8 bytes | 255 bytes |
| Full path length | warn above `260 − 1 − root budget` UTF-16 units (budget default 40) | warn above 1024 bytes | warn above 4096 bytes |
| Case-insensitive collision | yes | yes (default APFS) | no |
| Normalization collision | no (NTFS keeps NFC and NFD distinct) | yes | no |
| Control characters, leading/trailing whitespace | warning on all targets |||
| Not valid UTF-8 (path inputs only) | error | error | info |

Normalization variants (a name not in NFC) get a `non-nfc-name` info finding on
every target, because they are a common source of mismatches between tools.

**Collisions** are computed per directory: build a key per name per target
(Windows: simple uppercase per character; macOS: NFD then case fold; Linux:
none) and group siblings by key. A group with more than one distinct name is a
`case-collision` or `normalization-collision` with every member listed. Distinct
directories whose names collide (`Tex/a.png`, `tex/b.png`) are reported as
`directory-case-collision`: the files survive on a case-insensitive target, but
their references disagree on case. Collision detection is order-independent and
runs in O(n log n) with a sort by `(parent key, name key)`.

Case-folding fidelity is an approximation of each filesystem's real tables; see
open question Q2.

### 7.6 Closure

`BTreeMap<RelPath, ClosureEntry>` collected during traversal: the entry file,
every traversed layer, every `InsideRoot` leaf. Each entry records which
references point to it. Hashing streams each file through `sha2::Sha256` with a
64 KiB buffer on a pool of `--threads` workers (default: available parallelism,
capped at 8). Results are written in map order after all hashes finish, so
output does not depend on scheduling. A file that cannot be read during hashing
becomes an `unreadable` finding and an incomplete reason.

---

## 8. File formats and compatibility

### Inputs

| Format | Reference | Handling |
|---|---|---|
| USD text layer | [OpenUSD](https://openusd.org/release/index.html) `.usda`, text `.usd` | Section 7.4. Detected by `#usda` header. Version line accepted for any `#usda 1.x` |
| USD crate / package | `.usdc`, binary `.usd`, `.usdz` | Detected by magic; leaf in the closure; `untraversed-layer` |
| Godot (second slice) | [Godot TSCN format](https://docs.godotengine.org/en/stable/contributing/development/file_formats/tscn.html), [UIDs in 4.4](https://godotengine.org/article/uid-changes-coming-to-godot-4-4/) | `[ext_resource ... path="res://..." uid="uid://..."]` in `.tscn`/`.tres` (`format=3`), `project.godot` values with `res://`, `.import` `source_file`, `.uid` sidecars. `res://` maps to the root, `user://` is `user-data-path` (info). `uid://` resolves through `.uid` files, `.import` files and resource headers; the binary `.godot/uid_cache.bin` is not read. Binary `.scn`/`.res` are leaves with `untraversed-layer` |
| Path list | — | UTF-8 lines, `\n` or `\r\n`; or NUL-separated (`--input nul`), which also accepts non-UTF-8 names (reported as `not-utf8`) |
| `sha256sum` file | [GNU coreutils](https://www.gnu.org/software/coreutils/manual/html_node/md5sum-invocation.html) | `<hex>  <path>` or `<hex> *<path>`; lines starting with `\` use backslash escapes for `\\` and `\n` (and `\r`, which current coreutils also escapes). Digest length is checked, not the content |
| BagIt manifest | [RFC 8493 §2.1.3](https://www.rfc-editor.org/rfc/rfc8493#section-2.1.3) | `manifest-<alg>.txt`: checksum, whitespace, path; `%0A`, `%0D`, `%25` decoded; paths checked as written (including the `data/` prefix) |
| OCFL inventory | [OCFL 1.1](https://ocfl.io/1.1/spec/) | `inventory.json`: logical paths of the `head` version's `state`. Content paths and other versions are deferred |

### Outputs

**Closure manifest.** GNU `sha256sum` text mode: lowercase hex, two spaces,
root-relative path with `/` separators, `\n` line endings, sorted byte-wise,
with the backslash-escape form for names containing `\` or newline. `cd root &&
sha256sum -c closure.sha256` verifies it with stock tools. No header or comment
lines, because `sha256sum -c` would reject them.

**JSON report.** UTF-8, object keys in a fixed order, arrays sorted as in
section 4. The top level carries `"schema": "pathmuster-report"` and an integer
`"schema_version": 1`. Within a schema version, changes are additive only (new
optional fields, new finding codes); removing or renaming a field raises the
version. Absolute paths appear only for the root and out-of-root targets.

**Text report.** `file:line:column: severity[code]: message`, then a summary.
Paths are root-relative. Control characters and terminal escape sequences in
names are shown escaped (`\x1b`), never written raw.

Compatibility commitments for 0.x: finding codes and JSON field names are stable
once released; CLI flags may change between minor versions with a changelog
entry.

---

## 9. Errors, findings and reporting

Two separate channels:

- **`AuditError`** (fatal, `Result::Err`): the audit cannot produce a report.
  Variants: `RootNotFound`, `EntryNotFound`, `EntryOutsideRoot`,
  `EntryNotRegularFile`, `UnsupportedFormat`, `EntryUnreadable(io::Error)`.
  `InputError` covers malformed standalone inputs where a line cannot be read at
  all (reported with line number). Both are `thiserror` enums with source
  chains.
- **`Finding`** (data): everything about the project, including unreadable or
  unparsable nested files. A broken project is a successful audit with findings.

```rust
pub struct Finding {
    pub code: FindingCode,          // stable kebab-case string, e.g. "case-mismatch"
    pub severity: Severity,         // Error, Warning, Info (default per code)
    pub subject: Subject,           // Reference(ReferenceId) | Path(String) | Paths(Vec<String>) | File(RelPath)
    pub targets: Vec<Target>,       // where it breaks; empty means all
    pub message: String,            // one sentence, no trailing period
    pub evidence: Evidence,         // structured, code-specific
}
```

Initial finding codes and default severities:

| Code | Severity | Evidence |
|---|---|---|
| `missing` | error | resolution candidates tried |
| `absolute-path` | error | syntax class; whether it exists here |
| `outside-root` | error | normalized target; exists here |
| `backslash-separator` | error (macOS, Linux) | raw string |
| `case-mismatch` | error (Linux) | requested name, on-disk name, directory |
| `normalization-mismatch` | error (Linux, Windows) | requested and on-disk forms (NFC/NFD) |
| `ambiguous-match` | error | candidate names |
| `case-collision` | error (Windows, macOS) | colliding names, directory |
| `normalization-collision` | error (macOS) | colliding names |
| `directory-case-collision` | warning | directory variants |
| `reserved-name` | error (Windows) | component, reserved word |
| `invalid-character` | error (per target) | component, character, offset |
| `trailing-dot-or-space` | error (Windows) | component |
| `component-too-long`, `path-too-long` | error / warning | measured length, limit, unit |
| `control-character` | warning | component, code point |
| `not-utf8` | error (Windows, macOS) | lossy display, raw bytes as hex |
| `non-nfc-name` | info | component |
| `uri-reference`, `environment-variable`, `device-path` | warning | raw string |
| `unsupported-construct` | error, gap | file, line, column, the construct, why it is outside the subset |
| `expression` | error, gap | raw string |
| `unexpanded-pattern` | error, gap | pattern |
| `resolved-via-search-path` | warning | search path used |
| `symlink` | warning | link and target |
| `not-a-regular-file` | error | file type |
| `parse-error`, `unreadable` | error, gap | file, position or I/O error kind |
| `untraversed-layer` | error, gap | layer, format |
| `closure-incomplete` | error | reasons |
| `limit-exceeded` | error, gap | limit name and value |

Policy (CLI): `--deny`, `--warn`, `--allow` remap a code's severity; `--fail-on`
sets the threshold (default `error`). Remapping changes the reported severity,
so JSON consumers see what the policy decided. The library exposes the same as
`Policy` for embedding. Codes marked "gap" cannot be remapped: passing one to
`--warn` or `--allow` is a usage error (exit 2).

### 9.1 Audit completeness

A **coverage gap** is any point where the audit could not fully read its
input: `unsupported-construct`, `parse-error`, `unreadable`,
`untraversed-layer`, `expression`, `unexpanded-pattern`, `limit-exceeded`. Each
gap is a finding with its location. `Report::coverage_gaps()` lists them.

The overall result is an `Outcome`:

| Outcome | Exit | Condition |
|---|---|---|
| `Pass` | 0 | No coverage gap, and no finding at or above the fail threshold |
| `Fail` | 1 | No coverage gap, and at least one finding at or above the threshold |
| `Incomplete` | 4 | At least one coverage gap, whatever the other findings or the policy |

A complete audit is a statement that every construct in every traversed file
was recognised. An unsupported construct can therefore never produce `Pass`:
the report says `incomplete` in the text summary
(`audit: incomplete (2 gaps)`) and in the JSON `outcome` field, and the exit
code is 4. `--fail-on never` affects only `Pass` versus `Fail`.

This is separate from closure completeness. A missing file is a defect the
audit found (`missing`, exit 1); the audit itself is complete. A gap means the
audit cannot vouch for what it did not read.

---

## 10. Performance and resources

- The work is I/O bound. Each directory touched is listed once and cached for
  the audit's lifetime; the cache is bounded by the number of directories
  reached, which the reference limit bounds.
- The lexer is linear in layer size and allocates only for extracted paths.
  Layers are read whole, up to `max_layer_bytes` (default 256 MiB); larger layers
  are `limit-exceeded`. Memory-mapping is not used (simpler, and avoids issues
  with files changing during the audit).
- Default limits: 10,000 layers, 1,000,000 references, traversal depth 256,
  256 MiB per layer. All are options and every hit is a finding.
- Rule checks are O(total path length) plus one sort for collisions.
- Hashing dominates `closure` runtime on large texture sets. It is parallel,
  streaming, and constant memory per worker (64 KiB buffer). `check` never
  hashes.
- No global state; two audits can run concurrently in one process.

---

## 11. Security and robustness

Project files and manifests are treated as untrusted.

- **No panics on input.** No `unwrap`, `expect`, unchecked indexing or
  arithmetic overflow on input-derived values in library code, enforced with
  clippy lints and fuzzing (section 13).
- **No unbounded recursion.** Traversal uses a worklist; the lexer is iterative;
  bracket depth is counted and capped.
- **Bounded memory.** Size limits per layer and per input line (default 64 KiB
  per path); reference and layer count limits.
- **No network or remote filesystem access.** URIs are never fetched. UNC and
  Windows device paths are never opened, so a hostile project cannot make a
  Windows host connect to an SMB server and leak credentials.
- **No blocking on special files.** File type is checked with
  `symlink_metadata` before opening; only regular files are read, so a FIFO
  named as a texture cannot hang the audit.
- **Symlink containment.** Symlinks are followed only when their canonical
  target is inside the root (and following is enabled). Out-of-root targets are
  classified, not traversed.
- **No code execution or expansion.** No environment variable, `~` or USD
  expression is expanded; Godot scripts are not run.
- **Output safety.** Text output escapes control characters. The manifest
  writer uses the `sha256sum` escape form, so a name containing a newline
  cannot inject a manifest line.
- **Read-only.** The library opens project files for reading only. The CLI
  writes only to `--output` paths and refuses to write into the audited root
  unless the path is given explicitly.
- **Races.** Files can change during an audit. Results describe what was read;
  hashing re-checks file type and reports changes as `unreadable` rather than
  following a swapped-in symlink.
- `#![forbid(unsafe_code)]` in both crates.

---

## 12. Dependencies and alternatives

Kept small. All must pass `cargo deny` licence and advisory checks.

| Need | Choice | Alternatives considered |
|---|---|---|
| Unicode NFC/NFD | [`unicode-normalization`](https://crates.io/crates/unicode-normalization) | `icu_normalizer` (ICU4X): more complete, larger data and dependency tree |
| Case folding | std `char::to_uppercase` / `to_lowercase` with single-character mapping (Q2) | `caseless`, `icu_casemap`: full folding; adopt only if fixture evidence shows the approximation misses real collisions |
| SHA-256 | [`sha2`](https://crates.io/crates/sha2) (RustCrypto) | `ring`, `aws-lc-rs`: faster but C/assembly build dependencies |
| Serialization | `serde`, `serde_json` | none reasonable |
| Errors | `thiserror` | hand-written `Display` impls |
| Parallel hashing | `rayon` with a local, sized pool | `std::thread::scope` with a shared index: no dependency, slightly more code. Either is acceptable |
| CLI | `clap` (derive), CLI crate only | `lexopt`, `pico-args`: smaller, but no generated help or completions |
| USD text | in-house lexer (section 7.4) | [`openusd`](https://crates.io/crates/openusd) crate: pre-1.0, broader scope, unclear whether it exposes source spans; see Q1 |
| Godot text | in-house parser for `[section key=value]` headers | none maintained for Godot 4 text resources |
| BagIt / OCFL | in-house readers (a few dozen lines each) | Rust BagIt crates exist but are small and not actively maintained |

Dev dependencies: `proptest`, `insta` (JSON and text snapshots), `assert_cmd`,
`predicates`, `tempfile`. Fuzzing with `cargo-fuzz` in a separate, unpublished
`fuzz/` package.

---

## 13. Testing strategy

**Unit tests (pure modules).** Table-driven tests for path syntax
classification (drive, drive-relative, UNC, device, `file:` URIs, schemes vs.
drive letters, env vars), every rule on every target (`CON`, `con.txt`, `COM¹`,
`aux.exr`, `trailing.`, `trailing `, `a:b.png`, U+0007, 255 vs 256 units in
UTF-8 and UTF-16), `RelPath` validation, and the `sha256sum`, BagIt and OCFL
readers.

**Property tests (`proptest`).**

- `RelPath` normalization is idempotent.
- Writing any `RelPath` set as a `sha256sum` manifest and reading it back gives
  the same set (escape round trip); same for BagIt percent-encoding.
- Collision results are invariant under input permutation.
- Report output is identical for any permutation of directory listing order
  (resolver fed a shuffled fake listing).
- The USD lexer and every input reader return `Ok` or a finding, never panic,
  on arbitrary bytes.
- Inserting comments and whitespace between tokens of a valid layer does not
  change the extracted references.

**Fixture tests.** Small synthetic projects under `tests/fixtures/`, created for
this repository (no third-party assets). The core USD fixture covers one case per
classification: resolved in-root, missing, POSIX absolute, Windows drive,
outside-root, case mismatch, search-path-only, crate layer, UDIM pattern,
variant-only reference, deleted reference, cycle between two sublayers. Expected
reports are `insta` snapshots.

Fixtures that cannot be committed portably (case-colliding pairs, `CON.txt`,
trailing dots, NFC/NFD pairs) are generated at test time in a temporary
directory and skipped, with a message, on hosts where they cannot be created. The
repository's own file list is checked with `pathmuster check-paths` in CI, so the
repository stays portable.

**Integration tests.** `assert_cmd` tests for each subcommand, exit code and
output format; a test that runs the system `sha256sum -c` on a generated
closure when the tool is present.

**Oracle tests (optional CI job).** A container with OpenUSD's Python package
runs `UsdUtils.ComputeAllDependencies` on the USD fixtures and compares the sets
of layers and assets with the closure, for fixtures where both should agree.
OpenUSD is a test oracle only, never a dependency.

**Fuzzing.** `cargo-fuzz` targets for the USD lexer, the Godot parser and the
manifest readers; run for a fixed time in a scheduled CI job.

**Cross-platform CI.** Linux, macOS and Windows runners, so the exact-name
resolver is exercised on case-insensitive filesystems.

---

## 14. Initial usable implementation slice

Version 0.1 is useful on its own for USD pipelines and for anyone moving file
trees between operating systems.

1. `path`, `rules`: all three target profiles, per-component, per-path and
   collision checks, with unit and property tests.
2. `inputs` and `check-paths`: lines, NUL, `sha256sum`, BagIt manifest, OCFL
   head-version logical paths.
3. `resolve`: exact-name lookup, directory cache, symlink policy, search paths.
4. USD adapter: text layers; sublayers, references, payloads, `asset`/`asset[]`
   values and time samples, clip `assetPaths`, variants; crate and package
   layers as untraversed leaves.
5. `audit`, `closure`: worklist traversal with limits, closure, `sha256sum`
   writer.
6. Reports: text and JSON (`schema_version` 1), policy and exit codes.
7. CLI: `check`, `closure`, `check-paths`.
8. Tests: everything in section 13 except the oracle job and fuzzing, which
   follow immediately after.
9. Documentation: crate docs, `docs/findings.md` (one entry per code, with
   example and fix), `docs/report-format.md`.

Acceptance: on the core USD fixture, `pathmuster check` reports each planted
defect with the correct code and location, the closure lists exactly the
expected files, `sha256sum -c` accepts the generated manifest, and the results
match on Linux, macOS and Windows CI runners. A second fixture containing
constructs outside the supported subset produces located
`unsupported-construct` findings, `audit: incomplete` and exit code 4, under
every policy setting.

Second slice, in order: Godot adapter; BagIt export of the closure (a new bag
with `data/` payload, `manifest-sha256.txt`, `bagit.txt`, `bag-info.txt`
including `Payload-Oxum`, `tagmanifest-sha256.txt`); SARIF 2.1.0 output; oracle
job and fuzzing in CI.

---

## 15. Deferred functionality and non-goals

**Deferred (planned once the initial slices are in use):**

- `.usdc` crate parsing and `.usdz` package inspection (package-relative paths).
- UDIM tile expansion, value-clip templates, USD expression evaluation.
- Dependencies inside non-USD leaves (MaterialX documents, OSL shaders).
- OCFL content paths and all versions; Godot `export_presets.cfg` filters.
- Non-mutating relink suggestions ("`Wall.png` → `wall.png`"); later, perhaps, an
  opt-in rewrite to relative paths with a dry run.
- A configuration file (`pathmuster.toml`) for policy; flags only until users ask.
- A public adapter trait.

**Non-goals:**

- Parsing proprietary or undocumented formats (`.blend`, `.ma`/`.mb`, `.hip`,
  `.nk`, Premiere, Resolve, Unreal, Unity scenes). A tool that exports its own
  dependency list as JSON or text can be checked with `check-paths`.
- Fetching URIs or calling custom USD asset resolvers.
- Verifying checksums. `sha256sum -c` and BagIt validators already do this; the
  closure is written so they can.
- A new manifest format. The closure is `sha256sum`; the archival export is
  BagIt.
- Copying or packaging projects (other than the BagIt export) or capturing
  runtime environments (ReproZip, Nix, container images solve that).
- Checking that web links exist (link checkers such as lychee do that).
- Detecting file content corruption or validating image or geometry data.

---

## 16. Open technical questions

Each has a recommended default so implementation can start without further
investigation. Defaults are revisited only if implementation evidence
contradicts them.

| # | Question | Recommended default |
|---|---|---|
| Q1 | Use the `openusd` crate for `.usda` parsing, or the in-house lexer? | **In-house lexer.** The needed subset is lexical (asset paths are `@`-delimited), source spans are required for evidence, and it avoids a pre-1.0 dependency whose per-file licensing needs checking. Re-evaluate `openusd` when `.usdc` support starts, since the crate format is binary and not worth re-implementing |
| Q2 | How faithfully to model each filesystem's case comparison (NTFS upcase table, APFS folding, HFS+ tables)? | **Documented approximation:** Windows = per-character simple uppercase; macOS = NFD then case fold. Collect real-filesystem fixtures on CI runners; switch to full folding (`icu_casemap`) only if a missed collision is found. Edge cases (`ß`, Turkish dotted I, Greek final sigma) are listed in `docs/findings.md` |
| Q3 | Treat Windows reserved names with extensions (`CON.txt`) as errors, given that recent Windows versions relaxed some cases? | **Error for all forms.** Older Windows versions, archive tools and many applications still fail on them |
| Q4 | Windows path-length check, given the destination root is unknown? | **Warning** when the root-relative length exceeds `259 − root budget` UTF-16 units, budget default 40, flag `--windows-root-budget` |
| Q5 | Follow in-root symlinks for resolution and closure? | **Follow, with a `symlink` warning.** `--no-follow-symlinks` makes them `not-a-regular-file` |
| Q6 | Should `closure` fail when external (out-of-root) files exist? | **Fail (exit 1)** unless `--allow-external`; external files are listed in the report, never in the manifest |
| Q7 | A case-mismatched reference: is its target part of the closure? | **Yes.** The file exists and must travel; the reference gets a `case-mismatch` error. The closure is complete for that file |
| Q8 | Which comment forms does the `.usda` text grammar accept? | Match the OpenUSD text-format lexer and pin the behaviour with tests that the oracle job cross-checks. Until confirmed, accept `#` line comments only; any other comment form is reported as `unsupported-construct`, so an unexpected form makes the audit incomplete instead of hiding or inventing a reference |
| Q9 | `<UDIM>` in the first slice? | **Defer;** report `unexpanded-pattern`, a coverage gap, so the audit is `Incomplete` (exit 4) and the closure incomplete. Tile expansion (globbing `1001`–`1999`) is small and comes right after the first slice if early users need it |
| Q10 | Unicode tables differ between Rust toolchains and crate versions. Record them? | **Yes:** the JSON report's `tool` section records the Unicode version used, so differences between runs are explainable |
| Q11 | Godot `uid://` resolution when `.godot/` is absent (not committed)? | Build the map from `.uid` sidecars, `.import` files and resource headers. If a `uid://` cannot be mapped but the `path=` fallback resolves, report a warning, matching Godot's own fallback |
| Q12 | Single crate with a `cli` feature, or library and CLI as separate packages? | **Separate packages** in one workspace, so library users never compile `clap` |

---

## 17. Rust quality expectations

- **Edition 2024; MSRV 1.85**, stated in `Cargo.toml` (`rust-version`) and
  tested in CI. Raised only in a minor release, with a changelog entry.
- `#![forbid(unsafe_code)]` in both crates. No exception is expected.
- `cargo clippy --all-targets -- -D warnings` in CI, plus `clippy::unwrap_used`,
  `clippy::expect_used` and `clippy::indexing_slicing` denied in library
  non-test code.
- `cargo fmt --check` in CI.
- `#![deny(missing_docs)]` on the library: every public item is documented, with
  an example where useful.
- `cargo deny check` for licences, advisories, duplicate versions and sources.
- No panics on untrusted input (section 11), backed by property tests and
  fuzzing.
- Public enums that may grow are `#[non_exhaustive]`; the public API is checked
  with `cargo semver-checks` once 0.1 is released.
- No `println!` or logging from the library; diagnostics are returned as data.
