# Implementing tutti

For someone writing a program that opens or saves tutti projects, in any language. The
normative text is [`spec/1.0/tutti.md`](../spec/1.0/tutti.md); what a reader and a writer each
must do, and the levels a program may claim, are [`spec/1.0/conformance.md`](../spec/1.0/conformance.md).
This page says how to get there with the least work and the fewest mistakes.

## Three ways in

- **Rust.** Take `tutti-fs` (the disk) and `tutti` (the core). `tutti_fs::open(folder)` gives a
  `Project` with the document read and validated; `tutti_fs::create(folder, title, generator)`
  makes one. Put files in with `new_audio` (a writer to stream a recording through),
  `put_audio`, `put_state`, `put_render` and `device_files`; edit the document; `save`. The crate
  does the rest: names and hashes the files, builds the index from them, validates, writes the
  README, renames the document into place. You never write the spec's procedure yourself.
- **JavaScript.** `@mxm-audio/tutti` (`js/`) is the same core compiled to WebAssembly, with the
  same `Project` over a store you supply: `nodeStore` for Node, `memoryStore` for a host
  without a file system, or your own object with the same synchronous methods. In a browser,
  `@mxm-audio/tutti/browser`'s `browserFolder` reads a project from the browser's own storage
  into a memory store and writes a save back in the format's order; WebAssembly can't wait on a
  promise, so a store of your own is prepared first the same way. `packZip` and `unpackZip`
  make and read the one-file form in memory.
- **Any other language.** The document is JSON; the schema
  ([`schema/1.0/tutti.schema.json`](../schema/1.0/tutti.schema.json)) is what to generate and
  validate against; the spec's section 10 is the procedure, and section 9 what to check. Run
  `tutti check` over what you write until it says valid, and keep it in your tests. A C ABI over
  the core is planned, so a C++ or Python program can take the procedure rather than write it.

The spec is the authority, `tutti check` is the gate, and the crate is the one implementation we
ship and every binding wraps. A program that writes the folder itself and passes `check` is as
much a tutti program as one that uses the crate.

## What a reader does

In the order the checks happen (conformance 1):

1. **One document.** The one `.tutti` file at the folder's top; two is an error.
2. **I-JSON.** Refuse a byte-order mark, a duplicate key, a number that is not a number.
3. **The version.** A higher major is refused by name, with the spec URL the document carries.
   A higher `min_version` within the major opens read-only: play and export, never edit.
4. **Defaults and ids.** A missing key has the default the spec names. Every reference is an id
   in a key named for its target; one that resolves to nothing is reported, never fatal.
5. **Keep what you don't understand.** Unknown keys, unknown `type`s and `kind`s, every
   `extensions` value: in your tree, and back out if you write.
6. **Play what you can, say what you don't.** A missing plugin, a required extension you lack,
   a feature above your level: the user hears it by name. Never refuse a project for data you
   don't understand, and never delete anything. For a missing plugin, look for a current render
   that stands in for it (`render::stands_in`, spec 7.6 and 7.7): play it in place of the devices
   it lists, run the rest of the chain yourself.
7. **Files by hash.** A file whose hash doesn't match plays silence and warns. Resolve a
   reference inside the folder only, never through a link.

## What a writer does

In the order the save happens (conformance 2; `Project::save` does exactly this):

1. Every file the document will refer to is in the folder first: audio under `audio/`, never
   renamed or overwritten; states under `plugins/states/` by content hash with their kind; a
   plugin's own files under `plugins/files/<device id>/`; renders under `renders/`, each exactly
   its range, its `devices` saying what is in it and its `fingerprint` from `render_fingerprint`
   or `pattern_render_fingerprint` over the document as it is saved.
2. The index (`files`) names each of them with its hash and length, built from the files, not
   from what the document claims. A new reference to a file that isn't there is refused; a file
   the index already listed that went missing or changed keeps its entry as it was and is
   reported, never repaired by indexing what is there now.
3. `generator`, `dependencies` (every plugin a device uses, with its version and where it comes
   from; one no device uses goes), `extensions_used`, and `extensions_required` only for what
   the sound depends on, each with a core fallback.
4. The document in the pretty form: the spec's key order, two-space indent, an event, a point
   and a file entry each on one line, defaults left out, numbers as JavaScript renders them.
5. `README.md` from the template.
6. The document to a temporary name, then renamed over the old one: the commit point. A crash
   on either side leaves the old document or the new, never a torn one.

Ids: keep every id you read; give your own objects ids of your own form, `<kind>_<ULID>`
recommended, and never change one on copy, move or save.

Through the crate, most of this is done for you: `Project::save` builds the index, keeps
`dependencies` in step with the devices (fill an entry's vendor, version and URL before you
save; the crate adds the least the spec asks for any you don't, and drops one no device uses),
computes `extensions_used`, writes the README and commits. The model's constructors
(`Device::new`, `Note::new` and the rest) carry the spec's defaults, and `tutti_fs::ulid` (or the
JavaScript package's `ulid`) gives a new id.

## Verification, cheaply

A file is verified by its length and modification time: hashed again only when either differs
from the record of its last verification, and never otherwise. Keep the records between
sessions in a place of your own (`Project::verified` and `trust`; the document never carries
them, since a modification time is one file system's). A file changed behind the project's back
is a breach of the folder's rules, and the hash rule is what catches it, in `check` and at
playback; a save need not defend against it. Rehash everything only for an export
(`save_verified`).

## The levels, and claiming one

`tutti check <folder>` says which features a project uses and so the lowest level a reader needs
to play it in full. A program claims a level by passing that level's fixtures under
`examples/conformance/`: Level 0 plays (the time model, notes, audio pieces, channels), Level 1
hosts (plugins, states, sends, automation, warps, expressions, markers, a plugin's own files),
Level 2 round-trips (everything, including what it doesn't model). A program that passes none
still opens projects under the rules above and says what it doesn't honour; that is what
newDAWn, the first consumer, does today.

## Pitfalls, each with its rule

- **Time in seconds anywhere in the document.** Never: ticks at the declared `ppq`, seconds
  derived through the tempo map with the spec's closed form (5.2), whose test vectors are in
  `examples/conformance/l0/ramp`.
- **A mean-tempo shortcut under a ramp.** It is 106 ms off over four beats; integrate.
- **Audio placed by seconds or by a stretch ratio.** One rule: `(frame, tick)` points plus the
  tempo the material was recorded at (5.5); a one-point map and `bpm: null` is unwarped audio.
- **A state of one kind fed to a plugin of another.** Garbage, usually silently: `kind` names
  the bytes' convention, and only an alias marked `state_compatible` may cross.
- **A plugin's sample path inside its state.** It travels only as far as that path. Give the
  plugin its folder (`plugins/files/<device id>/`, 7.8) and ask it to collect what it uses
  (CLAP's resource-directory extension); say in your docs which plugins can't.
- **Automation or parameter events in normalized values.** The plain value, as the parameter's
  snapshot has it (6.7, 6.8); CLAP has no normalized form at all.
- **Dropping what you don't model.** The next program's work, gone. Keep it, on the object it
  came on, found by id.
- **A zip of your own shape.** The zip form is defined (2.5): the document first and stored,
  entry names a relative path of plain parts, no declared length trusted. `tutti zip` and `unzip`
  do it; the `zip-slip` and `zip-bomb` fixtures are what a reader must refuse.

## Where things are

- The spec, conformance and extensions texts: `spec/1.0/`.
- The schema: `schema/1.0/tutti.schema.json`; the generated field reference:
  [`field-reference.md`](field-reference.md).
- The agent guide, what a program reads first: [`agent-guide.md`](agent-guide.md).
- The examples and fixtures: `examples/`; the command: `cargo run -p tutti-cli -- check <folder>`.
- The research the design rests on: [`research.md`](research.md); the reasons: `NOTES.md`.
