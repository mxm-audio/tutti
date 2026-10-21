# Minimal

This folder is an **audio project in the tutti format** (version 1.0), written by
tutti-examples 1.0. Everything the project uses is in this folder; nothing in it refers outside.

## What is here

- `Minimal.tutti`: the project, one JSON document. Its first lines name the format, the version,
  the spec and the schema. Open it with any JSON reader.
- `audio/`: every sound the project plays, each known by its SHA-256.
- `plugins/states/`: the saved states of the plugins it uses, each named by its content's SHA-256.
- `renders/`: bounces the project may carry so it plays where its plugins are missing.
- `versions/`, `exports/`, `trash/`, `log/`, `lock`: the writing program's own. Leave them alone.

## The five rules

1. One project per folder; the document is the one `.tutti` file at the top.
2. Audio is never renamed and never overwritten.
3. A file is identified by its SHA-256, not by its name.
4. Nothing refers outside the folder, except the plugins the document names, which are
   installed, and listed with their versions and sources under `dependencies`.
5. A program keeps what it does not understand, and writes it back.

## Time, in three sentences

Musical time is an integer count of ticks, `ppq` of them per quarter note (`ppq` is in the
document). The tempo map turns ticks into seconds: at a constant tempo `A` bpm, a tick `t` after
a tempo point at tick `a` falls `60 · (t − a) / (ppq · A)` seconds after it; a `linear` segment
from `A` to `B` bpm over ticks `a` to `b` integrates to `60 / (ppq · k) · ln((A + k · (t − a)) /
A)` with `k = (B − A) / (b − a)`. Audio is placed by `(frame, tick)` points plus the tempo its
material was recorded at, so `map: [{frame: 0, tick: 0}], bpm: 95` means frame 0 sits at tick 0
and the file plays at its own speed when the project runs at 95 bpm.

Two values to check a reader by: at `ppq` 960, tick 7 680 is beat 8, and beat 8 at 87 bpm is
5.517241 s from the start.

## Where the rules are

- The spec: https://github.com/mxm-audio/tutti/blob/main/spec/1.0/tutti.md
- The schema: https://github.com/mxm-audio/tutti/blob/main/schema/1.0/tutti.schema.json
- For agents and implementers: https://github.com/mxm-audio/tutti/blob/main/docs/agent-guide.md
- The reference implementation and the `tutti` command: https://github.com/mxm-audio/tutti
