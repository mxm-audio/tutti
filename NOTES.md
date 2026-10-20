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
- **2026-10-20:** **The format resolves dependencies.** Reason: the owner, "The format will also
  need a way to resolve dependcies. Eg. what plugins are used, and from where can they be
  donwloaded? (I imagine most of them are open source in the future.)" The document carries a
  `dependencies` list (id, name, vendor, version, url, repository, licence, formats), filled from
  what CLAP descriptors and VST3 factories declare; the format declares, a program resolves.

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
  together, and the generated field reference (stage 4).
