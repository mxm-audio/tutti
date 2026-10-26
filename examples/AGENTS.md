# AGENTS.md — examples

# Purpose

Example projects and the conformance fixtures: `minimal/`, `typical/` and `every-feature/`
(project folders, each with tiny synthesised audio), and `conformance/<level>/<case>/` for
Levels 0, 1 and 2 (`l0`, `l1`, `l2`) with an `expected.json` each, `conformance/hostile/<case>/`
for what a reader must refuse, two of them projects as one file (`<Name>.tutti.zip`, written by
the generator's own stored-zip writer so a fixture can declare any length). Level 1's `plugin`
case, a reference CLAP, is not here yet: it is to be built with the first program that claims
Level 1 (`spec/1.0/conformance.md` 4).

# Ownership

Owns every folder here. **Every project here is generated**, never edited by hand: `cargo run -p
tutti-schema --bin make-examples` writes them all from the crate itself (deterministic ids, audio
from closed forms, hashes and fingerprints real), so a change to the format is a change to the
generator, then a run. **A new case goes after the ones before it** in the generator: the ids
count up across every case, so one put in the middle rewrites every case after it. A case added
to an earlier level is generated after Level 2's, at the end of `main` (Level 1's
`render-choice`), and filed in its own level's folder.

# Local Contracts

- **Examples are tests.** Every project here validates against the schema, parses into the
  types, writes back byte-identical in the pretty form, and passes `validate`
  (`tutti-schema`'s tests); an invalid case fails at the pointer its `expected.json` names.
- **Audio here is made, never taken**: a sine, a click, noise, written by a test or a script in
  this repository; no sample pack's loop and nothing from a user's project. A file several
  cases use is the same bytes in each (git keeps it once).
- A fixture's expected result is a table (event times in seconds and samples, RMS within a
  tolerance, values at ticks), never a reference render, so engines may differ in their DSP and
  still pass. **A table is computed by the generator from the spec's arithmetic** (the core's
  tempo map and placement, closed forms for RMS), and its times are the document's: a test
  holds every tick and its seconds to the document's tempo map.
- **A table asks only what the spec pins.** Where it leaves something open, a case is built
  around it rather than guessing: tracks sound in their own seconds instead of soloed through a
  return, a stereo tone at the centre where a pan law would enter, a lane measured only between
  its first point and its last, a static gain of 1 under a gain lane. A fictitious plugin
  (`org.example.*`) stands where no real one may: what it is given is a table, not a sound.
- **A link is made by the runner, never committed**: git keeps no link on every system
  (Windows checks one out as a file). A case that needs one says so in its `expected.json`
  (`link`: `path` and `target`, the target relative to the link's folder), and every runner
  (`tutti-schema`'s and `tutti-cli`'s tests, the JavaScript tests) checks it in a copy where the
  link is made: a junction on Windows, which needs no privilege.
- A case may carry files no reader reads beside its project: `symlink-audio`'s `link-target/`,
  `merge-different-tracks`'s `branches/` (the two branches' documents and their merge).
- **A render here is exactly its range** (spec 7.7) and made at a low rate where its sound is
  not what is tested (every-feature's renders at 8 kHz), so the files stay small; a case whose
  table measures RMS (`missing-plugin`, `render-choice`) renders at the fixtures' rate.

# Work Guidance

# Verification

- `cargo test -p tutti-schema`; `cargo test -p tutti-cli` (`tutti check` over every folder
  here); the JavaScript package's `npm test`.

# Child DOX Index

None.
