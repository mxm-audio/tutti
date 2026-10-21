# NOTES.md — tutti

The reasoning behind what `AGENTS.md` and the spec hold: the owner's rulings, dated, with their
reasons; the design choices and what they were weighed against. The research the format was
designed from is `docs/research.md`.

## The owner's rulings

- **2026-10-20:** **The project format is its own project**, not a part of newDAWn, which becomes
  its first consumer. Reason: the owner, "In a future where everybody will be vibe coding their own
  apps, yet still want to work together on projects. Or where humans and agents work together on
  projects, I believe that the file/project format will be the common denominator"; "I want this
  project file format to be its own project".
- **2026-10-20:** **The name is tutti**, "an open audio project format". Reason: a short brainstorm;
  "people dont necessarily use it to make songs", so the name must not say song; tutti is the
  musical direction for everyone together: humans, agents and every program on one page; "Audio
  Project" was the owner's phrase and is the tagline, not the name; nothing in audio software used
  the word. "tutti is fine".
- **2026-10-20:** **MIT for everything**: code, schema, spec text, examples; the CLA as in the kit;
  no trademark on the format's name, so anyone may say their program reads tutti.
- **2026-10-20:** **Public from the start, under `mxm-audio`**, created when the owner approves the
  first push; nothing is released (pre-alpha).
- **2026-10-20:** **The DAWproject bridge is planned but comes later**, after the first stages, on
  the owner's word.
- **2026-10-20:** **Usable from other languages, JavaScript at least.** Reason: the owner, "it
  should be a library that is not only usable from rust. Perhaps with wrappers for other
  languages, or at least javascript?" The core has no file IO and compiles to WebAssembly; an npm
  package wraps it; a C ABI or UniFFI later.
- **2026-10-21:** **Automation points and parameter events carry the plain value**, as `value` in
  the parameter's `params` entry, never a normalized one (spec 6.7, 6.8). Reason: the snapshot
  already says the plain value is authoritative and that neither form is derived from the
  other; CLAP parameters have no normalized form, so a CLAP host (newDAWn among them) could not
  write what a normalized lane asked for, while a VST3 host converts normalized to plain through
  the plugin at save time. Found in the brainstorm on where automation lives (on the track, in
  ticks, because a lane may target the channel or a send and state is timeless); the owner:
  "yes, change that".
- **2026-10-21:** **A sampler is a plugin, not a feature of the format.** An audio piece is a
  recording placed in time and always sounds as recorded; a note's pitch on it is informational.
  Reason: the owner, "I don't think we should implement a sampler in the file format. Then we
  would have to add envelopes, filters etc. So a sampler here is the sound.wav and the notes and
  a sampler plugin." What the format carries for it: the device's identity, its state, its
  parameter snapshot, its dependency entry, renders for a reader without the plugin, and (to be
  defined) the per-device files folder.
- **2026-10-20:** **The format resolves dependencies.** Reason: the owner, "The format will also
  need a way to resolve dependcies. Eg. what plugins are used, and from where can they be
  donwloaded? (I imagine most of them are open source in the future.)" The document carries a
  `dependencies` list (id, name, vendor, version, url, repository, licence, formats), filled from
  what CLAP descriptors and VST3 factories declare; the format declares, a program resolves.
- **2026-10-20:** **The repository's front page leads with the why and lists the features.**
  Reason: the owner, on the first README, "The github page needs more info. I told you the
  motivation for the making it a universal tool. That need to be obvious from the start of the
  page. And the It needs a list of features too. It is imperative that people should know why
  they must chose this format instead of making their own." So `README.md` opens with the
  premise (everybody writing their own programs, humans and agents on one project, the format
  as the common denominator), then *Why not make your own* (the problems a format hides: ids,
  time, audio under tempo, plugin state, the unknown, dependencies, each answered), a short
  word on DAWproject, the feature list by area, the folder and manifest in short, how to use
  it, and only then the repository table, status and licence.

## Design choices and what they were weighed against

- **One JSON document per project**, with events, points and file entries one per line, rather
  than one file per track: a set of files cannot be committed atomically without a journal every
  implementer would have to get right, and the index every layout shares conflicts on merge
  anyway; one document keeps a temp-file-and-rename commit point (Pi's review, round 2). A
  per-track layout can come as a minor version if merge pain is real.
- **JSON with a JSON Schema**, not XML with an XSD: DAWproject's XML has no extension story and
  its JAXB reader drops the unknown; a JSON Schema is what an agent's structured output generates
  against. Not YAML or TOML: the research found the serialisation doesn't change an agent's
  accuracy, so the one with a schema language wins.
- **A plain folder**, never a zip, a package or a database: Logic's package hides media from
  search and has arrived corrupted through Dropbox; Audacity's database brought slow closes and
  refusal on FAT drives (`docs/research.md`).
- **Ids are opaque strings**, writers using a kind prefix and a ULID: integers from one counter
  collide when copying between projects and conflict on every merge; DAWproject's `id0…` remade
  on save survives no round trip; UUIDv4 is 36 characters and unsortable.
- **Integer ticks at a declared `ppq`**, a tempo map with `hold` and `linear` and the closed-form
  integration written out: rationals have no JSON form, floating beats break equality, and two
  engines integrating a ramp differently drift by milliseconds (DAWproject states no rule).
- **Audio placed by `(frame, tick)` points plus a `bpm`**: one rule covers newDAWn's anchor and
  tempo, DAWproject's warps and Live's warp markers.
- **glTF's extension mechanism, not OTIO's per-object schema versions**: `extensions` on every
  object with reverse-DNS namespaces (CLAP's naming), `extensions_used` and `extensions_required`
  with a core fallback for anything required; per-object versions with up and down converters are
  right for a library of thirty types and heavy for a weekend DAW. Preservation is structural:
  the same JSON value on the same object, found by id.
- **Equality and fingerprints over RFC 8785 (JCS)**, the on-disk pretty form sharing its number
  and string rendering: a hash two implementations compute must be over bytes a standard defines
  (Pi's review, round 3).
- **Patterns reused by parts as the primitive**, a clip being a one-use pattern, so a clip DAW
  loses nothing; chords carry their voicing, so a reader without a chord engine sounds the right
  thing: every musical meaning carries its rendered fallback.
- **Linear gain, velocity 0..1**: no minus infinity in JSON; lossless for MIDI 1, MIDI 2 and CLAP.
- **Sidecar plugin states by content hash with a `kind`**, a readable parameter snapshot beside
  them, and renders a project may carry: Ardour's and REAPER's inline base64 makes every diff
  opaque; a state of one ABI fed to a plugin of another is garbage, usually silently.
- **The save procedure lives in the core, as `Project` over a `Store` of primitive operations;
  the disk is a crate of its own, `tutti-fs`** (2026-10-21). The owner's rule: an implementer
  never writes file-system code against the spec ("It can easily get out of sync, and be
  misunderstood. It is a huge risk to break the standard"). Not a feature flag in the core: its
  no-IO contract is what lets it build for the web, and Pi's review held the line (round 1).
  Not "the only place": the spec stays the authority and `tutti check` the gate, since a
  format safe only through one library is an SDK, and C++ DAWs, Python tools and agents
  writing JSON will never call it. The store is primitives (list, exists, make a folder, write
  a new file, stream one, rename over, remove), each a crash point, so newDAWn's
  failure-injecting file trait can implement it and keep its crash tests. A recording streams
  through a handle that hashes as it goes and owns a clone of the store, so a recorder runs on
  its own thread; the handle's guard keeps the file reserved until the command that names it
  has run. What a save verifies: lengths at every save, hashes once per session, a mismatch
  refusing; `save_verified` rehashes all. Pi asked for a rehash at every save (rounds 2 to 4):
  declined for the cost (a one-hour session of 48 kHz stereo is about 700 MB per track to
  read at every save, and a DAW saves after every change), stated as the owner's decision in
  the plan. Index entries no known field claims are kept and verified, and refuse the save
  when their file is gone until the program forgets them on purpose, since an unknown
  extension may refer to them. The name cleaning came from newDAWn's store, relicensed MIT.
- **Frozen spec folders instead of tags**: the workspace refuses tags, and a document's `version`
  must name an immutable text.
- **Three conformance levels** (plays, hosts, round-trips), claimed by passing fixtures: DAWproject's
  implementers "ignore parts of each other's data" with no way to say what they support.
- Decided against, one line each: per-object schema tags (bloat; the tree says what an object
  is); a JSON Schema in every project (the README and URLs do it; `archive` snapshots); `extras`
  beside `extensions` (one namespaced container); second-anchored positions in 1.0 (`bpm: null`
  covers unwarped audio); curves other than linear in 1.0; a zip profile in 1.0 (no safe reader
  planned yet); building the DAWproject bridge first (the owner: later).

## History

- 2026-10-20: the repository created in the workspace (stage 1 of the plan); the spec's draft, its
  conformance and extension texts, the folder README template, the research write-up, the agent
  guide and `llms.txt` written (stage 2); the core crate (the model, the two canonical forms,
  reading, validation, the time arithmetic, the render fingerprint, the folder trait and the
  feature list), the generated example projects and the Level 0 fixtures (stage 3); the JSON
  Schema, the hostile fixtures, the tests that hold spec, schema, examples, docs and code
  together, and the generated field reference (stage 4); the `tutti` command (stage 5); the
  wasm bridge and the npm package `@mxm-audio/tutti` with its Node tests (stage 7; `tutti` on npm
  is someone else's, hence the scope).
- 2026-10-20: the README rewritten to lead with the motivation and list the features, on the
  owner's word (the ruling above).
- 2026-10-21: automation points and parameter events carry the plain value (the ruling above);
  the agent guide renamed `docs/agent-guide.md`, since on a case-insensitive file system it and
  the folder's DOX file `docs/AGENTS.md` were one file and the guide had overwritten the DOX in
  the first commits; `docs/AGENTS.md` written anew.
- 2026-10-21: stage 12 of the plan's second round: `Project` in the core (`project.rs`, `names.rs`,
  `Store` and `MemoryStore` in `folder.rs`), `crates/tutti-fs` (the disk store, `open`,
  `create`), the command and the example generator onto them, the examples regenerated
  through `Project::save` (one list sorted, nothing else changed).
