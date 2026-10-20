# AGENTS.md — spec

# Purpose

The normative text of the format, one folder per version: `spec/<version>/tutti.md` (the
format), `conformance.md` (the levels: what a reader and a writer each must do), `extensions.md`
(namespaces, the registry of known extensions, used and required, deprecation into core) and
`README.template.md` (the README every writer puts in a project folder). `CHANGELOG.md` lists the
versions.

# Ownership

Owns `CHANGELOG.md` and every `spec/<version>/` folder.

# Local Contracts

- **A declared version is frozen.** Nothing in `spec/<version>/` changes after the version is
  declared, but an `ERRATA.md` beside the text; the next version is drafted in its own folder,
  marked *draft* in its heading until declared. A document's `version` then always names an
  immutable text, and a program in 2032 reads what 1.0 meant.
- **The spec states its own numbers.** It is the normative source (as a design system is), so a
  limit, a default or a formula is written here in full; the schema carries it as a constraint,
  the code as a constant, and `tutti-schema`'s tests hold the three equal.
- **One way to do each thing.** Where two encodings could mean the same, the spec names one and
  the reader accepts only that; a fallback is named for every enum.
- **A change to the format starts here**, then the schema, the examples, the crates, in one
  change.

# Work Guidance

- Write for the implementer who has never seen a DAW's internals: every unit, every order, every
  fallback, and a worked example with numbers where arithmetic is involved.

# Verification

- `cargo test -p tutti-schema`: the spec's version equals the schema's and the crate's; the
  numbers the spec states equal the constants.

# Child DOX Index

None.
