# The research behind tutti

**Status: the research of 2026-10-20, from which the format was designed; the design itself is
`spec/1.0/tutti.md`, and the choices with their reasons are in `NOTES.md`.** The brief: a project
format that a project can be started in by one DAW and opened in another, including DAWs that
don't exist yet, written by people or by agents; so it must be interoperable, expandable without
breaking other readers, and self-documenting. The owner's premise: "In a future where everybody
will be vibe coding their own apps, yet still want to work together on projects... the
file/project format will be the common denominator."

Everything below was read on 2026-10-20 from the pages in *Sources*. A judgement is marked as
one; what could not be verified says so.

## In short

- **DAWproject is the only cross-DAW format that ships**, in Bitwig, Studio One, Cubase and
  Cubasis. Its model is sound and tutti keeps its names where they fit. But it has no extension
  mechanism, no versioning policy (one flat `1.0`, a 2024 schema change without a bump), ids that
  are remade on every save, unknown elements dropped by its reader, and 36 open issues with a quiet
  maintainer. Users lose MIDI CC, tempo tracks, markers, automation and plugin state across DAWs.
- **OpenTimelineIO shows how to keep what you don't understand** (an unknown schema is kept and
  written back unchanged) and how to version a format (per-type upgrade and downgrade, "version
  families"). **glTF shows how to extend one** (`extensionsUsed`, `extensionsRequired`, vendor
  prefixes, a core fallback for anything required, minor versions forward-compatible by rule).
- **A plain folder beats a package or a database** (Logic's package, Audacity's database), and
  **what can be made again belongs in a cache, not in the project** (Pro Tools, Cubase, Logic).
- **Git fits DAW files badly** because they are not line-oriented and their ids churn; Live's
  XML with stable ids "diffs and merges cleanly". Every hosted collaboration service in this space
  died or pivoted, so a format that merges offline outlives any service.
- **Time must be exact and its rules written down**: integer ticks or rationals, a tempo map with
  the integration rule stated (DAWproject states none; a mean-tempo shortcut is 106 ms off over
  four beats), both domains stored for anything anchored in both (warp points).
- **Plugin state is bytes with no promise**: CLAP's `state` says nothing about compatibility,
  a VST3 preset carries two chunks, and a state of one ABI fed to a plugin of another is garbage,
  usually silently. Keep it in sidecar files named by content, with a readable parameter snapshot
  beside it.
- **Self-documentation works when the file names its own schema and the folder carries a short
  README**; a JSON Schema is what an agent's structured output generates against, and the
  serialisation (JSON, YAML, TOML) does not change an agent's accuracy.

## DAWproject

The open exchange format from Bitwig and PreSonus (MIT), a `.dawproject` ZIP holding
`project.xml` and `metadata.xml`, audio embedded or referenced, plugin states always embedded.

- **Model.** `Project{version, Application, Transport{Tempo, TimeSignature}, Structure{Track*},
  Arrangement{Lanes, Markers, TempoAutomation, TimeSignatureAutomation}, Scenes}`. A `Track` has
  a `contentType`, an optional `Channel` and child tracks (folders). A `Channel` has a `role`
  (`regular`, `master`, `effect`, `submix`, `vca`), `audioChannels`, a `destination`, `Devices`,
  `Mute`, `Pan`, `Volume` and `Sends` (`pre` or `post`). Timelines nest: `Lanes`, `Notes`/`Note`
  (`time`, `duration`, `key`, `vel`, `rel`, child timelines for per-note expression),
  `Clips`/`Clip` (`time`, `duration`, `playStart`, `playStop`, `loopStart`, `loopEnd`, fades,
  `contentTimeUnit`, a `reference` for alias clips), `Markers`, `Warps` (`Warp{time,
  contentTime}` pairs), `Audio` (`File`, `duration`, `algorithm`, `channels`, `sampleRate`),
  `Points` (`Target` by `parameter` or `expression`, then `RealPoint` with `hold` or `linear`).
  `timeUnit` is `beats` or `seconds`, inherited down nested timelines. Devices: `deviceID` (a UUID
  for VST3, an integer for VST2, the id string for CLAP, undefined for AU: issue #61),
  `deviceName`, `deviceRole`, `deviceVendor`, `loaded`, a `State` file reference, `Parameters`,
  `Enabled`. `RealParameter.unit` is `linear`, `normalized`, `percent`, `decibel`, `hertz`,
  `semitones`, `seconds`, `beats` or `bpm`.
- **Ids.** `Referenceable.id` is an XML `xs:ID`; the reference implementation mints `id0`,
  `id1`… and remakes them on every save, so nothing survives a round trip.
- **No extension mechanism, no versioning policy.** The XSD has no `xs:any`; the only version is
  a flat `version="1.0"` string; commit `23ba268` (2024-09-02) added attributes without a bump;
  unknown elements are dropped by the JAXB reader's default. Maintainers chose XML over JSON for
  polymorphism (#27) and have "no current plan" for a C/C++ library (#39). Last commit 2025-07;
  36 open issues, mostly unanswered, asking for a generic device element (#98), receivers instead
  of sends (#91), sidechains (#90), multi-out (#70), clip gain (#71), a sampler device (#67),
  string parameters (#64), LV2 (#58), version control (#40), notation (#38).
- **Adoption** (2026): Bitwig 5.0.9+, Studio One 6.5+, Cubase 14.0.20+ (Cubase 15: "improved
  DAWproject"), Cubasis 3.7.1+, VST Live, n-Track, Waveform 14, MAGDA; REAPER only through the
  Java ProjectConverter; none in Live, Logic, FL Studio, Pro Tools, Ardour or Zrythm. Steinberg is
  the one vendor advancing it; Ableton and Bitwig announced nothing on interchange in 2025 or
  2026, Bitwig 6 importing `.als` and `.flp` directly instead.
- **Losses users hit:** MIDI CC and pitch bend, the tempo track, markers, folder tracks,
  automation and crossfades (Cubase's own release note), plugin state across DAWs (VST3 presets
  failing in Cubase 14+, AU parameter ids misread), alias clips flattened, Studio One not reading
  Bitwig's clip-launcher or CLAP data.
- **Libraries:** Rust `dawproject` 0.11.0 (MIT/Apache-2.0, read and write, generated from the
  XSD, MSRV 1.88), Python DAWproject-Py, TypeScript dawproject-typescript, the DawVert converter.

*What tutti takes:* the track/channel vocabulary (`role`, `destination`, `sends`, `warps`,
`hold|linear`, the parameter units), the device fields, markers, scenes. *What it fixes:* ids,
versioning, extensions, preservation, the integration rule, the container.

## The MIDI 2.0 Clip File

M2-116-U (2023): `.midi2`, magic `SMF2CLIP`, UMP messages with delta clockstamps, DCTPQ ticks per
quarter, Flex Data in place of SMF meta events, one clip per file (SMF type 0's role). No DAW reads
or writes it as of 2026; the multi-track Container File (an XML manifest over clip files, audio,
MusicXML, video and lyrics, shown at NAMM 2026) is unpublished. Not a bridge today; a possible
export later.

## CLAP and VST3 state

- `clap.state` saves and loads a bare byte stream on the main thread; the header never uses the
  word opaque and makes no compatibility promise. `clap.state-context` (1.2.0) tells a plugin
  whether a state is for a preset, a duplicate or a project. `clap.preset-load` loads from a file
  path or a bundled location; `preset-discovery` declares file types. A draft
  `plugin-state-converter` factory converts state between plugin ids. There is **no
  `.clap-preset` file format in CLAP**: the name appears only in DAWproject's Javadoc.
- clap-wrapper writes the CLAP state bytes as the VST3 `Comp` chunk unchanged, so a wrapped
  CLAP's VST3 state equals its CLAP state; a native VST3 build's does not.
- `.vstpreset`: `'VST3'`, int32 version 1, a 32-character class id, an int64 offset to a chunk
  list; chunks `Comp` (component), `Cont` (controller), `Prog`, `Info` (XML meta, last).

*What tutti takes:* a `kind` on every state blob; the rule that a state is fed only through its
own ABI or a compatible alias; a readable parameter snapshot as the fallback.

## Other DAWs' own formats

- **Ableton Live** `.als`: gzipped XML, `<Ableton MajorVersion MinorVersion SchemaChangeCount
  Creator>`; warp markers are `(BeatTime, SecTime)` pairs; stable ids, so it "diffs and merges
  cleanly" (Wit's measurement). **Bitwig** `.bwproject`: binary, undocumented.
- **Ardour** `.ardour`: XML with no DTD, "not an interchange format"; a `version` on the root, a
  newer major refused, an older session backed up as `snapshot-<version>.ardour`; VST chunks as
  base64 in a `<chunk>`, else `param-N`; unsupported old data "skipped entirely".
- **REAPER** `.rpp`: plain text, line tokens and nested `<TAG>` chunks, plugin state as base64
  inside `<VST>`.
- **LMMS**: XML whose root `version` is the count of upgrade methods, run in order from the
  stored index; a newer file loads as-is. **Zrythm**: zstd-compressed JSON `project.zpj`, a
  `pool/` of audio, `plugins/` for states, a major.minor format version.
- **Studio One** `.song`: a ZIP of XML with a blob per plugin. **Renoise** `.xrns`: a ZIP with
  `Song.xml` and the samples. **Audacity 3**: one SQLite file, which brought slow closes, bloat,
  refusal on FAT drives and corruption in synced folders.

Three patterns for plugin blobs: inline base64 (every diff opaque, Ardour and REAPER); sidecar
files named by content (the text stays diffable, DAWproject, Zrythm, Ardour's LV2 states); a
parameter list as a fallback readable without the plugin (Ardour's `param-N`, LMMS). tutti uses
the last two together.

## OpenTimelineIO

The Academy Software Foundation's editorial interchange format, the best model for extensibility.

- JSON; every object tagged `"OTIO_SCHEMA": "Clip.2"`; **no file-level version**: "each data
  type is versioned separately". Keys in a predictable order, indented, never minified; no
  instancing (a reused clip is written twice).
- Media is referenced (`ExternalReference{target_url, available_range}`), never embedded; a
  `MissingReference` stands in for what is gone.
- A `metadata` dictionary on every object that the core never interprets, **namespaced per
  tool** (`metadata["mystudio"]`), carried through unchanged, and the sanctioned place to
  prototype a schema change.
- **`UnknownSchema`**: an object whose schema is not registered is kept, its original label
  remembered, and written back unchanged, because "it would be more polite to pass these through
  quietly if you don't understand them".
- Upgrade functions per type to version N, downgrade functions N to N−1, and **version
  families** that bundle a release's versions so a writer can target an older reader. Adding or
  removing a field with a sound default needs no function; renaming one does.
- The schema reference page is generated from the serializer, so docs cannot drift. Bundles:
  `.otiod` (a folder) and `.otioz` (the same tree zipped) with `content.otio` at the root and a
  flat `media/` of unique basenames.

*What tutti takes:* preservation of the unknown, namespaced vendor data, generated references.
*What it leaves:* per-object schema versions (heavy for a weekend DAW; one file version with
additive minors covers it).

## glTF 2.0

`asset.version` and `asset.minVersion`; minor versions "MUST be backward and forward compatible",
a reader ignoring what it does not know. Any object may carry `extensions`
(`{"VENDOR_feature": {…}}`) and `extras`; `extensionsUsed` and `extensionsRequired` at the top;
an extension is *required* only if a plain loader would fail without it, and material extensions
always leave a core fallback. Vendor prefixes are registered; extensions have a lifecycle (draft,
release candidate, ratified, archived). Every schema is JSON Schema 2020-12.

*What tutti takes:* used/required, the core-fallback rule, `minVersion`, additive minors. It
names namespaces reverse-DNS, as CLAP does, instead of registering prefixes.

## AAF and OMF

AAF carries its own MetaDictionary of class, property and type definitions, so a file can extend
the model, and keeps essence (media) and composition (edits) in separate trees. It is hard because
of its structured-storage binary container and a very large object spec; the maintained
OpenTimelineIO AAF adapter still cannot do effects. OMF was never standardised and has a 2 GB
limit. Lesson: self-description in the file is right; a binary compound container is not.

## Self-documenting packages

- JSON Schema has no standard way for an *instance* to name its schema; `$schema` in an instance
  is an editor convention. glTF and MNX publish schemas but their files do not point at them.
- Frictionless Data: `datapackage.json` with a `$schema` profile URL and `resources`, an
  optional `README.md`. RO-Crate: a folder with `ro-crate-metadata.json` (JSON-LD) and a human
  `ro-crate-preview.html`. BagIt (RFC 8493): `bagit.txt`, `manifest-<alg>.txt`,
  `bag-info.txt`; "complete" versus "valid" (checksums verified). OPC: a ZIP with
  `[Content_Types].xml` and typed relationships. EPUB: `mimetype` first and uncompressed, then
  `META-INF/container.xml` and a manifest of every resource.
- For agents: `llms.txt` (a Markdown index a model reads first) and `AGENTS.md` (plain Markdown,
  closest file wins, in 60 000+ repositories). Design advice from the agent-knowledge-base
  literature: "a knowledge base should explain its own structure" with schemas in the package;
  keep the key metadata in the first lines, since agents often load only the first 20 to 50;
  validation as a gate in CI. A controlled study (arXiv 2602.05447, 9 649 runs, 11 models) found
  the serialisation format (YAML, Markdown, JSON, TOON) does not significantly change an agent's
  accuracy. Structured output in the major APIs guarantees output that follows a supplied JSON
  Schema, so a published schema is what an agent generates against.

*What tutti takes:* the manifest in the document's first lines naming the format, the version,
the spec and the schema; a short README in every folder; the files index with hashes (BagIt's
check); a published JSON Schema with examples as tests (MNX's rule: "one and only one way to do
things", examples that ship as tests).

## Collaboration

- DAW files fit git badly: "most DAW session file formats are semantically scoped... rarely line
  oriented" (Paul Davis, Ardour); one sample rename fanned out to 418 clip changes in a raw diff
  (Wit's measurement); Live's XML with stable ids merges cleanly; project files are rewritten
  every save about 0.1% different, while source audio is written once and never changes (so it
  is content-addressed and kept apart).
- Every hosted collaboration service in this space died or pivoted: Splice Studio (2023),
  Endlesss and Blend (2024), Soundtrap sold back to its founders. No shipping DAW is built on a
  CRDT; Yjs appears in web experiments. The 2026 MR-DAW study found 18 of 20 interviewed musicians
  share files asynchronously.

*What tutti takes:* stable ids that never change on copy or rename, no timestamps that churn,
records one per line, blobs apart by hash, so a project merges offline and outlives any service.

## Time across DAWs

OpenTimelineIO stores `RationalTime{value, rate}`; DAWproject declares `timeUnit` per timeline
and stores warps as `(beats, seconds)` pairs, tempo and meter changes as points whose `time` and
`value` are strings (a precision worry with doubles) and states no integration rule; Live stores
`(BeatTime, SecTime)` warp markers; converting beats to seconds under tempo automation needs
segment-wise integration (the dawtool converter "supports linear automation only").

*What tutti takes:* integer ticks at a declared `ppq`; an explicit tempo map with the closed-form
integration and its rounding written in the spec, with published test vectors; both domains for
anything anchored in both (the `(frame, tick)` map).

## What the other DAWs' folders teach

The folder layout and its lessons (copy files in or they go missing; keep what can be made
again apart from what can't; delete in two steps; versions inside the folder sharing its media;
back a project up before upgrading its format; folders beat packages and databases; don't name
inner folders after the project; relink by hash, not by name) are newDAWn's research of
2026-10-14, `docs/song-folder.md` in its repository, whose layout tutti adopts.

## Sources

- **DAWproject:** the repository (README, `Project.xsd`, `MetaData.xsd`, the Java sources
  `DawProject.java`, `Referenceable.java`, `Track.java`, `Channel.java`, `Arrangement.java`,
  `device/*.java`, `timeline/*.java`) at https://github.com/bitwig/dawproject; issues #27, #39,
  #61, #91, #98, #102, #104 and the discussions there; the Bitwig FAQ
  https://www.bitwig.com/support/technical_support/system-requirements-62/;
  https://www.bitwig.com/stories/transferring-projects-between-bitwig-studio-and-other-daws-333/;
  https://bitwig.com/stories/cubase-14-now-supports-dawproject-341;
  https://www.soundonsound.com/techniques/cubase-14-using-dawproject-exchange-projects-between-cubase-cubasis;
  Steinberg's Cubase 15 press release (2025-11-05); https://forums.steinberg.net/t/cubase-14-dawproject/948676;
  https://forum.renoise.com/t/daw-project-export-tool-for-renoise-3-5-early-state/76981;
  https://github.com/git-moss/ProjectConverter; https://github.com/SatyrDiamond/DawVert;
  https://docs.rs/dawproject; https://github.com/roex-audio/DAWproject-py.
- **MIDI 2.0:** https://amei.or.jp/midistandardcommittee/MIDI2.0/MIDI2.0-DOCS/M2-116-U_v1-0_MIDI_Clip_File_Specification.pdf;
  https://www.iana.org/assignments/media-types/audio/midi-clip; https://midi.org/smf2-container-format-for-data-exchange;
  https://midi.org/the-state-of-midi-2-0-high-resolution-performance-and-the-rise-of-profiles-update-feb-2026;
  https://github.com/atsushieno/ktmidi/blob/main/docs/MIDI2_FORMATS.md.
- **CLAP and VST3:** https://github.com/free-audio/clap (`include/clap/ext/state.h`,
  `state-context.h`, `preset-load.h`, `factory/preset-discovery.h`,
  `factory/draft/plugin-state-converter.h`, `ChangeLog.md`);
  https://github.com/free-audio/clap-wrapper (`src/wrapasvst3.cpp`);
  https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Locations+Format/Preset+Format.html;
  https://github.com/steinbergmedia/vst3_public_sdk/blob/master/source/vst/vstpresetfile.h.
- **Other DAWs:** https://github.com/lucas-ywk/ableton-save; https://www.nationalarchives.gov.uk/pronom/fmt/1463;
  https://forum.ableton.com/viewtopic.php?p=1383377; https://github.com/offlinemark/dawtool;
  https://discourse.ardour.org/t/88512; https://community.ardour.org/node/15635;
  https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/session_state.cc;
  https://git.ardour.org/ardour/ardour/src/branch/master/libs/ardour/vst_plugin.cc;
  https://pypi.org/project/rpp/; https://github.com/CharlesHolbrow/rppp;
  https://raw.githubusercontent.com/LMMS/lmms/master/src/core/DataFile.cpp;
  https://manual.zrythm.org/en/projects/project-structure.html;
  https://manual.audacityteam.org/man/audacity_projects.html.
- **OpenTimelineIO:** https://opentimelineio.readthedocs.io/en/latest/tutorials/otio-file-format-specification.html;
  …/tutorials/versioning-schemas.html; …/tutorials/write-an-adapter.html;
  …/tutorials/otio-serialized-schema.html; …/tutorials/otio-filebundles.html;
  https://github.com/PixarAnimationStudios/OpenTimelineIO/commit/9459b2c3d3b2820091b6d864027f11e1af362634;
  https://github.com/OpenTimelineIO/otio-aaf-adapter.
- **glTF:** https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html;
  https://github.com/KhronosGroup/glTF/blob/main/extensions/README.md; the schema files under
  `specification/2.0/schema/`.
- **AAF and OMF:** https://en.wikipedia.org/wiki/Advanced_Authoring_Format;
  https://loc.gov/preservation/digital/formats/fdd/fdd000004.shtml;
  https://aaf.sourceforge.net/docs/aafapiman/interfaceIAAFDictionary.html;
  https://edlmax.com/EdlMaxHelp/Omf/Omf_Format.htm.
- **MusicXML and MNX:** https://www.w3.org/2021/06/musicxml40/; https://mnx.formats.music/docs/.
- **Self-documenting packages:** https://json-schema.org/understanding-json-schema/reference/schema;
  https://code.visualstudio.com/docs/languages/json; https://datapackage.org/standard/data-package/;
  https://www.researchobject.org/ro-crate/specification/1.2/; https://www.rfc-editor.org/rfc/rfc8493.html;
  https://en.wikipedia.org/wiki/Open_Packaging_Conventions; https://www.w3.org/TR/epub-33/;
  https://llmstxt.org/; https://agents.md/;
  https://okf-schema.readthedocs.io/en/latest/explanation/design-principles.html;
  https://arxiv.org/abs/2602.05447; https://developers.openai.com/api/docs/guides/structured-outputs.
- **Collaboration:** https://hn.svelte.dev/item/48258015;
  https://discourse.ardour.org/t/how-is-version-control-not-a-thing-in-daws/89124;
  https://github.com/sep-lab/Wit; https://arxiv.org/abs/2601.00326v1;
  https://cdm.link/2024/05/endlesss-discontinued/; https://splice.com/blog/?p=846;
  https://app.dealroom.co/companies/soundtrap.
- **Canonical JSON:** RFC 8785 (JSON Canonicalization Scheme), RFC 7493 (I-JSON), RFC 8259.
