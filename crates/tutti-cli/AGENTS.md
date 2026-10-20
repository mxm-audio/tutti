# AGENTS.md — crates/tutti-cli

# Purpose

The `tutti` command: `check` (parse, validate, hash every indexed file, report the features a
project uses and so the conformance level a reader needs), `inspect` (tracks, patterns, devices,
extensions), `deps` (every dependency, and whether a plugin with that id is installed in the
standard CLAP and VST3 folders), `archive` (copy the spec, schema and agent guide into a project
folder's `spec/`), `fmt` (rewrite the document in the pretty form). Folders only: a zipped
project is unpacked with the user's own tools. Its library half (`src/lib.rs`) is what the tests
and the generators share: `DirFolder`, the one document in a folder, the plugin folders, the
installed check, `archive`.

# Ownership

Owns `Cargo.toml`, `src/` and `tests/`.

# Local Contracts

- **The one place with file IO**: it implements `tutti`'s folder trait with `std::fs`, resolving
  without following links and refusing a link where the spec refuses one.
- Arguments are read by hand, as newDAWn's command line does: no argument-parsing dependency.
- **`deps` says how sure it is.** A CLAP id lives inside the binary, so a `.clap` named like the
  id's last segment is *likely installed*; a VST3 class id listed in a bundle's `moduleinfo.json`
  is *installed*. Nothing is loaded to find out.
- The spec, the schema and the agent guide `archive` copies are compiled in (`ARCHIVE_FILES`),
  so the command carries this version's texts wherever it goes.
- Exit codes: 0 valid, 1 invalid (the reasons on stderr, one per line, with a JSON pointer where
  one applies), 2 usage.

# Work Guidance

# Verification

- `cargo test -p tutti-cli`: `check` over every example and every hostile fixture (exit 1 with
  the named error), `inspect` and `deps` on the typical project, `archive` and `fmt` on a copy.

# Child DOX Index

None.
