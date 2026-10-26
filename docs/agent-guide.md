# tutti for agents

You are a program, or an agent writing one, that has found a folder with a `.tutti` file in it,
or has been asked to write one. This page is what to read first. The normative text is
`spec/1.0/tutti.md`; the schema is `schema/1.0/tutti.schema.json`; this page only points.

## Reading a project

1. The folder holds exactly one `.tutti` file at the top. It is JSON. Its first keys say what it
   is: `format` (`"tutti"`), `version` (`"1.0"`), `about`, `spec`, `schema`, `generator`.
2. `README.md` beside it explains the folder in about 2 kB. Read it if you are new.
3. Parse the document as I-JSON: refuse duplicate keys; numbers are finite; integers fit in a
   double. Accept any key order and whitespace.
4. Apply defaults: a key that is absent has the default the spec names (`tutti.md` 4).
5. Resolve ids: every reference is an id string in a key named for its target (`pattern`,
   `destination`, `to`, `device`, `scene`). A reference that resolves to nothing is reported, not
   fatal.
6. Keep what you don't understand. Unknown keys, unknown `type` values and every `extensions`
   value stay in your in-memory tree and go back out when you write.

Validate with the reference tools before trusting your own reader: `tutti check <folder>` (the
`tutti` crate's command) or the npm package's `check`.

## The time model, in one paragraph

Positions and lengths are integers in **ticks**, `ppq` per quarter note (`ppq` is in the
document; 960 is usual). The **tempo map** `tempo: [{at, bpm, curve}]` turns ticks into seconds;
`curve` is `hold` or `linear`, and the spec gives the closed form (`tutti.md` 5.2), with test
vectors in `examples/conformance/l0/ramp`. **Audio** is placed by `map: [{frame, tick}]` points
and a `bpm`: outside the points the file runs at its own speed at that bpm, so at another project
tempo it is stretched, pitch kept (`tutti.md` 5.5). Never store seconds; derive them.

## The model, in one paragraph

A document has `tracks` (a tree through `children`); a track is its channel (`gain` linear, `pan`,
`mute`, `solo`, `destination`, `sends`), has `devices` in signal order (each with a `role`, a
`plugin` id like `clap:dk.mxm.mxm-mono-01`, a `state` file by hash, a `params` snapshot), owns
its `patterns` (events: `note`, `chord`, `param`) and places them with `parts` (`at`, `length`,
`offset`, `loop`), and has `automation` lanes. A note with `audio` sounds frames of a file in
`audio/` instead of the instrument. A track and a pattern may carry `renders`: audio of what
some of the track's devices made, each listing those `devices`. `markers`, `range` (the span the
user marks: looped over, exported), `master`, `scenes` and `metadata` are at the top.
Every object may carry `extensions` under a reverse-DNS namespace.

## Writing a project

- **From Rust, let the crate do it**: `tutti_fs::create(folder, title, generator)` or
  `tutti_fs::open(folder)` gives a `Project`; put files in with `new_audio` (a writer to
  stream a recording through), `put_audio`, `put_state`, `put_render`; edit the document;
  `save(generator)`. It names and hashes the files, fills the index, validates, writes the
  README and renames the document into place. Everything below is what it does, for a
  program that writes JSON itself.
- Ids: strings matching `^[A-Za-z0-9_-]{1,64}$`, unique across the document. Make yours as
  `<kind>_<ULID>` (`trk_01JB2K7Q9M3F5A7Y9R2E4W6N`). Never change an id you read.
- Write the pretty form: two-space indent, keys in the spec's order, events and points one per
  line, defaults left out, numbers rendered as JavaScript renders them. The `tutti` crate and the
  npm package do this for you; if you write JSON yourself, run `tutti check` on the result. It
  notes every key it doesn't know, which is how a misspelt key shows: one is kept, never refused.
- Put every file you refer to inside the folder first (`audio/`, `plugins/states/`,
  `renders/`), then write the document to a temporary name and rename it into place.
- Fill `files` (path, sha256, bytes), `dependencies` (every plugin you host, with its version
  and where it comes from), `extensions_used`, `generator`. Write `README.md` from the template.
- Never rename or overwrite a file in `audio/`. Name new files with `A-Z a-z 0-9 - _` and one
  dot.

The smallest valid document:

```json
{
  "format": "tutti",
  "version": "1.0",
  "generator": { "name": "my-daw" },
  "ppq": 960,
  "tempo": [ { "at": 0, "bpm": 120 } ],
  "meter": [ { "at": 0, "numerator": 4, "denominator": 4 } ]
}
```

A track with one pattern played once, one note:

```json
"tracks": [
  { "id": "trk_01JB2K7Q9M3F5A7Y9R2E4W6N", "name": "Lead",
    "devices": [ { "id": "dev_01JB2K7T1A0P2M7R4W9Y1Z3B", "role": "instrument",
                   "plugin": "clap:dk.mxm.mxm-mono-01", "name": "mxm-mono-01" } ],
    "patterns": [
      { "id": "pat_01JB2K7UB8N5T7X9A2C4E6G8J0", "name": "A", "length": 3840,
        "events": [
          { "type": "note", "id": "evt_01JB2K7V0Q1S3U5W7Y9A1C3E5G", "at": 0, "length": 960, "pitch": 60, "velocity": 0.8 }
        ] }
    ],
    "parts": [
      { "id": "prt_01JB2K7VQ2M4O6Q8S0U2W4Y6A8", "pattern": "pat_01JB2K7UB8N5T7X9A2C4E6G8J0", "at": 0, "length": 3840 }
    ] }
]
```

## Common tasks

- **Add a note**: append an event to the pattern's `events` with a fresh id; keep the array
  sorted by id when you write.
- **Place audio**: copy the file into `audio/` under a free name, compute its SHA-256, add a
  pattern with one `note` carrying `audio` (`file`, `sha256`, `sample_rate`, `channels`,
  `frames`, `from` 0, `to` frames, `map: [{frame: 0, tick: 0}]`, `bpm` the material's tempo or
  `null`), a part placing it, and a `files` entry.
- **Find "the second verse"**: `markers` with `kind: section` name the ranges; a part's `at` and
  `length` say what plays in them.
- **Change a plugin parameter**: a device's `params` entry by `id` (plain `value`); automate it
  with a lane whose `target` is `{"device": id, "param": id}` and plain values, as the
  entry's `value`; never normalized.
- **Hand a project to another program**: nothing to do; the folder is the hand-off. To send it
  as one file, `tutti zip <folder>` makes `<Name>.tutti.zip` (the document first and stored,
  every entry's name and length checked by the reader; spec 2.5) and `tutti unzip` makes the
  folder again. Never leave the folder.
- **Carry stems with it, as a backup**: render each track over the whole song, from tick 0 to
  past its last part with room for what rings on, the same span for every track, before the
  fader, exactly that long, into `renders/`, and add to the track's `renders` the file,
  its hash, its facts (`sample_rate`, `channels`, `frames`), `devices` (what is in it: the
  crate's `without_effects` or `with_effects`, or your own list), `from`, `to` and the
  `fingerprint` (`render_fingerprint`); a pattern's `renders` hold a pattern alone at a `bpm`
  (spec 7.7). `tutti inspect` then lists each one with the devices in it and left out, whether it
  can stand in for the chain, and whether it is still current.

## Pitfalls

- Ticks, not seconds, not beats as floats. Convert through the tempo map.
- `velocity` and `release` are 0 to 1, not 0 to 127. `gain` is linear, not dB.
- Ids are strings, never array indexes, never renumbered.
- A pattern belongs to one track; a part may only play a pattern of its own track.
- Dropping a key you did not understand is a bug. Writing a key at its default is not a bug, but
  the pretty form leaves it out.
- `plugins/states/<sha256>.<kind>`: feed a state only to a plugin of the same kind.
- A render is exactly its range: `frames` is the range's samples, nothing more; a tail is a
  longer range. Its `devices` say what is in it; a render without effects still holds the
  instrument. No two renders share a file.
- `plugins/files/<device id>/`: a plugin's own files, any kind, nested as it likes; every one in
  the index under that path; the index entries go with the device, the folder is the program's
  to remove (spec 7.8).
