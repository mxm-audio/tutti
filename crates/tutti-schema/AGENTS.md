# AGENTS.md — crates/tutti-schema

# Purpose

The tests that hold the spec, the schema, the examples, the docs and the code together, and the
generator of `docs/field-reference.md`. Not a library anyone ships.

# Ownership

Owns `Cargo.toml`, `src/` and `tests/`. `src/bin/make_examples.rs` is the generator of
`examples/` (every example project and the conformance fixtures of Levels 0, 1 and 2 and the
hostile ones, with their `expected.json`). It writes each example as any writer does, the files
into a `tutti-fs` store and the document through `Project::save`, which indexes and verifies
them and writes the README; only the hostile fixtures, the `expected.json` files and a Level 2
case's branch documents are written raw. It uses the core for ids, hashes, fingerprints and the
time arithmetic the fixtures' expected values come from, and closed forms for their RMS.

# Local Contracts

- **The validator is a dev-dependency only** (`jsonschema`, pinned, no default features: the
  schema refers to nothing outside itself), so its toolchain floor never reaches a program that
  takes `tutti`; this crate's own floor is the validator's, not the core's.
- `src/bin/fields.rs` prints `docs/field-reference.md` from the schema; `src/lib.rs` holds what
  the tests and the generators share: the schema as a value, the repository's root, the
  field-reference generator, a document's ids and its shared patterns (what Level 2 keeps).
- The tests: every example valid, parsed, written back byte-identical, `validate` passing; every
  invalid fixture failing at its pointer, a case with a `link` in a copy where the link is made
  (what the link reaches untouched); every fixture needing the level it is filed under (Level 2
  at most); every tick and its seconds in a table following the document's tempo map;
  `missing-plugin`'s render current and standing in, its device kept, an edit going into
  overrides only and the render stale after; `render-choice`'s renders current, exactly their
  range, their fingerprints the published ones, the render played the one that stands in for
  both devices, an effect edit staling only the render that holds it and a note edit all three;
  `folder-notes`' instruments given what its table lists (the folders' notes with the track's
  own), its renders current with their published fingerprints, a folder note edit staling all
  three and a layer's own note edit only its own;
  `everything` written back by another program equal, its ids, sharing,
  index and README as its table says; `merge-different-tracks`' branches its edits made to the
  base, and `git merge-file` merging them cleanly into its merged document (git is needed);
  the every-feature example walked against the schema both ways (no key the schema lacks, no
  property the example never uses), and its renders against their own two definitions, since by
  name alone a render's keys hide among other objects'; the spec's version, the schema's `$id` and
  `tutti::VERSION` equal; the constants in code equal the schema's constraints; the committed
  field reference equal to the generator's output.
- **Scratch folders are the build folder's** (`CARGO_TARGET_TMPDIR`), not the system's.

# Work Guidance

# Verification

- `cargo test -p tutti-schema`.

# Child DOX Index

None.
