# tutti

**An open audio project format.** A project is a plain folder: one JSON document that says in
its first lines what it is and where its rules are, the audio it plays, the states of the plugins
it uses, and a short README. Start a project in one program and open it in another, including
programs that don't exist yet.

## Why a shared format

> In a future where everybody vibe-codes their own apps, yet still wants to work together on
> projects, and where humans and agents work together on projects, the project format is the
> common denominator.

Every program that edits audio on a timeline has its own project format, and none of them opens
in any other. That was bearable while a handful of companies wrote the programs. It isn't any
more. Soon everybody writes their own: a producer makes the sequencer they always wanted, an
agent writes a tool for one job, a studio builds its own pipeline. People and agents still want
to work on the same project, in different programs, at the same time. The one thing all of them
have to agree on is the project format, and it has to exist before the programs do.

tutti is that format. It is designed so that:

- **A project outlives the program that made it.** It is a folder of plain files. The document
  names its own format, version, spec and schema in its first lines, and the folder carries a
  README that explains itself. A program written in ten years, by someone who never heard of
  this repository, can read the folder and learn the rules from it.
- **Two programs never fight over it.** Every unit and every rule is written down: what a tick
  is, how a tempo ramp integrates to seconds, where frame 0 of an audio file sits, what a gain
  of 1.0 means. Two independent implementations agree on where every note falls, to the sample
  under a held tempo and within one under a ramp.
- **No program loses another's work.** What a reader doesn't understand it keeps and writes
  back. Your program's own data lives under your own namespace, on any object, and every other
  program carries it through.
- **Agents are first-class readers and writers.** JSON with a published JSON Schema is what
  structured output generates against; the folder's README, the spec and `docs/agent-guide.md` are
  what an agent reads first.

## Why not make your own

Because a project format is where the hard problems hide, and they surface years later, in other
people's projects. Every one of these has already been solved here:

- **Ids.** Integers from a counter collide the first time a track is copied between projects,
  and conflict on every merge. tutti's ids are opaque strings that are never remade, with
  time-sortable ULIDs recommended, so copies, branches and merges never collide.
- **Time.** Floating-point beats break equality; seconds stored in the file go wrong at the
  first tempo change; two engines integrating a ramp differently drift by milliseconds. tutti
  stores integer ticks at a declared resolution and writes the integration rule out, with test
  vectors.
- **Audio under tempo.** Anchors, warp markers and stretch ratios are three views of one thing.
  tutti has one rule, `(frame, tick)` points plus the tempo the material was recorded at, that
  covers all three with pitch kept.
- **Plugin state.** A state blob is bytes with no promise; fed to a plugin of another format it
  is garbage, usually silently. tutti keeps states as sidecar files named by content with their
  kind declared, a readable parameter snapshot beside them, and an order of authority.
- **The unknown.** The first version never has everything. tutti has namespaced extensions on
  every object, `extensions_used` and `extensions_required` with a core fallback for anything
  required, structural preservation of what a reader doesn't know, and a versioning rule that
  says exactly what a minor and a major may change.
- **Which plugins.** A project that names a plugin nobody has is silent. tutti's document lists
  its dependencies: each plugin's id, version, vendor, licence and where to download or build it
  from. The format declares; a program resolves.

Those decisions were made once, against the formats that exist (DAWproject, OpenTimelineIO,
glTF, Ableton, Bitwig, REAPER, Ardour, LMMS, Zrythm, AAF, the MIDI 2.0 clip file:
[`docs/research.md`](docs/research.md)), and written into a spec, a schema, tests and fixtures.
Using them costs you one MIT dependency, a crate or an npm package, or just a JSON library and
`tutti check`. Making your own costs you the same decisions, made alone, plus a format no other
program opens.

And what you give up is nothing. Anything your program models that tutti doesn't goes under
your namespace, kept by every other program; a thing many programs need is adopted into the core
at the next version, with your form still read.

## And DAWproject?

DAWproject (Bitwig, PreSonus) is the one cross-DAW format that ships, in Bitwig, Studio One,
Cubase and Cubasis, and tutti keeps its model's names where they fit. But it has no extension
mechanism and no versioning policy, its ids are remade on every save, its reader drops what it
doesn't know, and its container is a zip. tutti was designed to answer each of those. A bridge
to and from DAWproject is planned, so a tutti project reaches those programs too.

## Features

**The folder**

- A plain folder, never a package, a zip or a database: every file visible to search, sync and
  git. To send one, a defined zip form, `<Name>.tutti.zip`: the document first and stored,
  every entry's name and length checked, nothing written outside the folder being made.
- Audio in `audio/`, never renamed and never overwritten; plugin states in `plugins/states/`,
  each named by its content's SHA-256; optional renders so a project plays where its plugins are
  missing.
- A `README.md` in every project, written by every writer from one template: what the folder
  is, the five rules, the time model with two values to check a reader by, and the URLs of the
  rules.
- Private paths for a program's own files (versions, exports, trash, log, lock) that other
  programs ignore and never delete.
- Nothing in a project refers outside its folder.

**The document**

- One JSON document, I-JSON strict: no duplicate keys, every number finite, every integer exact.
- The manifest first: `format`, `version`, an `about` sentence, the `spec` and `schema` URLs,
  the `generator`, the extensions used and required, the `files` index, the `dependencies`.
- A pretty form on disk made for diffs: keys in a fixed order, an event, a point and a file
  entry each on its own line, defaults left out, so two people's tracks merge cleanly in git.
- Equality and fingerprints over RFC 8785 (JCS), so two implementations hash the same bytes.

**Identity and time**

- Ids are opaque strings, unique across the project, never changed on copy, move or save;
  writers use `<kind>_<ULID>`.
- Musical time is integer ticks at a declared `ppq`; seconds are never stored.
- A tempo map with `hold` and `linear` segments and the closed-form integration in the spec; a
  meter map; a key map.
- Audio placed by `(frame, tick)` points and the tempo of its material: one rule for anchors,
  warp markers and stretch, pitch kept; unwarped audio is the one-point case.

**The model**

- Tracks as a tree: a track with children is a folder, and its channel the group bus.
- A channel per track: linear gain, pan with the pan law stated, mute, solo, destination,
  sends; return tracks; the master.
- Patterns and parts: a pattern holds events, a part is a window on it with an offset and a
  loop. A clip is a part on a pattern of its own, so a clip DAW loses nothing and a pattern DAW
  keeps its sharing.
- Events: notes with velocity and release 0..1 and per-note expressions; chords that carry
  their voicing, so a reader without a chord engine sounds the right thing; parameter events;
  unknown types kept whole.
- Audio pieces on notes: the file by hash, `from` and `to`, gain, polarity, channel mode, fades,
  reverse.
- Automation lanes on device parameters, channel gain, pan, mute, sends and promoted
  parameters, with hold and linear points.
- Markers and sections, a loop, scenes and slots for a clip launcher, drum maps, metadata.

**Plugins**

- Devices in signal order, each with a role: instrument, note effect, audio effect, analyzer.
- Plugin ids with a scheme, `clap:`, `vst3:`, `au:`, `lv2:`, `vst2:`, `builtin:`, and aliases
  that name the same plugin in other formats, saying whether the state carries over.
- The state as a sidecar file by hash with its kind declared; a readable parameter snapshot
  (value, unit, range, text) a stranger or another format's build can use; overrides; the order
  of authority written down.
- Missing-plugin behaviour defined: keep everything, bypass the effect or play the render, say
  so by name, and write edits only into overrides.
- `dependencies`: every plugin the project uses, with its version, vendor, licence, formats and
  where to download or build it from.

**Extensions and versions**

- `extensions` on every object under a reverse-DNS namespace; `extensions_used` and
  `extensions_required`; a required extension leaves a core fallback.
- Structural preservation: unknown keys, unknown types and every extension value go back out on
  the same object, found by id.
- `major.minor` with `min_version`: a minor only adds, with a default and a fallback for every
  addition; a higher major is refused by name; a higher `min_version` opens read-only. A spec
  version is a frozen folder at a fixed URL.

**Robustness and conformance**

- Bare file names in fixed folders, no paths, no links followed, sizes verified, hashes checked;
  a mismatch warns and plays silence, never deletes. Hostile fixtures prove it.
- Three conformance levels, plays, hosts and round-trips, claimed by passing fixtures;
  `tutti check` says which level a project needs.
- A reader never refuses a project for what it doesn't understand.

**Tooling**

- `tutti`, the Rust crate: reads any form, writes the pretty form, validates, does the time
  arithmetic, and holds `Project`, the one save procedure every writer follows, over a store
  trait. It never touches files itself and depends only on serde, serde_json and sha2, so it
  compiles to WebAssembly unchanged and adds almost nothing to a program that uses it.
- `tutti-fs`, the disk: `open` and `create` give a `Project` for a folder; put a recording
  in through a writer, a state by content, save. An implementer never writes the
  procedure.
- The `tutti` command: `check`, `inspect`, `deps`, `archive`, `fmt`, `zip` and `unzip`.
- `@mxm-audio/tutti` for JavaScript (Node, browsers, Bun, Deno), with TypeScript types
  generated from the schema and the same `Project` over a store the host supplies.
- The JSON Schema (2020-12), a generated field reference, three example projects, and the
  conformance fixtures with their expected tables.
- `docs/agent-guide.md` and `llms.txt`: what an agent reads first.

## A project, in short

```
My-Project/
├─ My-Project.tutti           the document: the whole project, one JSON file
├─ README.md                  what this folder is, from the format's template
├─ audio/                     every sound it plays, known by SHA-256
├─ plugins/states/            plugin states, named by their content's SHA-256
├─ renders/                   bounces it may carry, so it plays without its plugins
└─ versions/ exports/ ...     the writing program's own: ignored, never deleted
```

```json
{
  "format": "tutti",
  "version": "1.0",
  "about": "An open audio project (tutti 1.0): this folder holds every file it uses. Time is integer ticks, 'ppq' per quarter note, placed in seconds by 'tempo'. Audio is placed by (frame, tick) points. Keep what you don't understand.",
  "spec": "https://github.com/mxm-audio/tutti/blob/main/spec/1.0/tutti.md",
  "schema": "https://github.com/mxm-audio/tutti/blob/main/schema/1.0/tutti.schema.json",
  "generator": { "name": "my-daw", "version": "0.1" },
  "ppq": 960,
  "tempo": [ { "at": 0, "bpm": 120 } ],
  "meter": [ { "at": 0, "numerator": 4, "denominator": 4 } ],
  "tracks": [ ]
}
```

## Using it

- **From your program**: read [`docs/agent-guide.md`](docs/agent-guide.md), then the spec,
  [`spec/1.0/tutti.md`](spec/1.0/tutti.md). From Rust, `tutti-fs` opens, creates and saves a
  project folder for you; from JavaScript, `@mxm-audio/tutti`; or use any JSON library with
  the schema. Run `tutti check` over what you write.
- **On a project someone sent you**: read the `README.md` in its folder.
- **To say your program reads tutti**: pass the fixtures of a level
  ([`spec/1.0/conformance.md`](spec/1.0/conformance.md)). No permission is needed and no fee.

## In this repository

| Path | What |
|---|---|
| `spec/<version>/` | The normative text, frozen once a version is declared; `spec/CHANGELOG.md` |
| `schema/<version>/` | The JSON Schema (2020-12), hand-written, frozen with its version |
| `examples/` | Example projects and the conformance fixtures |
| `docs/` | The research behind the format, the agent guide, the generated field reference |
| `crates/tutti` | The model, canonical reading and writing, validation, the time arithmetic |
| `crates/tutti-schema` | The tests that hold the schema, the examples and the types together |
| `crates/tutti-cli` | `tutti check`, `tutti inspect`, `tutti deps`, `tutti archive`, `tutti fmt` |
| `crates/tutti-wasm`, `js/` | The core for JavaScript, and `@mxm-audio/tutti`, the npm package around it |

## Status

Pre-alpha. The spec's first version is a draft: nothing is released, and the format may still
change in every detail. The first program to read and write it is
[newDAWn](https://github.com/maxmcorp/newDAWn). The DAWproject bridge, a Standard MIDI File
converter and bindings for more languages are planned.

## Licence

MIT (`LICENSE`): the code, the schema, the spec text and the examples alike, so any program,
open or closed, can read and write the format, and anyone may say their program reads tutti.
Contributions are welcome: see `CONTRIBUTING.md`. How the repository is organised, and the rules
each part keeps, are in `AGENTS.md`.
