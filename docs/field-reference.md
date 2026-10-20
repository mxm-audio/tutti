# Field reference (tutti 1.0)

**Generated** from `schema/1.0/tutti.schema.json` by `cargo run -p tutti-schema --bin fields`; a test fails when this file
differs from the generator's output. Edit the schema, not this file. The normative text is
`spec/1.0/tutti.md`; the bounds here are its.

## The document

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `format` | `"tutti"` | yes |  | Always "tutti". |
| `version` | `version` | yes |  | The spec version the document follows, major.minor. |
| `min_version` | `version` |  |  | The lowest minor of this major that plays the document correctly (spec 8.3). Default: the major's .0. |
| `about` | string |  |  | One sentence for a stranger (spec 3.3). |
| `spec` | string |  |  | Where this version's normative text is. |
| `schema` | string |  |  | Where this version's JSON Schema is. |
| `generator` | `generator` | yes |  |  |
| `extensions_used` | array of `namespace` |  | `[]` | Every extension namespace that appears anywhere in the document (spec 8.1). |
| `extensions_required` | array of `namespace` |  | `[]` | The subset without which the project does not sound right (spec 8.1). |
| `private` | array of string, `^[A-Za-z0-9_.-]+/?$` |  | `[]` | Paths at the top of the folder that are the writer's own (spec 2.3). |
| `files` | array of `file_entry` |  | `[]` | The index of files the project refers to, sorted by path (spec 3.4). |
| `dependencies` | array of `dependency` |  | `[]` | What the project needs beyond its folder, sorted by id (spec 3.5). |
| `metadata` | `metadata` |  |  |  |
| `ppq` | integer, ≥ 1, ≤ 1048576 | yes |  | Ticks per quarter note; 960 recommended (spec 5.2). |
| `tempo` | array of `tempo_point` | yes |  | The tempo map, strictly increasing in at, the first point at 0 (spec 5.2). |
| `meter` | array of `meter_point` | yes |  | The meter map, strictly increasing in at, the first point at 0 (spec 5.3). |
| `key` | array of `key_point` |  | `[]` | The key map (spec 5.4). |
| `markers` | array of `marker` |  | `[]` | Named points and sections, sorted by id (spec 6.9). |
| `loop` | null or `loop` |  | `null` | The loop range (spec 3.6). |
| `master` | `master` |  |  |  |
| `tracks` | array of `track` |  | `[]` | The top-level tracks in display order (spec 6.1). |
| `scenes` | array of `scene` |  | `[]` | The clip launcher's scenes, in order (spec 6.10). |
| `extensions` | `extensions` |  |  |  |

## `generator`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `name` | string | yes |  | The last program that wrote the document. |
| `version` | string |  |  |  |
| `url` | string |  |  |  |

## `file_entry`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `path` | `reference` | yes |  |  |
| `sha256` | `sha256` | yes |  |  |
| `bytes` | integer, ≥ 0 | yes |  |  |

## `dependency`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `id` | string | yes |  | The plugin id in scheme form (spec 7.2). |
| `kind` | string | yes |  | plugin; content is reserved. |
| `name` | string | yes |  |  |
| `vendor` | string |  |  |  |
| `version` | string |  |  | The plugin version the project was saved with. |
| `url` | string |  |  | Where to get it, as the plugin declares it. |
| `repository` | string |  |  | A git URL, so a future program can rebuild the plugin. |
| `commit` | string |  |  |  |
| `license` | string |  |  | An SPDX identifier. |
| `formats` | array of string |  |  | clap, vst3, au, lv2, vst2. |
| `platforms` | array of string |  |  | windows, macos, linux, web. |

## `metadata`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `title` | string |  |  |  |
| `artist` | string |  |  |  |
| `album` | string |  |  |  |
| `year` | string |  |  |  |
| `genre` | string |  |  |  |
| `comment` | string |  |  |  |
| `copyright` | string |  |  |  |
| `created` | string |  |  | An ISO 8601 date, set once. |

## `tempo_point`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `at` | `ticks` | yes |  |  |
| `bpm` | `bpm` | yes |  |  |
| `curve` | `curve` |  | `"hold"` | How the tempo runs to the next point: hold (the default) or linear in ticks. |

## `meter_point`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `at` | `ticks` | yes |  |  |
| `numerator` | integer, ≥ 1, ≤ 99 | yes |  |  |
| `denominator` | `1`, `2`, `4`, `8`, `16`, `32`, `64`, `128` | yes |  |  |

## `key_point`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `at` | `ticks` | yes |  |  |
| `tonic` | `pitch_class` | yes |  |  |
| `scale` | `scale` | yes |  |  |

## `pattern_key`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `tonic` | `pitch_class` | yes |  |  |
| `scale` | `scale` | yes |  |  |

## `marker`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `id` | `id` | yes |  |  |
| `at` | `ticks` | yes |  |  |
| `name` | string |  | `""` |  |
| `kind` | string |  | `"marker"` | marker (a point) or section (a range with a length); an unknown kind reads as marker. |
| `length` | `ticks` |  |  | A section's length, required for a section. |
| `extensions` | `extensions` |  |  |  |

## `loop`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `start` | `ticks` | yes |  |  |
| `end` | `ticks` | yes |  |  |
| `enabled` | boolean |  | `false` |  |

## `master`

The master channel (spec 6.3); its id is the reserved master.

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `gain` | `gain` |  | `1` |  |
| `pan` | `pan` |  | `0` |  |
| `mute` | boolean |  | `false` |  |
| `devices` | array of `device` |  | `[]` |  |
| `extensions` | `extensions` |  |  |  |

## `track`

A track and its channel (spec 6.1); a track with children is a folder.

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `id` | `id` | yes |  |  |
| `name` | string |  | `""` |  |
| `color` | string, `^#[0-9a-f]{6}$` |  |  |  |
| `children` | array of `track` |  | `[]` |  |
| `channel` | `channel` |  |  |  |
| `devices` | array of `device` |  | `[]` | The chain in signal order (spec 7.1). |
| `parameters` | array of `promoted` |  | `[]` | Promoted parameters (spec 6.8). |
| `drum_map` | null or array of `lane` |  | `null` | A lane per drum in display order (spec 6.11). |
| `patterns` | array of `pattern` |  | `[]` | The track's own patterns, sorted by id. |
| `parts` | array of `part` |  | `[]` | Where its patterns play, sorted by id. |
| `automation` | array of `automation_lane` |  | `[]` | Automation lanes, sorted by id (spec 6.8). |
| `renders` | array of `render` |  | `[]` | Bounces of this track's chain, sorted by file (spec 7.7). |
| `slots` | array of `slot` |  | `[]` | Clip-launcher slots (spec 6.10). |
| `extensions` | `extensions` |  |  |  |

## `channel`

The mixer strip (spec 6.2).

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `gain` | `gain` |  | `1` |  |
| `pan` | `pan` |  | `0` |  |
| `mute` | boolean |  | `false` |  |
| `solo` | boolean |  | `false` |  |
| `channels` | `1`, `2` |  | `2` |  |
| `destination` | null or `id` |  | `null` | null: the parent folder's channel, or master at the top; a track id; or master. |
| `sends` | array of `send` |  | `[]` | At most one per destination, sorted by to. |

## `send`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `to` | `id` | yes |  |  |
| `gain` | `gain` |  | `1` |  |
| `pre_fader` | boolean |  | `false` |  |
| `enabled` | boolean |  | `true` |  |

## `promoted`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `name` | string | yes |  | The short name patterns and automation use. |
| `device` | `id` | yes |  |  |
| `param` | string | yes |  | The device parameter's id (spec 7.3). |

## `lane`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `pitch` | `pitch` | yes |  |  |
| `name` | string |  | `""` |  |

## `device`

A device in a chain (spec 7.1).

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `id` | `id` | yes |  |  |
| `role` | string | yes |  | instrument, note_effect, audio_effect or analyzer; an unknown role reads as audio_effect, bypassed. |
| `name` | string |  | `""` |  |
| `plugin` | string | yes |  | The plugin id in scheme form: clap:, vst3:, au:, lv2:, vst2:, builtin: (spec 7.2). |
| `aliases` | array of `alias` |  | `[]` |  |
| `state` | null or `state_ref` |  | `null` |  |
| `params` | array of `param` |  | `[]` | The parameter snapshot, sorted by id (spec 7.3). |
| `overrides` | array of `override` |  | `[]` | Values applied after the state, sorted by param (spec 7.5). |
| `enabled` | boolean |  | `true` |  |
| `extensions` | `extensions` |  |  |  |

## `alias`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `plugin` | string | yes |  |  |
| `state_compatible` | boolean |  | `false` | Whether the state bytes are the same through this alias. |

## `state_ref`

The sidecar plugins/states/<sha256>.<kind> (spec 7.4).

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `sha256` | `sha256` | yes |  |  |
| `kind` | string, `^[A-Za-z0-9]{1,8}$` | yes |  | clap, vstpreset, fxp, aupreset or lv2. |

## `param`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `id` | string | yes |  |  |
| `name` | string |  |  |  |
| `value` | number | yes |  | The plain value, authoritative. |
| `normalized` | `unit_interval` |  |  | An independent fallback for a host that only takes normalized values. |
| `min` | number |  |  |  |
| `max` | number |  |  |  |
| `unit` | string |  |  | linear, normalized, percent, decibel, hertz, semitones, seconds, beats, bpm, bool, enum, other. |
| `text` | string |  |  |  |

## `override`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `param` | string | yes |  |  |
| `value` | number | yes |  |  |
| `normalized` | `unit_interval` |  |  |  |

## `render`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `file` | `reference` | yes |  |  |
| `sha256` | `sha256` | yes |  |  |
| `from` | `ticks` | yes |  |  |
| `to` | `ticks` | yes |  |  |
| `fingerprint` | `sha256` | yes |  | The SHA-256 of the JCS bytes of the descriptor the spec defines (7.7). |

## `pattern`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `id` | `id` | yes |  |  |
| `name` | string |  | `""` |  |
| `length` | integer, ≥ 1, ≤ 1099511627776 | yes |  |  |
| `key` | null or `pattern_key` |  | `null` |  |
| `events` | array of `event` |  | `[]` | Sorted by id. |
| `extensions` | `extensions` |  |  |  |

## `part`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `id` | `id` | yes |  |  |
| `pattern` | `id` | yes |  | A pattern of the same track. |
| `at` | `ticks` | yes |  |  |
| `length` | integer, ≥ 1, ≤ 1099511627776 | yes |  |  |
| `offset` | `ticks` |  | `0` |  |
| `loop` | null or `loop_range` |  | `null` |  |
| `mute` | boolean |  | `false` |  |
| `name` | null or string |  | `null` |  |
| `extensions` | `extensions` |  |  |  |

## `loop_range`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `start` | `ticks` | yes |  |  |
| `end` | `ticks` | yes |  |  |

## `event`

A note, a chord or a param (spec 6.7); an unknown type is kept whole and plays nothing.

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `type` | string | yes |  |  |
| `id` | `id` | yes |  |  |
| `at` | `ticks` | yes |  |  |

## `note`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `length` | integer, ≥ 1, ≤ 1099511627776 | yes |  |  |
| `pitch` | `pitch` | yes |  |  |
| `velocity` | `unit_interval` | yes |  |  |
| `release` | `unit_interval` |  | `0.5` |  |
| `expressions` | `expressions` |  |  |  |
| `audio` | `audio_piece` |  |  |  |
| `extensions` | `extensions` |  |  |  |

## `expressions`

Per-note curves, at relative to the note's start (spec 6.7).

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `pitch` | array of `point` |  | `[]` | Semitones, any sign. |
| `pressure` | array of `point` |  | `[]` |  |
| `timbre` | array of `point` |  | `[]` |  |
| `volume` | array of `point` |  | `[]` |  |
| `pan` | array of `point` |  | `[]` |  |

## `point`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `at` | `ticks` | yes |  |  |
| `value` | number | yes |  |  |
| `curve` | `curve` |  | `"linear"` |  |

## `audio_piece`

Frames of a file placed in a pattern (spec 5.5, 6.7).

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `file` | `reference` | yes |  |  |
| `sha256` | `sha256` | yes |  |  |
| `sample_rate` | integer, ≥ 1, ≤ 16777216 | yes |  |  |
| `channels` | integer, ≥ 1, ≤ 256 | yes |  |  |
| `frames` | integer, ≥ 0 | yes |  |  |
| `name` | string |  |  | The name it was imported with; default the file name. |
| `from` | integer, ≥ 0 | yes |  |  |
| `to` | integer, ≥ 1 | yes |  |  |
| `map` | array of `map_point` | yes |  | Strictly increasing in both frame and tick. |
| `bpm` | null or `bpm` | yes |  | The tempo the material plays at its own speed at; null (one point only) is unwarped. |
| `gain` | `gain` |  | `1` |  |
| `polarity` | boolean |  | `false` |  |
| `channel_mode` | string |  | `"as_recorded"` | as_recorded, swap, mono, left, right. |
| `fade_in` | null or `fade` |  | `null` |  |
| `fade_out` | null or `fade` |  | `null` |  |
| `reverse` | boolean |  | `false` |  |
| `extensions` | `extensions` |  |  |  |

## `map_point`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `frame` | integer, ≥ 0 | yes |  |  |
| `tick` | `signed_ticks` | yes |  |  |

## `fade`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `length` | `ticks` | yes |  |  |
| `outside` | `ticks` |  | `0` | How much of the fade lies past the piece's edge, in the file beyond the cut. |
| `shape` | string |  | `"linear"` | linear, equal_power, s_curve. |

## `chord`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `length` | integer, ≥ 1, ≤ 1099511627776 | yes |  |  |
| `root` | `pitch_class` | yes |  |  |
| `quality` | string | yes |  | maj, min, dim, aug, maj7, min7, 7, min7b5, dim7, sus2, sus4, 6, min6, 9, maj9, min9, add9, 11, 13, 5; an unknown quality plays its notes. |
| `bass` | null or `pitch_class` |  | `null` |  |
| `symbol` | string |  |  |  |
| `notes` | array of `pitch` | yes |  | The voicing to play. |
| `velocity` | `unit_interval` |  | `0.8` |  |
| `extensions` | `extensions` |  |  |  |

## `param_event`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `param` | string | yes |  | A promoted parameter's name on the track. |
| `value` | `unit_interval` | yes |  |  |
| `extensions` | `extensions` |  |  |  |

## `automation_lane`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `id` | `id` | yes |  |  |
| `target` | `target` | yes |  |  |
| `points` | array of `point` |  | `[]` | Strictly increasing in at, absolute ticks on the track. |
| `extensions` | `extensions` |  |  |  |

## `target`

Exactly one of: device and param; channel (gain, pan, mute, or send with to); parameter (spec 6.8).

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `device` | `id` |  |  |  |
| `param` | string |  |  |  |
| `channel` | string |  |  |  |
| `to` | `id` |  |  |  |
| `parameter` | string |  |  |  |

## `scene`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `id` | `id` | yes |  |  |
| `name` | string |  | `""` |  |
| `extensions` | `extensions` |  |  |  |

## `slot`

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `scene` | `id` | yes |  |  |
| `pattern` | `id` | yes |  | A pattern of the same track. |
| `loop` | null or `loop_range` |  | `null` |  |

