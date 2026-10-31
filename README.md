# PathMuster

**Check whether a project can move to another computer, operating system or
render node without losing the files it depends on.**

The crate and binary are named `pathmuster`. The name is not yet published on
crates.io.

> **Status: design stage.** There is no code yet. This repository contains the
> proposed design for review. Every command, API and output shown below is a
> proposal, not working software.

## The problem

Scene files and game projects refer to other files: layers, textures, caches,
scripts. A project that renders correctly on the machine where it was made
can break when it is copied somewhere else, because some of those references:

- are absolute paths (`C:\Users\ana\brick.png`, `/Users/ana/lib/metal.png`) that
  exist only on the original machine;
- point outside the project folder (`../../shared/metal.png`), so they are not
  copied with it;
- use Windows-only syntax, such as drive letters or backslashes, and fail on
  Linux or macOS;
- differ in letter case from the file on disk (`Wall.png` vs `wall.png`), which
  works on Windows and macOS and fails on Linux;
- name files that cannot exist on the destination: `aux.exr` or `a:b.png` on
  Windows, `Tex.png` next to `tex.png` on a case-insensitive filesystem, or the
  same accented name stored in two Unicode forms.

These failures are usually silent until render time, often on a different
machine from the one where the mistake was made. Existing tools either need
the full authoring application installed or report which paths resolve today
without saying why a resolved path will not survive a move.

## Who it is for

- **Artists and technical directors** who hand USD scenes to a render farm or a
  collaborator and want a pre-submit check.
- **Pipeline and build engineers** who want a CI gate that fails when a
  committed scene or project references files it should not.
- **Game developers** whose project works in the editor on Windows but breaks in
  a Linux build because of a case mismatch (Godot support is planned next).
- **Anyone moving file trees between operating systems**, including archivists
  working with BagIt or OCFL packages, who need to know which file names will not
  survive the trip.

## Proposed usage

> Proposed. Nothing below runs yet.

Audit a USD scene before submitting it:

```text
$ pathmuster check shot/scene.usda --target all
shot/scene.usda:12:31: error[absolute-path]: `C:\Users\ana\brick.png` is a Windows drive path
shot/scene.usda:14:24: error[outside-root]: `../../shared/metal.png` resolves outside the root
shot/layers/set.usda:8:30: error[case-mismatch]: `textures/Wall.png` matches `textures/wall.png` only case-insensitively (breaks on linux)
shot/layers/set.usda:9:30: error[missing]: `textures/floor.png` does not exist
closure: 3 layers, 4 assets, incomplete (1 missing)
4 errors, 0 warnings
$ echo $?
1
```

Write the set of files that must travel with the scene, as a standard
`sha256sum` manifest that stock tools can verify at the destination:

```text
$ pathmuster closure shot/scene.usda --output shot.sha256
$ cd shot && sha256sum -c ../shot.sha256
```

Check file names without any project format, for example whether a Git
repository will check out cleanly on Windows:

```text
$ git ls-files -z | pathmuster check-paths --input nul --target windows -
$ pathmuster check-paths bag/manifest-sha256.txt --target all
```

From Rust:

```rust
// Proposed API.
use pathmuster::{audit, AuditOptions, Target};

let options = AuditOptions::new("shot").targets(&[Target::Windows, Target::Linux]);
let report = audit("shot/scene.usda", &options)?;
for finding in report.findings() {
    eprintln!("{finding}");
}
```

## Planned capabilities

- **Reference extraction** from USD text layers (`.usda`): sublayers,
  references, payloads, asset-valued attributes, value clips and variants.
  Godot 4 text projects next.
- **Resolution with evidence**: each reference is classified as resolved,
  missing, absolute, outside the project, Windows-only, case-mismatched or
  unresolvable (URIs, environment variables), with the file, line and column
  where it was written. Results do not depend on the case sensitivity of the
  machine running the check.
- **Portable-path rules** for Windows, macOS and Linux: reserved names, invalid
  characters, trailing dots and spaces, length limits, case and Unicode
  normalization collisions.
- **Standalone path checking** of plain path lists, `sha256sum` files, BagIt
  manifests and OCFL inventories.
- **Dependency closure** written as a `sha256sum` manifest, with a clear
  statement of whether it is complete. BagIt export is planned.
- **Reports** as readable text or versioned JSON, with stable finding codes and
  CI-friendly exit codes.

## Non-goals

- Parsing proprietary or undocumented formats (`.blend`, Maya, Houdini, Nuke,
  Premiere, Resolve, Unreal, Unity). Tools that can export their own dependency
  list can be checked with `check-paths`.
- Modifying projects. The audit is read-only; relinking is not planned for the
  first releases.
- Fetching URIs or running custom USD asset resolvers.
- Verifying checksums: `sha256sum -c` and BagIt validators already do that.
- Inventing a new manifest format.
- Packaging projects or capturing runtime environments.

## Project maturity

| Area | State |
|---|---|
| Architecture | Proposed, under review |
| Library and CLI | Not started |
| USD adapter | Designed, not implemented |
| Godot adapter | Outlined, not implemented |
| Releases | None. Not published on crates.io |
| Licence | Not yet chosen |

## Technical overview

A Rust library with a small public API and a separate command-line binary.

- A **pure path-rule engine** (no filesystem access) classifies path syntax and
  applies per-platform rules. It is usable on its own.
- A **resolver** finds each referenced file by comparing directory entries
  exactly, so case and Unicode mismatches are detected on any host. It never
  follows URIs, never opens network or device paths, and only follows symlinks
  that stay inside the project.
- Small **format adapters** extract references from documented text formats;
  they do not resolve or interpret scenes. USD support uses a narrow lexer for
  asset paths rather than a full USD implementation.
- The **closure** is a sorted, deduplicated set of project-relative files,
  hashed in parallel and written as `sha256sum` text.
- Output is **deterministic**: the same inputs give byte-identical reports and
  manifests.

Planned engineering standards: stable Rust (MSRV 1.85), no `unsafe` code,
clippy with warnings denied, rustfmt, documented public items, `cargo deny` for
dependency licences and advisories, and no panics on untrusted input, backed by
property tests and fuzzing.

## Design

The full proposed design, including the data model, finding codes, file
formats, testing strategy and open questions, is in
[docs/architecture.md](docs/architecture.md).
