# @mxm-audio/tutti

The tutti core for JavaScript: the Rust crate compiled to WebAssembly, wrapped so you see plain
objects. Read a document in any form and get the pretty form back, validate it, check a folder's
files, compute where a tick falls in seconds or a frame in ticks, make ids, fill in a project
folder's README. The format itself is in the repository's `spec/`; the document types in
`schema.d.ts` are generated from its JSON Schema.

Pre-alpha: not published to npm; built from the repository.

```sh
npm install            # json-schema-to-typescript
npm run build          # wasm-pack build of ../crates/tutti-wasm into pkg/ (needs wasm-pack and the wasm32 target)
npm run types          # schema.d.ts from ../schema/1.0/tutti.schema.json
npm test               # node --test over the repository's examples
```

```js
const tutti = require('@mxm-audio/tutti');
const result = await tutti.checkFolder('My-Project');          // Node: validates the document and its files
console.log(result.issues, result.features, result.level);
const text = tutti.fmt(fs.readFileSync('My-Project/My-Project.tutti', 'utf8'));  // the pretty form
tutti.secondsAt(960, doc.tempo, 3840);                            // through the tempo map
tutti.ulid('evt');                                                // evt_01J…
```

Node 20 or later; the wasm build also runs in browsers (build with `--target web` instead) and
in Bun and Deno. File IO stays with the host: `checkFolder` is the one Node-only helper, and
`checkFiles` takes what you read yourself.
