# AGENTS.md — crates/tutti-convert

# Purpose

The converters' interface: what a crate converting another project format into tutti, or out of
it, implements (`Format`), reads through (`Source`, and `Recording`, which notes exactly what a
conversion read), and gives back (`Conversion`, `Export`, a `Report` of what did not map); and the
one writer, `write`, which puts a conversion into any store through the core's `Project`, after
checking that every file lands where the document refers to it; `write_export` for the other
way. `derived_id` gives the ids a conversion derives from its source, so converting an unchanged
project again writes the same document; `fingerprint` the document's identity a watcher records.
Its first consumers are MXM tutti Converter (`mxm-audio/mxm-tutti-converter`, the app that
watches folders) and the format crates beside it (the owner, 2026-11-03).

# Ownership

Owns `Cargo.toml`, `src/` and `tests/`.

# Local Contracts

- **No file IO, no platform code**: a source and a store are traits the host implements, so the
  crate, and every format built on it, builds for `wasm32-unknown-unknown`. Its tests go through
  `tutti-fs` for the disk.
- **A source is read-only by its type**: `Source` is the core's `Folder` and one listing,
  `entries` (not `list`, so code with `Store` in scope never meets two methods of one name);
  every `Store` is one by a blanket impl. A new operation a source needs goes into the core's
  `Folder` first, as `tutti-fs`'s `AGENTS.md` says of a store's.
- **A conversion is a value**, the document and its files with where each goes, so a format is
  object safe and IO-free; how it is written is `write`'s, once, for every format. A format never
  writes a file itself.
- **`write` refuses before it writes**: an output holding a document or any file outside
  `private/` (a program's own folder, which a watcher fills first and `write` leaves as it is);
  two files to one place, ignoring case; an audio name the core would change, or one no audio
  piece refers to; a device file of no device or on an unsafe path; a state no device refers to.
  Each refusal is tested with nothing written. Device files and states are put whole (the core's
  `DeviceFiles::put` takes bytes); audio streams.
- **`Recording` holds a conversion to what it read**: each file's SHA-256 and size are of the
  bytes that passed, read or copied, and a file giving different bytes at a second read is
  refused there and makes `served` an `Error::Changed`.
- **`derived_id` is readable when it can be, never ambiguous**: `prefix_a_b` only when no part is
  empty or holds `_` or a character an id may not, else `prefix_<hash>`; always well formed.
- **`fingerprint` is the bytes `tutti::equal` compares**: the JCS with `generator` left out.
- `Format::from_tutti` keeps its name beside `to_tutti`, with clippy's `wrong_self_convention`
  allowed on it alone: it converts, it does not construct.
- Dependencies: `tutti`, `serde` (the report and `Served` are serialisable) and `sha2`, pinned
  by the workspace; MSRV the core's (1.87), checked with `cargo +1.87.0 build -p tutti-convert`
  since the tests' `tutti-fs` needs more.

# Work Guidance

- A format is its own crate (`tutti-<format>`), depending on this one; this crate never names a
  format.

# Verification

```bash
cargo test -p tutti-convert
cargo clippy -p tutti-convert --all-targets -- -D warnings
cargo +1.87.0 build -p tutti-convert
cargo build -p tutti-convert --target wasm32-unknown-unknown
```

The tests, through a test format (`Lines`: a note list and a made sample, put as an audio file,
a device's own file and a state): detection and inputs; a conversion written into memory and on
disk, opened again and valid, the same document twice; the fingerprint leaving the generator out
and nothing else; what a recording notes, and a file changing between two reads refused; an
output holding a file refused and a private file kept; each misplaced file refused with nothing
written; a format's defaults `Unsupported`; a cancelled write; derived ids; an export written
and refused when misplaced; a report through serde.

# Child DOX Index

None.
