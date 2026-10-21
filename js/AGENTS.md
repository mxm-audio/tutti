# AGENTS.md — js

# Purpose

`@mxm-audio/tutti`, the npm package: the core compiled to WebAssembly twice from
`crates/tutti-wasm`, for Node (`pkg/`) and for the web (`pkg-web/`); one wrapper over either
(`core.mjs`: plain objects, `Project` over a store the host supplies, `memoryStore`,
`storeError`, `packZip` and `unpackZip`); the Node entry (`index.js`: the Node build,
`nodeStore` over Node's files, `openFolder` and `createFolder`); the browser entry
(`browser.mjs`: `load()` starts the web build once, `browserFolder` a project in the
browser's own storage); the API's types (`api.d.ts`, `index.d.ts`, `browser.d.mts`), the
document types generated from the schema (`schema.d.ts`), the Node tests (`test/`) and the
browser test (`test-browser/`). Node 20.19 or later, the first that loads `core.mjs` from
`index.js`; the web build runs in browsers, Bun and Deno.

# Ownership

Owns everything here. `pkg/`, `pkg-web/` and `node_modules/` are build output and ignored;
`schema.d.ts` is generated and committed; `package-lock.json` pins the dev dependencies.

# Local Contracts

- **Generated, not edited**: `pkg/` by `npm run build` (wasm-pack, release, Node target),
  `pkg-web/` by `npm run build:web` (web target), `schema.d.ts` by `npm run types`
  (json-schema-to-typescript over `schema/<version>/tutti.schema.json`). After a schema change,
  run `npm run types` and commit the result; CI checks the committed file is current.
- **One wrapper, two entries.** Whatever isn't Node's or the browser's own is in `core.mjs`,
  made over whichever build loaded it, so the two can't drift.
- **File IO stays with the host, through a store.** `Project` runs the core's save procedure
  over any object with the synchronous methods `Store` names in `api.d.ts`, throwing errors
  whose `code` the core reads (`storeError`: `ENOENT`, `EEXIST`, `LINK`); `nodeStore` is the
  one over Node's files (references resolved inside the folder, no part a link, a new file
  flushed, the rename a replace in one step), `memoryStore` the one in memory (its times a
  clock per write; `put` and `putUnseen` for tests). `openFolder` and `createFolder` are the
  Node-only one-liners.
- **A browser's project is read whole, then written back in the format's order**
  (`browserFolder`): WebAssembly can't wait on the browser's promises, so the folder is read
  into a `memoryStore` the `Project` runs over, and `flush` writes back what changed: every
  other file first, whole, through `createWritable`, then the document under a temporary name
  put in place in one step (`move(folder, name)`, the form WebKit takes). A flush that fails
  leaves the last good document; it never removes a file.
- **The browser test runs in a profile of its own** (`launchPersistentContext`): a fresh
  Playwright context is private browsing, where WebKit gives no storage. Playwright's WebKit
  for Windows keeps nothing `createWritable` writes (2026-10-21; iOS's WebKit does), so its
  storage check reports a skip with that reason, and Chromium's runs it.
- **The name is scoped**: `tutti` on npm belongs to someone else. `private: true` until the
  owner says the package is published (pre-alpha: nothing is released).
- The Node tests are `node --test` over `test/`, no test framework; the browser test is
  `npm run test:browser` (Playwright, a pinned dev dependency).

# Work Guidance

- Keep `api.d.ts` and `core.mjs` in the same order as `crates/tutti-wasm/src/lib.rs`, one
  function each, so a reader can walk the three together; `Project`'s methods follow
  `crates/tutti-wasm/src/project.rs`.

# Verification

```bash
npm install && npm run build && npm run types && npm test
npm run build:web && npm run test:browser    # browsers: npx playwright install webkit chromium
```

`npm run build` and `build:web` need `wasm-pack` (pinned in CI) and the `wasm32-unknown-unknown`
target; Playwright's browsers go where `PLAYWRIGHT_BROWSERS_PATH` says.

# Child DOX Index

None.
