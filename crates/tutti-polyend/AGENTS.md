# AGENTS.md — crates/tutti-polyend

# Purpose

Polyend Tracker projects into tutti, behind `tutti-convert`'s `Format` (`PolyendTracker`):
`files` reads the Tracker's files and writes them back byte for byte; `model` reads a project
folder (`project.mt`, `patterns/`, `instruments/*.pti`) into a `Model` and `Locators` (where each
instrument's samples are, beside the model, so two models compare equal whatever source they
came from); `song` works out the song as the device plays it; `convert` makes the tutti project
and, with `to_model`, reads the model back from it. The layouts and the sequencer's rules are
the Tracker Community Edition firmware's (`polyend/tracker-ce`, read for them on the owner's
word, 2026-11-03: "Ignore the license just make it"), checked against a Tracker Mini's card; the
code is this crate's own.

# Ownership

Owns `Cargo.toml`, `src/` and `tests/`.

# Local Contracts

- **Every byte is kept.** `files` parses what a conversion reads and keeps every other byte
  (headers, reserved fields, CRCs, a project file's whole body, an instrument's struct), and
  `to_bytes` writes each file back exactly: the tests hold it for every made file and, by hand,
  for a card's projects.
- **Versions bounded by fixtures**: `files::PROJECT_VERSIONS`, `PATTERN_VERSIONS` and
  `INSTRUMENT_VERSIONS` list the numbers read, each carried by a fixture; any other is
  `Error::Unreadable` naming the file and its number, never a guess. The older layout
  (`instruments/*.mti`, `samples/`) is refused by name (the owner, 2026-11-03: no projects of
  theirs are in it).
- **The device's rules, as `song`'s module doc states them**: a step is a sixteenth; every track
  plays track 1's length; Break Pattern ends a pattern; a note rings to its track's next note,
  `OFF`, `CUT` or `FADE`, across patterns; micro-move and velocity, the first effect slot, then
  the second, then the step's own field; Tempo sets the tempo map; version 4 clears the MIDI
  tracks. A rule changed here is a change of `convert::VERSION`.
- **The conversion** (`convert`'s module doc): a folder track per Polyend track, a track per
  instrument in it, a part per slot, patterns shared only where a slot's notes are identical
  (else variants), a note outlasting its pattern keeping its length with its part lengthened
  over it (tutti 6.6). A sample instrument is an MXM Creative Sampler device, its sample a WAV in
  the device's own files; its settings are kept, not yet given to the sampler (that mapping is
  the owner's to review first). Ids are derived from the source with a token of the project's
  name.
- **Nothing lost**: what tutti does not hold is kept under `dk.mxm.polyend-tracker` (registered
  in `spec/1.0/extensions.md`) on the object it belongs to, and `to_model(to_tutti(m))` is `m`,
  every sample's bytes too. Raise `convert::VERSION` whenever the output changes, so watchers
  convert again.
- **No file IO**: everything through `tutti-convert`'s `Source`; builds for
  `wasm32-unknown-unknown`. Dependencies: `tutti`, `tutti-convert`, `serde`, `serde_json`, pinned
  by the workspace; MSRV the core's.
- **No one's song is committed**: the tests build their projects byte by byte; real projects are
  read only by the ignored corpus test, from a folder named by `POLYEND_CORPUS`.

# Work Guidance

- A new file version: its fixture first (a made file in `tests/it/made.rs`, and a real one in the
  corpus), then its number in the list.

# Verification

```bash
cargo test -p tutti-polyend
cargo clippy -p tutti-polyend --all-targets -- -D warnings
cargo +1.87.0 build -p tutti-polyend
cargo build -p tutti-polyend --target wasm32-unknown-unknown
POLYEND_CORPUS=<a card's Projects/, copied> cargo test -p tutti-polyend -- --ignored   # by hand
```

The tests (`tests/it/made.rs`): every file written back; unknown versions and the older layout
refused by name; detection, the card's `Workspace/` left out; a step's ticks and every way a note
ends; micro-move and velocity's order; track 1's length and Break; tempo changes and version 4's
cleared tracks; a missing pattern; the conversion's folders, devices, samples, variants and
lengthened parts; the same document twice; the model read back byte for byte, samples included.
`tests/it/corpus.rs`: every real project read, written back, converted, valid, and read back.

# Child DOX Index

None.
