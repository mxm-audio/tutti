# AGENTS.md — crates/tutti-fs

# Purpose

tutti on disk: `DirStore`, the core's `Store` over `std::fs`, the one-call entry points an
implementer uses, `open(folder)` and `create(folder, title, generator)`, each giving a `Project`
that does the rest, and `zip`, a project as one file (spec 2.5): `pack` and `unpack`, refusing
what a reader must refuse (a name that leaves the folder, a link, a length not as declared,
a length beyond the limit), nothing written outside the folder being made (the core's `project` module: the save procedure, the files a program puts
in, the checks). The one place in tutti's crates with file IO: the core has none and builds for
the web, and a program on a disk goes through here rather than writing the spec's procedure
itself.

# Ownership

Owns `Cargo.toml`, `src/` and `tests/`.

# Local Contracts

- **`ulid(prefix)` gives a native program new ids** in the recommended form, as the JavaScript
  package's `ulid` does: here because it needs the clock and the system's randomness, which the
  core does without.
- **The procedure is the core's; this crate is the disk.** Nothing here decides what a save
  does: `DirStore` implements the primitive operations (`list`, `exists`, `make_folder`,
  `write_new`, `open_new` and `close`, `rename_over`, `sync_folder`, `remove`) and the reads,
  each one durable by itself (a new file is flushed and synced before it counts; a rename is
  the commit point, and the folder's flush after it is its own step, so a failure there is
  reported as after the commit), and `Project` composes them. A rule about the format goes in
  the core, never here.
- **References resolve inside the root only, following no link**: every part of a reference is
  a plain name (no empty part, `.`, `..`, `\`, NUL), each existing part is checked not to be a
  link, and a link in a file's place is `FolderError::Link` (spec 9).
- **Streams, never whole**: a hash reads a file in chunks; a recording writes through a
  buffered file the core's handle owns, so a recorder runs on its own thread.
- Dependencies: `tutti` and `sha2`, pinned by the workspace, and `zip` (deflate only), pinned
  here; its floor is this crate's (`rust-version` 1.88), above the core's.

# Work Guidance

- A new operation a store needs is added to the core's `Store` trait first, with `MemoryStore`
  implementing it, then here.

# Verification

```bash
cargo test -p tutti-fs
cargo clippy -p tutti-fs --all-targets -- -D warnings
```

The tests: a project created, filled through a handle on another thread, saved, reopened and
checked; a link refused; a failure at every step of a save, through a store that fails at the
n-th operation, leaving the old document or the new one and a folder that checks out.

# Child DOX Index

None.
