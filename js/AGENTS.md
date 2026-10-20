# AGENTS.md — js

# Purpose

`@mxm-audio/tutti`, the npm package: the wasm build of `crates/tutti-wasm` (`pkg/`), the wrapper
that turns its strings into plain objects (`index.js`), the API's types (`api.d.ts`), the document
types generated from the schema (`schema.d.ts`), and the tests that run the package over the
repository's examples (`test/`). Node 20 or later; the wasm runs in browsers, Bun and Deno too.

# Ownership

Owns everything here. `pkg/` and `node_modules/` are build output and ignored; `schema.d.ts` is
generated and committed; `package-lock.json` pins the one dev dependency.

# Local Contracts

- **Generated, not edited**: `pkg/` by `npm run build` (wasm-pack, release, Node target),
  `schema.d.ts` by `npm run types` (json-schema-to-typescript over
  `schema/<version>/tutti.schema.json`). After a schema change, run `npm run types` and commit
  the result; CI checks the committed file is current.
- **File IO stays with the host.** `checkFolder` is the one Node-only helper (it reads the
  folder, hashes the files with `node:crypto`, and hands the core what it needs); everything
  else takes strings and objects. A link under `audio/`, `plugins/states/` or `renders/` is
  reported as a reader refuses it.
- **The name is scoped**: `tutti` on npm belongs to someone else. `private: true` until the
  owner says the package is published (pre-alpha: nothing is released).
- The tests are `node --test` over `test/`, no test framework.

# Work Guidance

- Keep `api.d.ts` and `index.js` in the same order as `crates/tutti-wasm/src/lib.rs`, one
  function each, so a reader can walk the three together.

# Verification

```bash
npm install && npm run build && npm run types && npm test
```

`npm run build` needs `wasm-pack` (pinned in CI) and the `wasm32-unknown-unknown` target.

# Child DOX Index

None.
