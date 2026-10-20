# AGENTS.md — examples

# Purpose

Example projects and the conformance fixtures: `minimal/`, `typical/` and `every-feature/`
(project folders, each with tiny synthesised audio), and `conformance/<level>/<case>/` with an
`expected.json` each, `conformance/hostile/<case>/` for what a reader must refuse.

# Ownership

Owns every folder here. **Every project here is generated**, never edited by hand: `cargo run -p
tutti-schema --bin make-examples` writes them all from the crate itself (deterministic ids, audio
from closed forms, hashes and fingerprints real), so a change to the format is a change to the
generator, then a run.

# Local Contracts

- **Examples are tests.** Every project here validates against the schema, parses into the
  types, writes back byte-identical in the pretty form, and passes `validate`
  (`tutti-schema`'s tests); an invalid case fails at the pointer its `expected.json` names.
- **Audio here is made, never taken**: a sine, a click, noise, written by a test or a script in
  this repository; no sample pack's loop and nothing from a user's project.
- A fixture's expected result is a table (event times in seconds and samples, RMS within a
  tolerance), never a reference render, so engines may differ in their DSP and still pass.

# Work Guidance

# Verification

- `cargo test -p tutti-schema`; `tutti check` over every folder here.

# Child DOX Index

None.
