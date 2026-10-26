# AGENTS.md — crates/tutti-cli

# Purpose

The `tutti` command: `check` (parse, validate, hash every indexed file, report the features a
project uses and so the conformance level a reader needs, and note each key this version doesn't
know, each stale render and each render that isn't exactly its range, never as an error),
`inspect` (the range, tracks, patterns, devices, extensions, and every render: what it covers,
the devices in it and those left out, whether it stands in for the chain and whether it is
current, which is what the next person working on a project needs of a stem), `deps` (every dependency, and whether a plugin with that id is installed in the
standard CLAP and VST3 folders), `archive` (copy the spec, schema and agent guide into a project
folder's `spec/`), `fmt` (rewrite the document in the pretty form), `zip` and `unzip` (a
project as one file, spec 2.5, through `tutti_fs::zip`). The project itself is opened, checked and
rewritten through `tutti-fs` and the core's `Project`; the library half (`src/lib.rs`) holds
what is the command's alone: the plugin folders, the installed check, `archive`.

# Ownership

Owns `Cargo.toml`, `src/` and `tests/`.

# Local Contracts

- **No file IO of the format's**: the folder is `tutti-fs`'s. What the command reads or writes
  itself is the plugin folders it scans for `deps` and the texts `archive` copies in.
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

- `cargo test -p tutti-cli`: `check` over every example and every fixture of Levels 0 to 2 and
  every hostile fixture (exit 1 with the named error; a case with a `link` in a copy where the
  link is made), its notes on the `preserve` fixture's unknown keys, `inspect` and `deps` on the
  typical project, `archive` and `fmt` on a copy.

# Child DOX Index

None.
