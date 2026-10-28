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
- **2026-10-21:** **A file is verified by its length and modification time, hashed again only
  when either differs; a file changed behind the project's back is a breach, not a case.**
  Reason: the owner, "Audio should not silently change after it is saved. Should it? If so
  that should be a break of the contract. And why the whole audio files... look at the size
  and the timestamp. If they have not changed it should be pretty safe too": rule two of the
  folder forbids the change, the hash rule catches it (`check`, silence and a warning), and
  size and time are what git, rsync and every build tool trust. The one way past them is a
  same-length change with the time put back: forgery, and `check` still catches it. The
  records outlive a session through the program (`Project::verified`, `trust`), never the
  document, which must not carry a file system's times. This closed the question Pi had
  pressed (rehash at every save).
- **2026-10-21:** **The player, the second consumer, showed what every writer repeated, and
  the crate now does it** (the owner: "If something in tutti makes adding the tutti to the
  player [hard], then we should think about changing tutti too. Without changing what tutti
  is for."). Four things, none of them a change to the format: `Project::save` keeps
  `dependencies` in step with the devices, as spec 3.5 already asks (newDAWn never dropped the
  entry of a plugin it removed, and the player had to write the same upkeep); the model's
  constructors carry the spec's defaults (a writer had to know that a note releases at 0.5
  and a device is on); `Document::track` and `track_mut` find a track through folders; and
  `tutti_fs::ulid` gives native programs new ids, as the JavaScript package already did (both
  consumers had written their own). An entry another program's extension names is kept, by
  the same rule as an index entry nothing claims.
- **2026-10-21:** **A save never leaves a project less complete than it found it, and never
  blesses a change.** A file the index already lists that has gone missing or changed behind
  the writer's back keeps its entry as it was, and the save goes on and says so; only a new
  reference to a file that isn't there refuses the save, and an export (`save_verified`, a copy,
  a zip) refuses either. Reason: newDAWn's review found that refusing made a song unsaveable
  the moment a recording went missing (moved by a sync, deleted by hand), every edit piling
  up in its log; refusing helps nobody, while keeping the entry leaves the project exactly as
  incomplete as it was, which `check` reports. This replaces Pi's round-2 rule (refuse until
  forgotten) for entries no known field claims as well: they too are kept and reported, and
  `forget_file` drops one on purpose.
- **2026-10-21:** **A plugin's own files have a folder of their own in 1.0** (spec 7.8,
  `plugins/files/<device id>/`, nested paths allowed, every file in the index under its device's
  id). Reason: the owner, "To transfer a project from one machine to another we need to store
  the plugin's identity, the plugin's parameter values in the instance, and any resources the
  plugin needs for the settings. And we cannot assume the file types of the resources. Ie
  Harmor can use an image for synthesis. Basically anything that is needed to make two systems
  with the same plugins create the same result." Safety is required (an unsafe path is
  invalid) and portability advised (a warning), apart, as Pi's review asked. No `files` key on
  the device: the folder named by its id is the list, the index proves it present, a copy goes
  by the id. How the files get there is the host's and the plugin's (CLAP's resource-directory
  extension; a plugin without one keeps absolute paths, and the format can only say so).
- **2026-10-21:** **A project as one file, `<Name>.tutti.zip`, is in 1.0** (spec 2.5): the
  owner, "the zip thing is ready to go", after the brainstorm found that a browser writer can
  only hand over one file and that a zip form left undefined would be invented differently by
  every program. The document first and stored; entry names a relative path of plain parts,
  the whole file refused on one that is not; no declared length trusted; nothing written
  outside the folder being made. `tutti zip` and `unzip`, in `tutti-fs` over the `zip` crate
  (deflate only, pinned), the fixtures `zip-slip` and `zip-bomb` written by the generator's
  own stored-zip writer.
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
- **2026-10-21:** **Equality leaves `generator` out, and `tutti check` notes a key it doesn't
  know.** Reason: the web tracker's trial (below), the owner: "agree with the two decisions".
  A writer names itself in `generator`, so Level 2's "written again is equal" could never hold
  for another program's write-back; and a misspelt key is kept like any unknown one, so only a
  note shows it. A note, never an error: a later version's key is unknown here too.
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
- **2026-10-25:** **A render says which devices are in it** (spec 7.7, `devices`, required).
  Reason: the owner, "I think tutti need support for rendered stems too. and it must say what
  effects are rendered into the stems. What effect should be the users choice, but the next one
  working on the project should know it. Most typical use cases would be no effects and all
  effects"; and of what it is for, "a kind of handover file, with rendered stems as backup for
  safety". The list is the truth, a label (`name`) is for people, and "with/without effects"
  are the crate's helpers, never keywords: nothing derivable is stored.
- **2026-10-25:** **Renders stay pre-fader**, the chain's output before the fader and the
  sends: the owner chose it over a `fader` choice, which can come later as an optional key in a
  minor version.
- **2026-10-25:** **Pattern renders are in 1.0** (spec 6.5, 7.7): the owner, "Perhaps this
  should work at a pattern level too? So patterns can be exported too instead of only whole
  stems?", then yes to a pattern render at one stated tempo, which travels as a loop file and
  lets a reader without the plugin play a part from it, stretched.
- **2026-10-25:** **A render is exactly its range, with no tail** (spec 7.7): the owner, "it
  should always render the loop length", "The user decides how much empty space is around the
  song"; for a pattern, what rings past its end is cut ("Cut it", over wrapping the tail into
  the start). A tail is a longer range, as newDAWn's exports already had it.
- **2026-10-25:** **The document's `loop` is `range`** (spec 3.6, `range: {start, end, loop}`):
  the owner, "Perhaps we should change what is now called loop to call it the "range". Since it
  has multiple functions besides setting the loop". It is the span the user marks: playback
  loops over it when `loop` is on, and exports cover it (its renders no longer: 2026-10-26). A
  part's and a slot's `loop` are a pattern's own repeat and keep the word. No alias for the old
  key: nothing is released, and a stale writer shows in `tutti check` as an unknown key.
- **2026-10-26:** **A project's renders cover the whole song, not the range** (spec 3.6, 7.7):
  the owner, "it makes nowe sense to chose a range and then export that in a full project. We
  should probably have en "export range" function that does not save to the tutti project", and
  "renders in full + 4 bars ekstra". A handover's track renders run from tick 0 to past the last
  part, with room for what rings on, every track the same span; the range is for exports, which
  never go into the document. Still exactly the span, as a render always is.
- **2026-10-28:** **A program's own files live in `private/<namespace>/`**, named by the
  namespace its `extensions` use (spec 2.3), and the document's `private` list is gone. Reason:
  the owner asked whether a program other than newDAWn could keep data of its own in a project;
  in the document it could (`extensions`), but the folder's `versions/`, `exports/`, `trash/`,
  `log/` and `lock` and the `private` list were "the writing program's", and nothing said which
  program that was. A second writer could not tell whose `log/` or `lock` it found, nor whether
  to keep newDAWn's `private` entries or write its own. The owner chose namespaced folders over
  a `private` list split by namespace ("go with namespaced folders"): the folder's name says
  whose it is, so nothing in the document has to, which is one way to do it. No alias for the
  old paths or the key: nothing is released, and a stale `private` key shows in `tutti check`
  as an unknown one.
- **2026-10-28:** **Five meanings for track groups, settled before 1.0 is declared** (newDAWn's
  `plans/plan-track-groups.md`, its G0). Reason: newDAWn makes groups tutti's folders, central
  to the DAW (the owner, the same day), and a meaning left open would freeze with 1.0. The
  owner approved the wording as the plan put it ("Approve as worded"):
  - **Solo** (6.2) is a rule over connections. A track inside a soloed folder counts as soloed,
    and a connection carries sound only on a path from a soloed track to the master or into a
    soloed track. The old sentence, read literally, silenced a folder's children when the folder
    alone was soloed, since they're upstream of it, not on its path.
  - **A send taps** the chain output: pre-fader before gain, pan and mute, post-fader after them,
    as in Live ("before or after the mixer stage (pan, volume, and track-active controls)").
  - **Arriving audio** enters a track's chain after its instruments, before its first audio
    effect, where newDAWn already sums a track's recordings.
  - **A folder's notes** pass through its note effects to its members, nested folders passing
    them on (the owner chose any depth over direct members only). A track render holds the
    notes its track receives, and its fingerprint gains `folders`: the folders above with
    patterns or enabled note effects, each reduced as a track is. With none, the key is left
    out, so regrouping never stales a render. Without this, a 1.0 reader would play a layering
    folder's notes as silence: "a track without an instrument sounds its notes nowhere".
  - **Overlapping parts** on a track all play and sum, which comps need.

  Nobody uses tutti yet (the owner, the same day), so no reader or fingerprint had to be kept.

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
  refusing; `save_verified` rehashes all; a failure after the rename (the folder's flush) is
  `Unconfirmed`, the save being on disk, which a program with its own generation count needs
  to know (newDAWn's did, 2026-10-21). Pi asked for a rehash at every save (rounds 2 to 4):
  declined for the cost (a one-hour session of 48 kHz stereo is about 700 MB per track to
  read at every save, and a DAW saves after every change), stated as the owner's decision in
  the plan. An index entry no known field claims is kept and verified while the document
  mentions its file anywhere else (its path, its name or its hash: how an unknown extension
  would refer to it), refusing the save when the file is gone until the program forgets it on
  purpose; one nothing mentions goes, since keeping every present file for ever kept a state
  newDAWn's model had dropped indexed, and so kept, for ever (2026-10-21; Pi's blocker 2 of
  round 2 asked for all kept, which that showed wrong). The name cleaning came from newDAWn's
  store, relicensed MIT.
- **Frozen spec folders instead of tags**: the workspace refuses tags, and a document's `version`
  must name an immutable text.
- **Three conformance levels** (plays, hosts, round-trips), claimed by passing fixtures: DAWproject's
  implementers "ignore parts of each other's data" with no way to say what they support.
- **A fixture's table asks only what the spec pins** (2026-10-25, writing Levels 1 and 2): where
  the text is silent (how a stereo source's balance runs between the centre and the side, a lane
  before its first point), the case is built so every reading gives the same numbers, and the
  gap goes to the owner as a question, never into a table as a guess. A table that guessed would
  fail a conforming program. Two such gaps were closed on 2026-10-28 (below): what solo does to
  a return's inputs, and where a send taps the pan.
- **A render stands in only for a prefix of the chain in signal order, every instrument
  included** (2026-10-25, the plan's review with Pi): a reader runs the devices after a render
  itself, which it can only do when the render holds all that comes before them; and what comes
  out before the instruments is notes, which no audio device takes. A render of any other
  choice is valid and for listening and export. Prefer the render standing in for the fewest
  devices, since the more of the chain a reader runs, the more of the user's edits it plays.
- **A render's fingerprint covers what it holds and nothing else** (2026-10-25): the track
  reduced to the listed devices, the parameters and lanes and parameter events that reach them,
  the channel without what acts after the tap, and no names, colours, drum map, slots or other
  renders, so tweaking an effect a "without effects" render leaves out, the fader or the track's
  name keeps it current; unknown keys stay in, so a later key that changes the sound still
  stales it. A pattern render's descriptor keeps the reduced track (its extensions, its unknown
  keys) beside the pattern, without the patterns, parts and automation that don't play in it.
  The meter and key maps stay in both: a plugin reads them through its host's transport. The
  track render's descriptor still holds every pattern of the track, not cut to its range:
  conservative, and simple to state.
- **A render's file is one render** (no two name the same file): a host matching renders by
  file, as newDAWn's store does, then always finds the one meant (Pi's review, round 4).
- **`symlink-audio` links the `audio` folder, not a file, and the runner makes the link**: git
  on Windows checks a link out as a file and making a file link there needs a privilege, while
  a junction needs none and is a link to every reader; and a reader that checks only a path's
  last part misses a folder link, which is the harder test. The target lies inside the case's
  copy, so a reader that follows links "only inside the folder" fails it too: the spec refuses
  every link.
- Decided against, one line each: per-object schema tags (bloat; the tree says what an object
  is); a JSON Schema in every project (the README and URLs do it; `archive` snapshots); `extras`
  beside `extensions` (one namespaced container); second-anchored positions in 1.0 (`bpm: null`
  covers unwarped audio); curves other than linear in 1.0; a zip profile left for later (no safe reader
  planned yet); building the DAWproject bridge first (the owner: later).

## What the web tracker's trial taught the format (2026-10-21)

newDAWn's web tracker (`mxm-audio/mxm-tracker`, newDAWn's `plans/plan-web-tracker.md`) is the
format's second program and its trial run: an app of its own in TypeScript that reads, edits and
saves songs through this package alone, with nothing of newDAWn's code. Its round trip (W7) is
tests in both programs: ten songs from the web tracker open in newDAWn, and the same ten edited
in newDAWn come back.

- **The package was enough.** Opening, validating, editing in place, saving to the browser's
  storage, sharing as one file, the tempo map's arithmetic (`samplesAt`), deterministic ids for
  a generator (`ulid` with a clock and random bits given) and `equal` all came from it; the
  trial needed nothing added after W1's browser build.
- **Two writers, one document.** Every song from either program opens in the other editable,
  and the web tracker writes newDAWn's songs back equal, by `equal`, to what it read.
- **`generator` and Level 2 disagree.** A writer fills `generator` with itself (spec 3.2,
  conformance 2), and Level 2 asks that a document read and written again be equal to what was
  read. When another program writes it back, the two can't both hold: the round trip compares
  with the writer's name put back. **Done (the owner's ruling above)**: equality leaves
  `generator` out (3.1), and the round trip compares the documents as they are.
- **A misnamed field would be valid.** The tracker's port was first written with `voice`,
  newDAWn's own name, for a recording note's audio, where the format says `audio`. Reading the
  format's model caught it, not a check: a reader keeps unknown keys (8.2), so a document with
  the wrong name would have validated, its meaning lost without a word. **Done (the owner's
  ruling above)**: `tutti check` notes each key this version doesn't know, once, with the first
  place it is and how many objects carry it (`unknown_keys`, `unknownKeys` in the package), and
  the web tracker's round trip asserts its songs carry none.
- **Two time implementations agree**: the web tracker's engine schedules through the core's
  `TempoMap`, newDAWn's playback through its own arithmetic, and their note events are the same
  frame for frame at 48 kHz on all ten songs (125 notes). Every song holds one tempo, 120 BPM
  at `ppq` 960, so ramps and other `ppq`s were not tried by the trial; Level 0's `scale` and
  `ramp` cover them.
- **The schema stands alone**: every song both ways is valid by the hand-written schema in Ajv
  8.17.1, strict, which shares no code with either program (its `uri` formats need Ajv's formats
  package, and were left to `check`); and each, packed and unpacked, passes `check`.
- **Fixtures for tutti: none admitted as they are.** The songs carry the MXM sampler's own state,
  where Level 1's `plugin` is a tiny reference CLAP and `missing-plugin` a fictitious one, and a
  fixture here is made by `make-examples`, never by another program. What the trial suggests,
  now that equality leaves `generator` out: a Level 2 case `another-writer`, a project written by one
  generator, read and written by another, and expected equal but for `generator`.

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
  through `Project::save` (one list sorted, nothing else changed). Then, for newDAWn's move
  onto it (stage 15): `Project::find_audio`, `Store::sync_folder` as a step of its own after
  the rename with `ProjectError::Unconfirmed`, and the mention rule for unclaimed index entries.
- 2026-10-21: a file verified by length and modification time (`Folder::modified`, `Verified`,
  `Project::verified` and `trust`); the zip profile (spec 2.5, `tutti::zip`, `tutti_fs::zip`, `tutti
  zip` and `unzip`, the `zip-slip` and `zip-bomb` fixtures).
- 2026-10-21: stage 13: `Project` for JavaScript (`crates/tutti-wasm/src/project.rs`, over a
  store the host supplies), `nodeStore` and `memoryStore` in the package, `checkFolder` gone.
- 2026-10-21: stage 14: a plugin's own files (spec 7.8; `Project::device_files`,
  `copy_device_files`; the index path rule in `validate`; the `device-files-escape` fixture;
  every-feature carries one).
- 2026-10-21: stage 16, the closeout: `docs/implementers.md`; the README says the spec is the
  authority, `check` the gate and the crate the one implementation we ship; newDAWn states in
  its `AGENTS.md` that it claims no conformance level yet. CI by hand is the owner's.
- 2026-10-21: a code review of the day's commits (ten findings, nine taken): a part with `:`
  is drive-relative on Windows and is refused in every path (a zip-slip the fixtures missed);
  plain values must be finite; a torn state file is written again; every file in a live
  device's folder is indexed, however it got there; a folder with two documents refuses an
  unpack or a copy; an unpack leaves the reserved paths out; the JavaScript `readOnly` is
  computed for an adopted or copied project; `modified` crosses to JavaScript as a decimal
  string, a nanosecond count overflowing a number; the DOX says what the crate removes.
  Declined: streaming the copy's reads (the store trait reads whole today; the copy now
  remembers what it wrote, so the save hashes nothing again).
- 2026-10-21: the package in a browser, for a web app's trial of the format (newDAWn's web
  tracker plan, its W1). The one-file form moved from `tutti-fs` into `tutti-zip`, over any
  store, so the disk and bytes in memory share one implementation (the zip crate builds for
  wasm32); `Folder::copy_to` keeps the disk's pack streamed. `pack` and `unpack` reach
  JavaScript as `packZip` and `unpackZip`. The wrapper is `core.mjs`, made over either build:
  `index.js` loads it for Node (`require` of an ES module, so Node 20.19 is the floor) and
  `browser.mjs` for a browser, with `browserFolder`: a project read whole into a memory store,
  and a save written back other files first, the document last under a temporary name moved
  into place (`move(folder, name)`, the form WebKit takes). Measured the same day: on an iPhone
  (iOS 27), `createWritable` and that `move` work; in Playwright's WebKit for Windows,
  `createWritable` keeps nothing and a fresh context gives no storage at all, so the browser
  test runs in a profile of its own and its storage check skips there, saying so, and runs in
  Chromium.
- 2026-10-21: the trial's two rulings: `equal` leaves `generator` out (spec 3.1, conformance's
  Level 2); `unknown_keys` (`unknown.rs`), a walk of the document's own serialisation in which
  an unknown key is a `Key` newtype, noted by `tutti check` and reaching JavaScript as
  `unknownKeys`; spec 8.2 lets a checker note such a key, never as an error.
- 2026-10-25: the fixtures' gaps filled, on the owner's word ("start the work that needs no
  decision"), toward declaring 1.0: Level 1's `automation`, `sends`, `warps`, `expressions` and
  `missing-plugin`; Level 2's `everything` and `merge-different-tracks`; the hostile
  `symlink-audio`, and `duplicate-key` listed in `conformance.md`. Level 1's `plugin` waits for
  its reference CLAP. Written by the generator; their tests written, to be run in one batch
  later (the owner: "lets get everything as ready as possible. Just dont do the tests yet").
- 2026-10-25: renders that say what is in them (the rulings above): spec 3.6, 6.1, 6.5, 7.6,
  7.7, conformance's profile and `render-choice`, extensions 31; the schema's `render`,
  `pattern_render` and `range`; `Render`, `PatternRender`, `AudioFacts` and `Range` in the
  model; `render.rs` rewritten (`signal_order`, `stands_in`, `without_effects`, `with_effects`,
  the two descriptors, the frames a render holds, `renders`); `validate::check_render` and
  `check_pattern_render`, one file per render; the index and the copy carrying pattern renders,
  a render's hash checked against the index; `tutti check`'s render notes and `inspect`'s
  render listing; `renderFingerprint`, `patternRenderFingerprint`, `renderDevices` and
  `renders` in the package; every-feature's bass handed over as stems and a pattern render, the
  examples' `range`. Planned with newDAWn's half and reviewed with Pi over eight rounds. The
  batch of fixture tests written the same day ran with it, all passing.
- 2026-10-28: private folders (the ruling above): spec 2, 2.1, 2.3, 2.5, 3.2 and 9,
  conformance 1 and 2, extensions 1 and 3 and newDAWn's registry row, the README template; the
  schema's and the model's `private` gone with its check in `validate`; `tutti-zip` skips
  `private/` where it skipped the five reserved names, and a test holds it both ways; the
  examples, the field reference and `schema.d.ts` generated again.
- 2026-10-28: track groups' five meanings (the ruling above): spec 6.2, 6.6, 7.1 and 7.7,
  conformance 3 and 4; `render::folders_above` and the descriptor's `folders`, with tests that
  regrouping leaves a fingerprint and a folder's notes change it; `features` naming "folder
  notes" at Level 1; four fixtures generated after `render-choice` (`folder-solo` and
  `overlap`, Level 0; `return-solo` and `folder-notes`, Level 1), the last held to its table by
  `tutti-schema`'s tests.
