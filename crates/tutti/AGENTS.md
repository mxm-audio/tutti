# AGENTS.md — crates/tutti

# Purpose

The format's reference implementation: the model (the types a document deserialises into,
unknown fields and extensions kept), reading a document in any form, writing the canonical pretty
form and the JCS (RFC 8785) bytes equality and fingerprints are over, `validate` (what the schema
can't say), the arithmetic the spec defines (ticks to seconds through the tempo map and back,
an audio piece's placement and playback rate, a render's fingerprint), and `Project`: a project
in a store, opened, created, saved and filled by the one procedure every writer follows (spec
10, conformance 2), written once over the `Store` trait so the disk (`tutti-fs`), a test's
memory (`MemoryStore`) and a JavaScript host run the same one.

# Ownership

Owns `Cargo.toml` and `src/`.

# Local Contracts

- **No file IO, no platform code.** Documents are bytes in and bytes out; a folder is a trait the
  host implements: `folder::Folder` to read, `folder::Store` to write, primitive operations
  each one a point where a crash can happen, which `Project` composes. The crate builds for
  `wasm32-unknown-unknown` unchanged, and CI proves it.
- **What `Project::save` guarantees is stated in `project.rs`'s module doc, exactly**, and
  nowhere wider: every referenced file present, the index built from the files (a file's
  length and modification time read at every save and compared with the record of its last
  verification, the hash computed again when either differs and never otherwise, a
  disagreement refusing before the document is touched; the records kept between sessions by
  the program, `verified` and `trust`), an entry no known field claims kept while the document mentions the file (by path,
  name or hash) and dropped when nothing does, the document valid, the README current, the
  document renamed into place, and a failure after that rename reported as `Unconfirmed`, the
  save being on disk and the hash the new one. `save_verified` rehashes everything, for an
  export. The owner decides whether every save rehashes (the plan, stage 12; `NOTES.md`).
- **A recording streams through `Project::new_audio`'s handle**, a `Write` that hashes as the
  bytes pass into the store's own writer; it borrows nothing from the project, and is `Send`
  exactly when the store is. The crate never deletes a file; a program's cleanup consults
  `Project::unnamed` before touching `audio/`.
- **Dependencies: serde, serde_json, sha2**, pinned; nothing else without a reason in `NOTES.md`.
- **MSRV 1.87**, compiled against (`cargo +1.87.0 test -p tutti`) whenever the crate takes a
  language feature it did not have before: Cargo never checks a crate's own `rust-version`.
- **Every struct keeps what it doesn't know**: a flattened map of unknown keys and an
  `extensions` map, written back as read. A number the spec states is a constant here, held equal
  to the schema's by `tutti-schema`'s tests; never copy such a number into docs.
- **Reading normalises, writing canonicalises**: a reader accepts any key order and whitespace
  and sorts what the spec says is sorted; a writer emits the pretty form exactly.

# Work Guidance

# Verification

```bash
cargo test -p tutti
cargo clippy -p tutti --all-targets -- -D warnings
cargo build -p tutti --target wasm32-unknown-unknown
```

# Child DOX Index

None.
