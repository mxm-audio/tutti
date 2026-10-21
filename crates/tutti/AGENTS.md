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
  each one a point where a crash can happen, which `Project` composes; `Folder::copy_to`
  copies a file into a writer (whole by default, streamed where the store overrides it), which
  `tutti-zip` packs through. The crate builds for `wasm32-unknown-unknown` unchanged, and CI
  proves it.
- **What `Project::save` guarantees is stated in `project.rs`'s module doc, exactly**, and
  nowhere wider: every newly referenced file present, else a refusal before the document is
  touched; the index built from the files (a file's length and modification time read at every
  save and compared with the record of its last verification, the hash computed again when
  either differs and never otherwise; the records kept between sessions by the program,
  `verified` and `trust`); a file the index already listed that went missing or changed kept in
  the index as it was and reported in `Saved::warnings` (a save never leaves a project less
  complete than it found it, nor blesses a change); an entry no known field claims kept while
  the document mentions the file (by path, name or hash) and dropped when nothing does or on
  purpose (`forget_file`); `dependencies` in step with the devices (an entry for every device's
  plugin, the least the spec asks where the program gave none; a plugin's entry no device uses
  dropped unless the document names it elsewhere); the document valid, the README current, the
  document renamed into
  place, and a failure after that rename reported as `Unconfirmed`, the save being on disk and
  the hash the new one. `save_verified` hashes everything again and refuses a missing or changed
  file, for an export.
- **The model's constructors carry the spec's defaults** (`Device::new` on, `Note::new` released
  at 0.5, `TempoPoint::new` held, `Pattern::new`, `Part::new`, `MeterPoint::new`,
  `Dependency::plugin`), and `Document::track` and `track_mut` find a track through folders: a
  writer never restates a default the spec sets (the owner, 2026-10-21, when the player became
  the second consumer).
- **A device's own files are `Project::device_files`'s** (spec 7.8): put, list and read by
  their path inside `plugins/files/<device id>/`, safety required and portability advised
  (`validate::check_device_file_path`), indexed by the device's id at the save and dropped
  from the index with the device; `copy_device_files` for a duplicate.
- **A recording streams through `Project::new_audio`'s handle**, a `Write` that hashes as the
  bytes pass into the store's own writer; it borrows nothing from the project, and is `Send`
  exactly when the store is. The crate never deletes a file of the project, one the document
  does or could refer to; it removes only what it made itself and nothing names, an unfinished
  recording when its handle is dropped and a stale temporary file before writing a new one.
  A program's cleanup consults `Project::unnamed` before touching `audio/`.
- **Dependencies: serde, serde_json, sha2**, pinned; nothing else without a reason in `NOTES.md`.
- **MSRV 1.87**, compiled against (`cargo +1.87.0 test -p tutti`) whenever the crate takes a
  language feature it did not have before: Cargo never checks a crate's own `rust-version`.
- **Every struct keeps what it doesn't know**: a flattened map of unknown keys and an
  `extensions` map, written back as read. `unknown_keys` finds them through the document's own
  serialisation (`unknown.rs`): an unknown key serialises as a `Key` newtype, which JSON writes
  as its string and a field's name never is, so a new struct needs nothing there.
- **`equal` leaves `generator` out** (spec 3.1; the owner, 2026-10-21): it names the last
  program that wrote the document, so another program's write-back is still equal. A number the spec states is a constant here, held equal
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
