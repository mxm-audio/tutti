# AGENTS.md — crates/tutti-cli

# Purpose

The `tutti` command: `check` (parse, validate, hash every indexed file, report the features a
project uses and so the conformance level a reader needs), `inspect` (tracks, patterns, devices,
extensions), `deps` (every dependency, and whether a plugin with that id is installed in the
standard CLAP and VST3 folders), `archive` (copy the spec, schema and agent guide into a project
folder). Folders only: a zipped project is unpacked with the user's own tools.

# Ownership

Owns `Cargo.toml` and `src/`.

# Local Contracts

- **The one place with file IO**: it implements `tutti`'s folder trait with `std::fs`, resolving
  without following links and refusing a link where the spec refuses one.
- Arguments are read by hand, as newDAWn's command line does: no argument-parsing dependency.
- Exit codes: 0 valid, 1 invalid (the reasons on stderr, one per line, with a JSON pointer where
  one applies), 2 usage.

# Work Guidance

# Verification

- `cargo test -p tutti-cli`; `tutti check` over `examples/`, and the hostile fixtures exiting 1
  with their named error.

# Child DOX Index

None.
