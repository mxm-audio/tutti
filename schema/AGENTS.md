# AGENTS.md — schema

# Purpose

The JSON Schema (2020-12) of a tutti document, one file per spec version:
`schema/<version>/tutti.schema.json`, the document at the top and every object under `$defs`.

# Ownership

Owns every `schema/<version>/` folder.

# Local Contracts

- **Hand-written and normative**, never generated from the Rust types: an implementer in any
  language, or an agent with structured output, consumes it directly, and `tutti`'s types are
  tested against it, not the other way round. What a schema can't say (ids unique, references
  resolve, a hash matches its file) is `tutti::validate`'s.
- **Frozen with its spec version**, as `spec/AGENTS.md` says.
- `additionalProperties` is open on every object but the manifest's enums: unknown fields are
  the format's forward compatibility, and the schema must not refuse a newer document.

# Work Guidance

# Verification

- `cargo test -p tutti-schema`: every example validates; every invalid fixture fails at its
  pointer; the every-feature example and the schema are walked against each other both ways.

# Child DOX Index

None.
