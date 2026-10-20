# tutti

**An open audio project format.** A project is a plain folder: one JSON document that names its
own format and spec in its first lines, the audio it plays, the states of the plugins it uses, and
a short README that says how it all fits. Start a project in one program and open it in another,
including programs that don't exist yet.

- **Interoperable**: tracks, patterns and parts, notes and audio pieces, devices and their states,
  mixer, automation, markers, tempo and meter maps, with every unit and rule stated.
- **Expandable**: vendor extensions in namespaces, `extensions_used` and `extensions_required`,
  and the rule that a reader keeps what it doesn't understand and writes it back.
- **Self-documenting**: the document says what it is and where its spec and schema are; the
  folder carries a README a person or an agent reads first; the spec ships with a JSON Schema,
  worked examples and conformance fixtures.
- **Not only Rust**: the schema is the first API, any language reads the documents; the core
  crate has no file IO and compiles to WebAssembly for the JavaScript package.

Pre-alpha. The spec's first version is being written; nothing is released, and the format may
still change in every detail. The first program to read and write it is
[newDAWn](https://github.com/maxmcorp/newDAWn).

## In this repository

| Path | What |
|---|---|
| `spec/<version>/` | The normative text, frozen once a version is declared; `spec/CHANGELOG.md` |
| `schema/<version>/` | The JSON Schema (2020-12), hand-written, frozen with its version |
| `examples/` | Example projects and the conformance fixtures |
| `docs/` | The research behind the format, the implementers' guide, the agent guide |
| `crates/tutti` | The model, canonical reading and writing, validation, the time arithmetic |
| `crates/tutti-schema` | The tests that hold the schema, the examples and the types together |
| `crates/tutti-cli` | `tutti check`, `tutti inspect`, `tutti deps`, `tutti archive`, `tutti fmt` |
| `crates/tutti-wasm`, `js/` | The core for JavaScript, and `@mxm-audio/tutti`, the npm package around it |

## Licence

MIT (`LICENSE`): the code, the schema, the spec text and the examples alike, so any program,
open or closed, can read and write the format, and anyone may say their program reads tutti.
Contributions are welcome: see `CONTRIBUTING.md`. How the repository is organised, and the rules
each part keeps, are in `AGENTS.md`.
