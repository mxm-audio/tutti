# AGENTS.md — crates/tutti-wasm

# Purpose

The core for JavaScript: `wasm-bindgen` functions over `tutti`, strings in and strings out
(documents as their text, lists as JSON), which the npm package in `js/` wraps with plain
objects. Names mirror the Rust API (`fmt`, `jcs`, `validate`, `seconds_at`, `tick_of_frame`,
`render_fingerprint`, `ulid`, `readme`, `new_document`, …).

# Ownership

Owns `Cargo.toml` and `src/`. The package that wraps it, its types and its tests are `js/`.

# Local Contracts

- **No file IO here either**: the host reads and writes files. `check_files_with` takes what the
  host found (reference to `{sha256, bytes}`) and runs the core's files check over it through a
  folder that answers from that map.
- **wasm-bindgen is this crate's only dependency beyond the core**, pinned in the workspace; the
  crate's floor is wasm-bindgen's (`rust-version` in `Cargo.toml`), never the core's.
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
