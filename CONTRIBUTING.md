# Contributing

Issues and pull requests are welcome.

- **Read the `AGENTS.md` chain first**: the root, then the one in each folder you touch. They are
  the working rules: what each part owns, what it must not do, and how it is checked.
- **The spec is the source of truth.** A change to the format is a change to `spec/` first, then
  to the schema, the examples and the crates, in one pull request; a declared version is never
  edited, only the next draft.
- **All three platforms**, and the web: the core crate has no file IO and must keep building for
  `wasm32-unknown-unknown`.
- **Check before you send**:

  ```bash
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```

**Before your first pull request is merged you will be asked to sign the MXM Contributor Licence
Agreement** (CLA Assistant does this on the pull request). It lets mxm keep publishing your
contribution under the MIT licence, and under other licences if the format ever moves to a
neutral home; you keep the copyright in what you wrote.
