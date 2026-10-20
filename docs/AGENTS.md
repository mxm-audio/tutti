# AGENTS.md — docs

# Purpose

What is not normative but needed: `research.md` (the research the format was designed from,
with its sources), `implementers.md` (the guide), `agents.md` (what an agent reads first, with
`llms.txt` beside it at the repository root), `field-reference.md` (generated from the schema),
and later `dawproject-mapping.md`.

# Ownership

Owns every file here but `field-reference.md`'s content, which `tutti-schema`'s generator owns.

# Local Contracts

- `field-reference.md` is generated (`cargo run -p tutti-schema --bin fields`); a test fails when
  the committed file differs. Edit the schema, not the reference.
- A fact in `research.md` carries its source; a judgement is marked as one.

# Work Guidance

# Verification

- `cargo test -p tutti-schema` (the field reference is current).

# Child DOX Index

None.
