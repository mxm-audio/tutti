# AGENTS.md — crates/tutti

# Purpose

The format's reference implementation: the model (the types a document deserialises into,
unknown fields and extensions kept), reading a document in any form, writing the canonical pretty
form and the JCS (RFC 8785) bytes equality and fingerprints are over, `validate` (what the schema
can't say), and the arithmetic the spec defines: ticks to seconds through the tempo map and back,
an audio piece's placement and playback rate, a render's fingerprint.

# Ownership

Owns `Cargo.toml` and `src/`.

# Local Contracts

- **No file IO, no platform code.** Documents are bytes in and bytes out; anything that lists or
  reads a folder goes through a trait the caller implements. The crate builds for
  `wasm32-unknown-unknown` unchanged, and CI proves it.
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
