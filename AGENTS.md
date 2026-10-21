# AGENTS.md — tutti

DOX rail for this repository: the purpose, the rules every part keeps, and the Child DOX Index.
The reasoning and the owner's dated rulings are in [NOTES.md](NOTES.md); the research the format
was designed from is [docs/research.md](docs/research.md).

# Purpose

**tutti, an open audio project format** (the owner, 2026-10-20): a project is a plain folder
holding one JSON document, `<Name>.tutti`, and every file it uses, so that a project started in
one program opens in another, including programs that don't exist yet. The format is
interoperable (every unit and rule stated), expandable (namespaced extensions, unknown data kept
and written back) and self-documenting (the document names its format, version, spec and schema
in its first lines; every project folder carries a short README; the spec ships with a schema,
examples and conformance fixtures). This repository holds the spec, the schema, the examples, the
docs, the reference crates and, later, the JavaScript package and the converters.
[newDAWn](https://github.com/maxmcorp/newDAWn) is its first consumer; it takes the `tutti` crate
as a git dependency on `main`, locked to a commit, as it takes mxm-kit.

Pre-alpha: nothing is released, there are no tags, and the spec's first version is a draft.

# Ownership

Root owns `Cargo.toml`, `Cargo.lock`, `LICENSE`, `README.md`, `CONTRIBUTING.md`, `NOTES.md`,
`llms.txt`, `.gitattributes` and `.github/`. Each folder with an `AGENTS.md` owns its contents;
the index is below.

# Local Contracts

## DOX

- `AGENTS.md` files are binding work contracts for their subtrees. Before editing, read this file
  and every `AGENTS.md` from here down to each path you touch; the nearest controls local detail,
  and no child may weaken a parent. After editing, update the closest owning `AGENTS.md` and any
  parent or index it affects when purpose, structure, contracts, workflows or the owner's
  preferences changed; correct stale text, moving the reasoning still in force to `NOTES.md`, and
  delete a discarded idea outright (git keeps it).
- Sections, in order: Purpose · Ownership · Local Contracts · Work Guidance · Verification · Child
  DOX Index. Keep each file under about 200 lines. A value the code or the spec holds is named,
  not copied: state the rule, the constant and the test, never the number, except in the spec
  itself, which is the normative source of its numbers.

## The format

- **The spec is the source of truth**, frozen per version (`spec/AGENTS.md`); the schema is
  hand-written and normative (`schema/AGENTS.md`); the crates implement them and are tested
  against them; the examples are tests. A change to the format lands in all of them at once.
- **A reader never refuses a project for data it doesn't understand**, keeps it, and writes it
  back; a writer emits the canonical pretty form. Equality and fingerprints are over RFC 8785.
- **The format declares, a program resolves**: dependencies (which plugins, which version, where
  from), extensions required, conformance levels. The format never needs this workspace, and its
  docs never send an implementer here.

## The code

- **Licence MIT** for everything here (`LICENSE`): code, schema, spec text and examples, so any
  program, open or closed, can read and write the format. No trademark on the name.
- **Dependencies are pinned exactly and `Cargo.lock` is committed.** The core (`crates/tutti`)
  takes serde, serde_json and sha2 and nothing else; it has no file IO and no platform code, and
  builds for `wasm32-unknown-unknown` (CI proves it). **File IO lives in `crates/tutti-fs`**, the
  disk store and `open`/`create`, nowhere else: the command, the example generator and the
  tests go through it, and the save procedure itself is the core's `Project`, which `tutti-fs`
  only lends a disk to (the owner, 2026-10-21: an implementer never writes file-system code
  against the spec).
- **MSRV is per crate, compiled against**: the core at 1.87 (`cargo +1.87.0 test -p tutti` when its
  language use changes); a crate that needs more says so in its `Cargo.toml` and `AGENTS.md`.
- **Windows, macOS and Linux, and the web.** Work and verify on Windows; the other platforms are
  checked in one batch when the owner asks (CI runs only by hand).
- **No tags** (the owner, 2026-10-07): dependents follow `main`, locked; a spec version is a frozen
  folder, never a tag.

# Work Guidance

- The stages and their checks are the plan this repository was made from (newDAWn's
  `NOTES.md`, 2026-10-20); each stage ends with its DOX pass.
- Write the spec for an implementer who has never seen a DAW's internals, with worked examples
  wherever arithmetic is involved.

# Verification

```bash
cargo fmt --all -- --check
cargo clippy -p <crate> --all-targets -- -D warnings   # each crate the change reaches
cargo test -p <crate>                                   # likewise; the workspace only when it reaches all
cargo build -p tutti --target wasm32-unknown-unknown    # when the core's dependencies change
```

Test a minimum, on Windows, before a push (the owner, 2026-10-06). A docs-only change gets no
build.

# Child DOX Index

| Doc | Scope |
|---|---|
| [`spec/AGENTS.md`](spec/AGENTS.md) | The normative text, one frozen folder per version, and the changelog |
| [`schema/AGENTS.md`](schema/AGENTS.md) | The JSON Schema, hand-written, frozen with its version |
| [`examples/AGENTS.md`](examples/AGENTS.md) | Example projects and the conformance fixtures, with made audio only |
| [`docs/AGENTS.md`](docs/AGENTS.md) | The research, the implementers' guide, the agent guide (`agent-guide.md`), the generated field reference |
| [`crates/tutti/AGENTS.md`](crates/tutti/AGENTS.md) | The reference implementation: model, read, write, validate, the time arithmetic, `Project` (the save procedure over a store trait); no file IO |
| [`crates/tutti-fs/AGENTS.md`](crates/tutti-fs/AGENTS.md) | tutti on disk: the store over `std::fs`, `open` and `create`; the one place with file IO |
| [`crates/tutti-schema/AGENTS.md`](crates/tutti-schema/AGENTS.md) | The tests that hold spec, schema, examples, docs and code together; the field-reference generator |
| [`crates/tutti-cli/AGENTS.md`](crates/tutti-cli/AGENTS.md) | The `tutti` command: check, inspect, deps, archive, fmt, over `tutti-fs` |
| [`crates/tutti-wasm/AGENTS.md`](crates/tutti-wasm/AGENTS.md) | The core for JavaScript: wasm-bindgen functions, strings in and out, no file IO |
| [`js/AGENTS.md`](js/AGENTS.md) | `@mxm-audio/tutti`, the npm package: the wasm build, the wrapper, the generated document types, the Node tests over the examples |
