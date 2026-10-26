# Conformance (tutti 1.0, draft)

What a reader and a writer each must do, the levels a program may claim, and the fixtures that
prove them. Section numbers refer to `tutti.md`.

## 1. Every reader, whatever its level

- Opens the one `.tutti` file at the top of a folder (2.1, 3.1), and refuses a folder with two.
- Checks the document is I-JSON and passes the robustness rules (3.1, 9), naming each fault by
  its JSON pointer.
- Refuses a higher major `version` by name; opens a higher `min_version` read-only (8.3).
- Applies defaults (4) and resolves ids (5.1); a dangling reference is reported, never fatal.
- **Keeps everything it does not understand** (8.2): unknown keys, unknown `type`s and `kind`s,
  every `extensions` value, and writes them back unchanged if it writes at all.
- **Never refuses a project for data it does not understand**, and says what it does not honour:
  a missing plugin (7.6), a required extension it lacks (8.1), a feature above its level.
- Never deletes a file in the folder; a hash mismatch plays silence and warns (9).
- Ignores and never deletes the reserved folders and the `private` paths (2.3).

## 2. Every writer, whatever its level

- Emits the pretty form (3.1), defaults left out (4), ids kept (5.1), new ids of its own form.
- Writes every file it refers to inside the folder, before the document that refers to it;
  writes the document last, to a temporary name renamed into place, so a crash leaves either the
  old document or the new one, never a torn one.
- **Never leaves a project less complete than it found it, and never blesses a change**: a file
  the index already lists that has gone missing or changed behind the writer's back keeps its
  entry as it was, and the writer says so; it is not dropped from the index and its new content
  is not indexed. An export (a copy, a zip, a project handed on) refuses such a file instead.
- Fills `generator`, `files`, `dependencies` (for every plugin it hosts) and
  `extensions_used`; lists in `extensions_required` only what the project needs to sound right,
  and leaves a core fallback for each (8.1).
- Writes `README.md` from `README.template.md` (2.4).
- Never renames or overwrites a file in `audio/`; names a new file uniquely (2.1).
- Keeps pattern sharing: a pattern several parts play stays one pattern (6.4).
- Writes nothing that refers outside the folder.

## 3. The levels

The levels are cumulative and describe an **implementation**. `tutti check` describes a
**project**: the features it uses, and so the lowest level a reader needs to play it in full.
A program claims a level by passing that level's fixtures; a program that passes none still
opens projects under section 1.

### Level 0: plays

Reads the document; ticks at any `ppq`; the tempo map with both curves and the integration rule
for every tick-anchored object (5.2); notes with `pitch`, `velocity` and `length`; audio pieces
with a one-point map, `bpm` a number or `null`, `gain`, `polarity`, `channel_mode`, fades and
`reverse` (5.5, 6.7); chords through their `notes`; channels: `gain`, `pan`, `mute`, `solo`,
`destination`, folders (6.1, 6.2). No plugins: a Level 0 reader may sound notes with its own
instrument.

Declared degradations a Level 0 reader may make, saying so: a piece under a `linear` tempo
segment stretched at a constant ratio per beat instead of following the ramp; a piece with a
multi-point map played by its first point and `bpm`.

### Level 1: hosts

Level 0, and: plugins by id for at least one scheme (7.2), states loaded by kind (7.4) and the
authority order kept (7.5), device order honoured (7.1), the missing-plugin behaviour (7.6),
sends and returns (6.2), automation lanes with every target form (6.8), multi-point maps (warps)
followed and audio following tempo ramps (5.5), per-note expressions (6.7), parameter events
through promoted names (6.8), the meter and key maps (5.3, 5.4), markers (6.9), a plugin's own
files given to it from its folder (7.8).

### Level 2: round-trips

Level 1, and: scenes and slots (6.10); pattern sharing kept on write; every id kept; the `files`
index complete and valid; `README.md` written; `extensions_required` honoured as 8.1 says; a
document read and written again is equal (3.1) to what was read, whichever program wrote it
before (equality leaves `generator` out).

### Profiles

- **playable anywhere**: every track with a non-builtin device carries a current track render,
  over the document's `range`, that stands in for each of them (7.7).
- **archive**: the folder carries `spec/` with this version's text, schema and agent guide
  (2.4).

## 4. The fixtures

Every fixture is a project folder under `examples/conformance/<level>/<case>/` with an
`expected.json` beside it. Expected results are **tables, never reference renders**: event times
in seconds and samples at a stated rate, RMS per channel within a stated tolerance, which
objects must survive a round trip. Engines may differ in their DSP and still pass. Audio in a
fixture is made (a sine, a click, noise), never taken from anywhere.

| Level | Case | What it proves |
|---|---|---|
| 0 | `scale` | Notes at the right seconds and samples at 120 bpm, any `ppq` (two documents, `ppq` 960 and 480, the same music). |
| 0 | `ramp` | The tempo integration vectors of 5.2, hold and linear. |
| 0 | `loop-audio` | A click loop at 95 bpm in a project at 87 bpm: the clicks' expected times, stretched, pitch kept. |
| 0 | `mixer` | Gain, pan, mute, solo: expected RMS per channel within tolerance. |
| 0 | `folders` | A folder's gain and mute reach its children; destinations resolve. |
| 0 | `preserve` | Unknown keys at the document, track and event level, an unknown event type, two extensions: read, write, equal (3.1). |
| 1 | `automation` | Device and channel targets, hold and linear, through a promoted name. |
| 1 | `sends` | A send to a return, pre- and post-fader, its RMS. |
| 1 | `warps` | A three-point map against the same material unwarped. |
| 1 | `expressions` | A per-note pitch curve heard as pitch. |
| 1 | `missing-plugin` | A device of a fictitious plugin with a state, a snapshot and a render: opens, plays the render, keeps the state file, writes edits to `overrides` only. |
| 1 | `plugin` | A tiny reference CLAP (a sine with one parameter and a state) in `fixtures/clap/`, built with the first Level 1 claimant. |
| 1 | `render-choice` | An instrument and an effect of fictitious plugins, a render without effects, one with effects and a pattern render: plays the one that stands in for both; an edit to the effect stales only the render that holds it, a note edit all three. The fingerprints are test vectors. |
| 2 | `everything` | Every feature; read and written to an equal document. |
| 2 | `merge-different-tracks` | Two branches editing different tracks (their documents in the case's `branches/`) merge in git without conflict and the result validates. |

### Hostile fixtures

Under `examples/conformance/hostile/<case>/`, each must be refused with the named error, and a
canary file one level above the project must stay untouched: `path-escape`, `symlink-audio`
(the `audio` folder a link; git keeps no link on every system, so a runner makes it in a copy of
the case, as its `expected.json` says), `size-lie`, `dup-id`, `duplicate-key` (one key twice in
an object), `send-cycle`, `nan-token`, `huge-tick`, `map-unordered`, `two-documents`,
`higher-major`, and for a project as one file (`tutti.md` 2.5) `zip-slip` (an entry whose name
leaves the folder) and `zip-bomb` (an entry that declares more than a project file may be),
each refused before anything is unpacked; `device-files-escape` (a device's own file indexed
under a path that leaves its folder). (A cycle in the track tree cannot be written: the tree is nested, so a track is
in one place; a track listed twice is `dup-id`.)

## 5. What a program may say

A program that passes a level's fixtures may say it reads tutti at that level, naming the
version ("reads tutti 1.0, Level 1"). Nothing in the format's name is trademarked. A program
that passes none may still say it opens tutti projects, and must say what it does not honour.
