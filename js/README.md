# @mxm-audio/tutti

The tutti core for JavaScript: the Rust crate compiled to WebAssembly, wrapped so you see plain
objects. Open, create and save a project in a store you supply, with the format's own save
procedure run by the core; read a document in any form and get the pretty form back; validate
it; compute where a tick falls in seconds or a frame in ticks; make ids; fill in a project
folder's README. The format itself is in the repository's `spec/`; the document types in
`schema.d.ts` are generated from its JSON Schema.

Pre-alpha: not published to npm; built from the repository.

```sh
npm install            # json-schema-to-typescript
npm run build          # wasm-pack build of ../crates/tutti-wasm into pkg/ (needs wasm-pack and the wasm32 target)
npm run types          # schema.d.ts from ../schema/1.0/tutti.schema.json
npm test               # node --test over the repository's examples, and projects in memory and on disk
```

```js
const tutti = require('@mxm-audio/tutti');

// A project on disk (Node): opened, checked, changed, saved.
const project = tutti.openFolder('My-Project');
console.log(project.report, project.check());               // validation, and the files
const doc = project.document;                                // an object, a fresh copy
doc.tracks[0].name = 'Lead';
project.document = doc;
project.save({ name: 'my-app', version: '0.1' });            // the format's save: verified, indexed, README, renamed into place

// A new project, a recording streamed in, a state by content.
const fresh = tutti.createFolder('New-Project', 'New Project', { name: 'my-app' });
const take = fresh.newAudio('Vox take 1', 'wav');            // audio/Vox-take-1.wav, a free name
take.write(bytes);                                           // as many times as the recorder has bytes
const stored = take.finish();                                // {reference, sha256, bytes}
const state = fresh.putState('clap', stateBytes);            // {sha256, kind, path}

// The same, in memory: a host without a file system, written out afterwards.
const store = tutti.memoryStore();
const built = tutti.Project.create(store, 'Sketch', { name: 'my-app' });
built.save({ name: 'my-app' });
store.files();                                               // Map of reference -> bytes

tutti.secondsAt(960, doc.tempo, 3840);                       // through the tempo map
tutti.ulid('evt');                                           // evt_01J…
```

**The store.** A `Project` runs over any object with the synchronous methods `Store` names in
`api.d.ts` (`list`, `read`, `size`, `modified`, `exists`, `makeFolder`, `writeNew`, `renameOver`,
`syncFolder`, `remove`), references being paths relative to the project's folder. `nodeStore`
is the one over Node's files and `memoryStore` the one in memory. A browser host prepares its
own first, since WebAssembly can't block on a promise: directory discovery and handle
acquisition are asynchronous in every browser, and synchronous access handles exist only in a
worker; so either a handle-backed store in a worker, or a memory store written out afterwards.
The package proves Node; it has no browser test.

**What `save` verifies.** A file is hashed again only when its length or modification time
differs from the record of its last verification; `verified()` gives the records to keep in a
place of your own and `trust()` takes them back at the next open, so a session hashes nothing
that hasn't changed. `saveVerified` hashes everything again, for an export.

Node 20 or later; the wasm build also runs in browsers (build with `--target web` instead) and
in Bun and Deno.
