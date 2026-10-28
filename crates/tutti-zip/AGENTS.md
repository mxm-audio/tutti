# AGENTS.md — crates/tutti-zip

# Purpose

A project as one file, `<Name>.tutti.zip` (spec 2.5): `pack`, a project from any store into
anything that writes and seeks, and `unpack`, anything that reads and seeks into any store,
refusing what a reader must refuse (a name that leaves the folder, a link, a length not as
declared, a length beyond the limit) with nothing it wrote left behind. One implementation for
a file on disk (`tutti-fs`'s `zip`) and for bytes in memory (`tutti-wasm`, the JavaScript
package): the rules the core states without IO are `tutti::zip`'s, the procedure is here.

# Ownership

Owns `Cargo.toml` and `src/`.

# Local Contracts

- **No file IO**: the archive is the caller's `Read + Seek` or `Write + Seek`, the project the
  core's `Store`. The disk around it (a temporary file renamed into place, a folder made and
  removed again) is `tutti-fs`'s.
- **A file is copied through `Folder::copy_to`**, which a store that can stream overrides, so
  the disk packs a large recording without holding it whole.
- **Names first**: every entry's name is checked before a byte is written, and the store is
  checked to hold no project before the first file goes in (`ZipError::HoldsProject`).
- Dependencies: `tutti` and `zip` (deflate only, pinned here); the crate's floor is the zip
  crate's (`rust-version` 1.88), above the core's. It builds for `wasm32-unknown-unknown`.

# Work Guidance

- A change to what is packed or refused lands here, once, and reaches the disk and JavaScript
  together; a rule the core can state without IO goes in `tutti::zip`.

# Verification

```bash
cargo test -p tutti-zip
cargo clippy -p tutti-zip --all-targets -- -D warnings
cargo build -p tutti-zip --target wasm32-unknown-unknown
```

Its tests pack a project in memory and unpack it into another store the same, leave `private/`
out both ways (spec 2.3), and refuse a store holding a project; the hostile fixtures are unpacked by the command's tests (`tutti-cli`)
and the package's (`js/test/zip.test.js`).

# Child DOX Index

None.
