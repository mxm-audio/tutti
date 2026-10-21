# AGENTS.md — crates/tutti-schema

# Purpose

The tests that hold the spec, the schema, the examples, the docs and the code together, and the
generator of `docs/field-reference.md`. Not a library anyone ships.

# Ownership

Owns `Cargo.toml`, `src/` and `tests/`. `src/bin/make_examples.rs` is the generator of
`examples/` (every example project and the Level 0 fixtures with their `expected.json`). It
writes each example as any writer does, the files into a `tutti-fs` store and the document
through `Project::save`, which indexes and verifies them and writes the README; only the
hostile fixtures and the `expected.json` files are written raw. It uses the core for ids,
hashes, fingerprints and the time arithmetic the fixtures' expected values come from.

# Local Contracts

- **The validator is a dev-dependency only** (`jsonschema`, pinned, no default features: the
  schema refers to nothing outside itself), so its toolchain floor never reaches a program that
  takes `tutti`; this crate's own floor is the validator's, not the core's.
- `src/bin/fields.rs` prints `docs/field-reference.md` from the schema; `src/lib.rs` holds what
  the tests and the generators share: the schema as a value, the repository's root, the
  field-reference generator.
- The tests: every example valid, parsed, written back byte-identical, `validate` passing; every
  invalid fixture failing at its pointer; the every-feature example walked against the schema
  both ways (no key the schema lacks, no property the example never uses); the spec's version,
  the schema's `$id` and `tutti::VERSION` equal; the constants in code equal the schema's
  constraints; the committed field reference equal to the generator's output.

# Work Guidance

# Verification

- `cargo test -p tutti-schema`.

# Child DOX Index

None.
