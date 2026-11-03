# AGENTS.md — crates/tutti-polyend

# Purpose

Polyend Tracker projects into tutti, behind `tutti-convert`'s `Format` (`PolyendTracker`):
`files` reads the Tracker's files and writes them back byte for byte; `model` reads a project
folder (`project.mt`, `patterns/`, `instruments/*.pti`, or the older `instruments/*.mti` with
`samples/instrNN.wav`) into a `Model` and `Locators` (where each
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
  `Error::Unreadable` naming the file and its number, never a guess; the numbers are exactly those
  a Tracker Mini's card carries, the factory songs of every firmware included. The older layout
  (`.mti` and `samples/`) is read, its older versions as the firmware reads them: a project
  before 10 has its pad root note reset to 24, an instrument of version 1 numbers its play modes
  from 5 on one lower, older volumes are converted, and patterns 1 to 3 are translated when
  played (`song::played_tracks`); the bytes stay as read. A sample no instrument names is refused,
  and so is an instrument file whose name gives no number (`NN name.pti`, or the device's own
  `instrument_NN.pti`).
- **The device's rules, as `song`'s module doc states them**: a step is a sixteenth; every track
  plays track 1's length; Break Pattern ends a pattern; a note rings to its track's next note,
  `OFF`, `CUT` or `FADE`, across patterns; micro-move and velocity, the first effect slot, then
  the second, then the step's own field; Tempo sets the tempo map; version 4 clears the MIDI
  tracks. A rule changed here is a change of `convert::VERSION`.
- **The conversion** (`convert`'s module doc): a folder track per Polyend track, a track per
  instrument in it, a part per slot, patterns shared only where a slot's notes are identical
  (else variants), a note outlasting its pattern keeping its length with its part lengthened
  over it (tutti 6.6). A Polyend track playing one instrument is that instrument's track; one
  playing several, a folder of them. A sample instrument is an MXM Creative Sampler device: its
  sample a WAV in the device's own files, what maps of it a parameter snapshot (tutti 7.3, by
  nice-plug's CLAP ids): the sample at its own pitch on note 60, transpose, start, end (a slice
  instrument's selected slice), loop mode and points, the reader, pan, the amplitude envelope's
  ADSR, the filter (type, cutoff and Q as the firmware plays them), the bit depth (the
  sampler's converter); the master full, the gain
  following velocity exactly, and Init's cutoff routes off. The rest kept, and named in the
  report. A beat-slice instrument's notes play their slices as audio pieces of its sample in
  `audio/`. **Volumes are the firmware's tables** (`files::amp_db`, `files::mix_db`; its own
  decibels are 10·log, half these): a Polyend track's volume and the dry mix's its track's gain
  (on each track, not the master, so effect returns would not be scaled), and each note's
  gain, its step's volume or else its instrument's, split by `convert::Levels` into the
  sampler's level, the note's velocity and the instrument track's gain, so level × velocity ×
  gain is the note's gain, a boost past unity included. A program builds the sampler's state
  from the two until the sampler loads its own files (MXM tutti Converter does, with the
  sampler's state crate). Ids are derived from the source with a token of the project's name.
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

The tests (`tests/it/made.rs`): every file written back; the pattern sizes the firmware knows;
unknown versions and a stray sample refused by name; the older layout read, translated, converted
and read back byte for byte; detection, the card's `Workspace/` left out; a step's ticks and every way a note
ends; micro-move and velocity's order; track 1's length and Break; tempo changes and version 4's
cleared tracks; a missing pattern; the conversion's folders, devices, samples, variants and
lengthened parts; the same document twice; the model read back byte for byte, samples included;
the volume tables and the dry mix; levels, envelopes, filters, bit depth, slices and beat slices
as the Tracker plays them;
instrument files placed by number or refused.
`tests/it/corpus.rs`: every real project read, written back, converted, valid, and read back (a
Tracker Mini's card: all 28, and its `Workspace/` copied as a project, 2026-11-04).

# Child DOX Index

None.
