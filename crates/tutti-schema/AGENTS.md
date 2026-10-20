# AGENTS.md — crates/tutti-schema

# Purpose

The tests that hold the spec, the schema, the examples, the docs and the code together, and the
generator of `docs/field-reference.md`. Not a library anyone ships.

# Ownership

Owns `Cargo.toml`, `src/` and `tests/`. `src/bin/make_examples.rs` is the generator of
`examples/` (every example project and the Level 0 fixtures with their `expected.json`), the one
place with file IO besides the command: it uses the core for ids, the pretty form, hashes,
fingerprints and the time arithmetic the fixtures' expected values come from.

# Local Contracts

- **The validator is a dev-dependency only**, so its toolchain floor never reaches a program that
  takes `tutti`.
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
