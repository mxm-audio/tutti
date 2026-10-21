# AGENTS.md — crates/tutti-wasm

# Purpose

The core for JavaScript: `wasm-bindgen` functions over `tutti`, strings in and strings out
(documents as their text, lists as JSON), which the npm package in `js/` wraps with plain
objects, and `Project` (`src/project.rs`): the core's project over a store the host supplies,
an imported type with synchronous methods, so the format's save procedure runs once, in the
core, for JavaScript too. Names mirror the Rust API (`fmt`, `jcs`, `validate`, `seconds_at`,
`tick_of_frame`, `render_fingerprint`, `ulid`, `readme`, `new_document`, `Project::open`,
`save`, `put_audio`, `new_audio`, …).

# Ownership

Owns `Cargo.toml` and `src/`. The package that wraps it, its types and its tests are `js/`.

# Local Contracts

- **No file IO here either**: the host's store reads and writes files. `HostStore` implements
  the core's `Folder` and `Store` over the imported `JsStore`, mapping a thrown error's `code`
  (`ENOENT`, `EEXIST`, `LINK`) to the core's; a recording buffers in Rust and is written at
  `finish`, the store writing whole files. `check_files_with` takes what the host found
  (reference to `{sha256, bytes}`) and runs the core's files check over it. `pack` and
  `unpack` are `tutti-zip`'s over the host's store, the one file as bytes.
- **wasm-bindgen and js-sys are this crate's only dependencies beyond tutti's own crates** (the
  core and `tutti-zip`, and serde, the core's), pinned in the workspace; the crate's floor is
  wasm-bindgen's (`rust-version` in `Cargo.toml`), never the core's.
- A function that can refuse returns `Result<_, JsError>` with the core's own message, so a
  JavaScript caller sees the same reason the command prints.
- Not published to crates.io (`publish = false`): the npm package is the product.

# Work Guidance

- A new core function reaches JavaScript by one function here and one line in `js/index.js`
  (and `js/api.d.ts`), nothing more.

# Verification

```bash
cargo build -p tutti-wasm --target wasm32-unknown-unknown
cargo clippy -p tutti-wasm --all-targets -- -D warnings
cd js && npm run build && npm test     # the package's tests exercise every function
```

# Child DOX Index

None.
