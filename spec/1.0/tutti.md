# tutti 1.0 (draft)

**An open audio project format.** This is the normative text of version 1.0. *Draft*: it is
being written, and nothing in it is declared until this heading loses the word. The words MUST,
MUST NOT, SHOULD and MAY are used as in RFC 2119. A *reader* is any program that opens a project;
a *writer* is any program that saves one; most programs are both.

Companion texts in this folder: `conformance.md` (what a reader and a writer each must do, and
the levels a program may claim), `extensions.md` (vendor namespaces and the registry) and
`README.template.md` (the README every writer puts in a project folder). The JSON Schema is
`schema/1.0/tutti.schema.json` in the repository; the reference implementation is the `tutti`
crate; the research the design rests on is `docs/research.md`.

## 1. What a project is

A project is a **plain folder** holding **one JSON document**, `<Name>.tutti`, and **every file
the project uses**: the audio it plays, the states of the plugins it hosts, the renders it may
carry. Nothing in a project refers to a file outside its folder, except the plugins it names,
which are installed, and whatever those plugins load by themselves.

A project is for any audio work a program edits on a timeline: a song, a podcast, a film cue, a
sound design session, a live set. The format calls it a project throughout.

The five rules a project keeps:

1. **One project per folder**, the document at the top, exactly one `.tutti` file.
2. **Audio is never renamed and never overwritten**; the project knows each file by its SHA-256.
3. **Hashes are identity**: a plugin state, a render and an audio file are named and found by
   their content.
4. **Nothing refers outside the folder.**
5. **Unknown data is kept**: a reader never refuses a project for data it doesn't understand, and
   a writer writes that data back.

## 2. The folder

```
My-Project/
├─ My-Project.tutti            the document: the whole project, JSON
├─ README.md                   what this folder is, from README.template.md
├─ audio/                      every sound the project plays
├─ plugins/states/<sha256>.<kind>   plugin states, content-addressed
├─ plugins/files/<device id>/       a plugin's own files (reserved; see 7.4)
├─ renders/                    bounces the project carries so it plays without its plugins
├─ analysis/                   results an application keeps beside the audio (reserved name)
└─ versions/ exports/ trash/ log/ lock    the writing program's own (see 2.3)
```

### 2.1 Names

- Folder names are fixed, lower case and English, as above.
- Every file name a writer gives keeps to the characters `A-Z a-z 0-9 - _` and one `.` before
  the extension: it MUST match `^[A-Za-z0-9_-]{1,120}\.[A-Za-z0-9]{1,8}$`. Names are unique
  ignoring case within a folder.
- The document's name is the project's title in file form (the same characters), and renaming
  the folder changes nothing inside. A reader opens the one `.tutti` file at the top whatever its
  name; two or more is an error (3.1).

### 2.2 References

A reference inside the document is a bare file name in a fixed folder, never a path:
`audio/<name>` for audio, `plugins/states/<sha256>.<kind>` for states, `renders/<name>` for
renders. A reader resolves a reference inside the folder only, without following links (9).

### 2.3 Reserved and private paths

`versions/`, `exports/`, `trash/`, `log/` and `lock` are **reserved for the writing program's own
use**: its kept versions, its exports, what it has set aside, its journal and its lock. A reader
that is not that program MUST ignore them and MUST NOT delete them. The document's `private`
list names any further paths that are the writer's own (3.2). A reader reads only the document,
`audio/`, `plugins/`, `renders/` and `analysis/`.

### 2.4 README.md

Every writer puts a `README.md` at the top of the folder, from `README.template.md`, filling in
the title and the generator: what the folder is, the layout, the five rules, the time model in
three sentences with the tempo formula and two worked values, and the spec, schema and
agent-guide URLs. It is about 2 kB, the first thing a person or an agent reads, and enough to
reconstruct the project if the URLs are ever dead. A writer MAY also copy the spec, the schema
and the agent guide into `spec/` at the top of the folder (an *archive*; `conformance.md`).

## 3. The document

### 3.1 JSON

The document is **I-JSON** (RFC 7493): UTF-8 without a byte-order mark, valid per RFC 8259, no
duplicate keys in any object (a reader MUST refuse duplicates), every number finite (no `NaN`
or `Infinity`), every integer within ±2^53, every string valid Unicode. A reader accepts any
whitespace and any key order.

Two canonical forms exist, both exact:

- **The pretty form** is what a writer emits on disk: UTF-8, LF line endings, one final LF,
  two-space indentation, no minification; in every object the known keys in the order this
  text lists them, then unknown keys sorted by UTF-16 code units; an array of scalars on one
  line; an array of objects one element per line, each element written compactly on its one
  line (one space after each `:` and `,`, so an event, a point, a part, a send, a `files` entry
  each take one line), except the elements of `tracks`, `children`, `patterns`, `devices` and
  `automation`, which hold arrays of their own and are written multi-line; an empty object or
  array as `{}` or `[]`; numbers and strings rendered as JCS renders them
  (ECMAScript's Number-to-string, JCS's string escaping); a key whose value is its default left
  out, except a required key (4). Two writers given the same project produce the same bytes.
- **JCS** (RFC 8785, the JSON Canonicalization Scheme) is what **equality and fingerprints** are
  over: keys sorted by UTF-16 code units, no whitespace, ECMAScript number rendering, JCS string
  escaping. Two documents are *equal* when their JCS bytes are equal after defaults are applied
  (4); a *fingerprint* is the SHA-256 of JCS bytes.

A project folder with two or more `.tutti` files at the top is invalid; a reader reports both
names and opens neither.

### 3.2 The manifest

The document's first keys are its manifest, so a program that opens it learns in the first lines
what it is and where the rules are.

| Key | Type | Required | Meaning |
|---|---|---|---|
| `format` | string | yes | Always `"tutti"`. |
| `version` | string | yes | The spec version this document follows, `major.minor`: `"1.0"`. |
| `min_version` | string | no (default `"<major>.0"`) | The lowest minor of this major that plays the document correctly (8.3). |
| `about` | string | no | One sentence for a stranger. Writers SHOULD write the sentence in 3.3. |
| `spec` | string (URL) | no | Where this version's normative text is; SHOULD be written. |
| `schema` | string (URL) | no | Where this version's JSON Schema is; SHOULD be written. |
| `generator` | object | yes | The last program that wrote the document: `name` (required), `version`, `url`. |
| `extensions_used` | array of string | no (default `[]`) | Every extension namespace that appears anywhere in the document (8.1). |
| `extensions_required` | array of string | no (default `[]`) | The subset without which the project does not sound right (8.1). |
| `private` | array of string | no (default `[]`) | Paths inside the folder that are the writer's own (2.3), each a folder name ending in `/` or a file name. |
| `files` | array of object | no (default `[]`) | An index of the files the project refers to (3.4). |
| `dependencies` | array of object | no (default `[]`) | What the project needs beyond its folder (3.5). |
| `metadata` | object | no (default `{}`) | `title`, `artist`, `album`, `year`, `genre`, `comment`, `copyright`, `created` (an ISO 8601 date, set once), all strings, all optional. No modification time: it would change on every save for nothing. |

### 3.3 The `about` sentence

> An open audio project (tutti 1.0): this folder holds every file it uses. Time is integer
> ticks, 'ppq' per quarter note, placed in seconds by 'tempo'. Audio is placed by (frame, tick)
> points. Keep what you don't understand.

### 3.4 The files index

`files` lists every file in `audio/`, `plugins/states/` and `renders/` that the project refers
to, one entry each: `path` (the reference as in 2.2), `sha256` (64 lower-case hexadecimal
digits), `bytes` (the file's length). It is an *index*: it lets a program check a folder is
complete (every path present) and valid (every hash matches) without reading the model, zip a
project, or clean a folder. The references in the model are authoritative; a disagreement between
them and the index is reported, never fatal. The index is sorted by `path`.

### 3.5 Dependencies

`dependencies` says what the project needs that is not in its folder: one entry per distinct
plugin id any device uses, and (reserved for a later minor version) per content pack a plugin
loads from outside. **The format declares; a program resolves.**

| Key | Type | Required | Meaning |
|---|---|---|---|
| `id` | string | yes | The plugin id, in the scheme form of 7.2 (`clap:dk.mxm.mxm-mono-01`). |
| `kind` | string | yes | `plugin`; `content` is reserved. |
| `name` | string | yes | The plugin's name as it shows itself. |
| `vendor` | string | no | As the plugin declares it (CLAP's descriptor, VST3's factory). |
| `version` | string | no | The plugin version the project was saved with. |
| `url` | string | no | Where to get it, as the plugin declares it (CLAP `url`, VST3 factory `url`). |
| `repository` | string | no | A git URL, when known, so a future program can rebuild the plugin. |
| `commit` | string | no | The commit the plugin was built from, when known. |
| `license` | string | no | An SPDX identifier. |
| `formats` | array of string | no | The ABIs the plugin ships as: `clap`, `vst3`, `au`, `lv2`, `vst2`. |
| `platforms` | array of string | no | `windows`, `macos`, `linux`, `web`. |

A writer that hosts a plugin fills its entry from what the plugin declares; a reader that lacks a
plugin shows its entry (7.6). Entries are sorted by `id`.

### 3.6 The rest of the document

After the manifest, in this order:

| Key | Type | Required | Meaning |
|---|---|---|---|
| `ppq` | integer | yes | Ticks per quarter note, 1 to 2^20. 960 is recommended. |
| `tempo` | array of tempo point | yes | The tempo map (5.2); at least one point, the first at tick 0. |
| `meter` | array of meter point | yes | The meter map (5.3); at least one point, the first at tick 0. |
| `key` | array of key point | no (default `[]`) | The key map (5.4). |
| `markers` | array of marker | no (default `[]`) | Named points and sections (6.9). |
| `loop` | object or null | no (default `null`) | `start`, `end` (ticks, start < end), `enabled` (default `false`). |
| `master` | object | no (default: its defaults) | The master channel and its devices (6.3). |
| `tracks` | array of track | no (default `[]`) | The top-level tracks in display order (6.1). |
| `scenes` | array of scene | no (default `[]`) | The clip launcher's scenes (6.10). |
| `extensions` | object | no (default `{}`) | Vendor data on the document (8.1). |

## 4. Defaults and what is written

Every key below has a type and, unless required, a **default**. A reader treats an absent key as
its default. A writer in the pretty form **leaves out a key whose value is its default**, and
always writes a required key. Equality (3.1) is judged after defaults are applied, so a document
with a default written out and one without are equal.

Every enumeration names a **fallback**: the value a reader uses when it meets a value it does
not know. A reader keeps the unknown value as read (8.2) and plays the fallback.

## 5. Identity and time

### 5.1 Ids

An id is an opaque string matching `^[A-Za-z0-9_-]{1,64}$`, **unique across the whole document
and every kind of object**. References are by id, in a key named for the target, never by array
index. An id never changes on copy, move, rename or save, and a reader never invents a new id for
an object that had one.

Writers SHOULD use a kind prefix and a ULID: `trk_01JB2K7Q9M3F5A7Y9R2E4W6N`, with the prefixes
`trk` (track), `pat` (pattern), `prt` (part), `evt` (event), `dev` (device), `aut` (automation
lane), `mrk` (marker), `scn` (scene). Two projects, or two branches of one, then never collide,
so a merge needs no counter. A reader keeps every id it read whatever its form; it uses its own
form only for objects it creates. The id `master` is reserved for the master channel (6.3).

Ids are required on every identified object: tracks, patterns, parts, events, devices,
automation lanes, markers and scenes. Points (tempo, meter, key, automation, expression, map)
have no ids; their time orders them.

**An id names one object.** *Transfer* keeps ids: a track copied into another project, or a
project opened from a branch or an archive, carries its ids unchanged. *Duplication* makes new
objects: a duplicated part, pattern or track inside a project gets fresh ids (a duplicated part
still points at the shared pattern). A transfer into a project that already holds an id is a
duplication of the clashing objects, and a reader reports it.

A reference to an id that is not in the document is *dangling*: a reader reports it and treats
the target as missing; it is never fatal.

### 5.2 Musical time and the tempo map

Musical time is an **integer count of ticks**, `ppq` per quarter note. Every position and length
in the document is in ticks: notes, parts, automation, markers, the tempo, meter and key points.
Seconds are never stored; they are derived through the tempo map. Nothing derivable is stored,
except the `files` index, which is declared an index. Ticks are within 0 to 2^40.

The tempo map is `tempo: [{at, bpm, curve}, …]`, **strictly increasing in `at`**, the first point
at tick 0. `bpm` is a number above 0 and at most 1000. `curve` says how the tempo runs from this
point to the next: `"hold"` (constant until the next point; the default) or `"linear"` (bpm a
linear function of ticks); after the last point the tempo holds, and before tick 0 the first
tempo holds. Fallback for an unknown `curve`: `linear`.

**The integration rule is normative.** For a segment from tick `a` at `A` bpm to tick `b` at `B`
bpm, with `k = (B − A) / (b − a)` and `t` a tick in `[a, b]`, the seconds from `a` to `t` are:

- hold, or `A = B`: `s(t) = 60 · (t − a) / (ppq · A)`
- linear: `s(t) = 60 / (ppq · k) · ln((A + k · (t − a)) / A)`, whose inverse is
  `t = a + A · (exp(s · ppq · k / 60) − 1) / k`

The seconds at a tick are the sum of the whole segments before it plus the partial one it falls
in; the samples at a tick are `round_half_up(seconds · rate)` for the rate in use.

Rounding: IEEE binary64 throughout, no fused multiply-add; segment durations are computed once
per segment and summed in tick order. Two implementations MUST agree bit for bit on hold
segments and within one sample at any rate on linear segments (`ln` and `exp` differ across
maths libraries by an ulp). The conformance fixtures publish test vectors; one is: `ppq` 960,
`tempo` `[{at: 0, bpm: 120, curve: linear}, {at: 3840, bpm: 60}]`, at 48 000 Hz; beats 1 to 5
(ticks 960 to 4800) fall at samples 25638, 55235, 90241, 133084 and 181084 (seconds 0.534126,
1.150728, 1.880015, 2.772589, 3.772589). A mean-tempo shortcut gives 2.6667 s for beat 4, 106 ms
off: the rule exists so that no two readers disagree by that.

### 5.3 The meter map

`meter: [{at, numerator, denominator}, …]`, strictly increasing in `at`, the first at tick 0;
`numerator` 1 to 99, `denominator` a power of two from 1 to 128. Each point's tick is the start of
a bar; bar and beat numbers are derived from the map and from `ppq` (a beat is `4 · ppq /
denominator` ticks). A reader that only plays never needs them.

### 5.4 The key map

`key: [{at, tonic, scale}, …]`, strictly increasing in `at`. `tonic` is 0 to 11 (C is 0);
`scale` is one of `major`, `minor`, `harmonic_minor`, `melodic_minor`, `dorian`, `phrygian`,
`lydian`, `mixolydian`, `locrian`, `major_pentatonic`, `minor_pentatonic`, `blues`, `chromatic`,
or an object `{"custom": [semitones above the tonic, ascending, 0 first]}`. Fallback for an
unknown scale name: `chromatic`. The key reaches no playback: it is what an editor's steps and
shading follow. A pattern may carry its own key (6.5).

### 5.5 Audio placement

A piece of audio (6.7) is placed by a **map of (frame, tick) points** and a **bpm**:

- `map: [{frame, tick}, …]`, at least one point, **strictly increasing in both** `frame` and
  `tick`. `frame` is a frame of the audio file (a sample of each channel; frame 0 is the file's
  first); `tick` is a tick of the pattern the piece's note is in (negative ticks are allowed: a
  crop may leave frame 0 before the pattern's start).
- `bpm` is a number above 0 and at most 1000, or `null`.

Between two consecutive points the map is linear; a point belongs to the segment it starts.
Before the first point and after the last, the slope is `q = ppq · bpm / (60 · rate)` ticks per
frame, `rate` being the file's sample rate: **the material plays at its own speed at `bpm`**. So
the tick of frame `f` is `tick₀ + (f − frame₀) · q` outside the points, and the frame at a tick is
the map's inverse.

With `q` the map's slope (ticks per frame) where a piece plays and `B(t)` the project's bpm there
(5.2), the piece advances `ppq · B(t) / (60 · q)` source frames per output second; outside the
points that is `B(t) · rate / bpm` frames per second, or `B(t) / bpm` source frames per output
frame when the rates are equal. The pitch is kept: a reader stretches, it does not resample.

**Worked example.** A two-bar loop recorded at 95 bpm, 222 821 frames at 44 100 Hz, placed with
`map: [{frame: 0, tick: 0}]` and `bpm: 95` in a project at `ppq` 960: `q = 960 · 95 / (60 ·
44100) = 0.034467` ticks per frame, so the loop's last frame falls at tick 7 680, eight beats. In
a project at 87 bpm it advances 87/95 of a source frame per output frame, so its 5.053 s of
material last 5.517 s: stretched by 95/87, pitch kept. A one-point map and a bpm is exactly
"anchor and tempo"; a map with more points is a warp (Live's warp markers, DAWproject's warps):
between the points the material follows them, and beyond them it runs at `bpm`.

`bpm: null` (allowed with exactly one point) means **unwarped**: the piece's start follows the
grid (its tick), and its content plays at 1×, `rate` source frames per output second, whatever
the tempo does.

## 6. The model

### 6.1 Tracks

Tracks form a tree. The document's `tracks` holds the top-level tracks in display order; a track
with `children` is a **folder** whose channel is the group bus its children feed. A track *is*
its channel: there are no separate channel objects or ids.

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `id` | id | yes | | |
| `name` | string | no | `""` | |
| `color` | string | no | none | `#rrggbb`, lower case. |
| `children` | array of track | no | `[]` | A folder's tracks, in display order. |
| `channel` | object | no | its defaults | The mixer strip (6.2). |
| `devices` | array of device | no | `[]` | The device chain in signal order (7). |
| `parameters` | array of object | no | `[]` | Promoted parameters (6.8). |
| `drum_map` | array of object or null | no | `null` | `[{pitch, name}]`: a lane per drum, in display order (6.11). |
| `patterns` | array of pattern | no | `[]` | The track's own patterns (6.5), sorted by id. |
| `parts` | array of part | no | `[]` | Where its patterns play (6.6), sorted by id. |
| `automation` | array of lane | no | `[]` | Automation lanes (6.8), sorted by id. |
| `renders` | array of render | no | `[]` | Bounces of this track's chain (7.7), sorted by file. |
| `slots` | array of slot | no | `[]` | Clip-launcher slots (6.10). |
| `extensions` | object | no | `{}` | (8.1) |

A track's patterns are its own: a part only plays a pattern of its own track, so a track object
is self-contained.

### 6.2 Channels, sends and routing

| Key | Type | Default | Meaning |
|---|---|---|---|
| `gain` | number ≥ 0 | `1.0` | Linear; `1.0` is 0 dB (dB = 20 · log10(gain)); `0` is silence. At most 1000. |
| `pan` | number −1 to 1 | `0.0` | Left to right. |
| `mute` | boolean | `false` | |
| `solo` | boolean | `false` | |
| `channels` | integer | `2` | `1` or `2`. |
| `destination` | id or null | `null` | Where the channel's output goes: `null` is the parent folder's channel, or `master` at the top; a track id; or `master`. |
| `sends` | array of send | `[]` | `{to (id, required), gain (default 1.0), pre_fader (default false), enabled (default true)}`, at most one per `to`, sorted by `to`. |

Routing MUST be a directed acyclic graph with `master` as its sink: a cycle through destinations
and sends is invalid (feedback is an extension's business). A *return* is a track without
patterns that receives sends. **Solo**: when any track is soloed, only the soloed tracks and the
tracks on their path to the master are heard. **Pan law**: a mono source pans with equal power
(−3 dB at the centre); a stereo source pans as a balance (the far side attenuated, the near side
unchanged).

### 6.3 The master

`master` is the master channel: `gain`, `pan`, `mute` (as 6.2, no `solo`, no `destination`),
`devices` (7) and `extensions`. Its id is the reserved `master`.

### 6.4 Patterns and parts: the primitive

A **pattern** holds events; a **part** is a window on a pattern placed on a track's timeline.
Several parts may play one pattern, so editing the pattern changes all of them; a *clip* in a
clip-based program is a part whose pattern no other part uses, so such a program writes one
pattern per clip and loses nothing, and loses sharing only when it writes a shared pattern back
as separate ones (which it SHOULD NOT; `conformance.md`).

### 6.5 Pattern

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `id` | id | yes | | |
| `name` | string | no | `""` | |
| `length` | integer ticks | yes | | The pattern's own length. |
| `key` | object or null | no | `null` | `{tonic, scale}` as 5.4: this pattern's own key, in place of the map's. |
| `events` | array of event | no | `[]` | Sorted by id. |
| `extensions` | object | no | `{}` | |

### 6.6 Part

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `id` | id | yes | | |
| `pattern` | id | yes | | A pattern of the same track. |
| `at` | integer ticks | yes | | Where the part starts on the track. |
| `length` | integer ticks | yes | | How long it plays, above 0. |
| `offset` | integer ticks | no | `0` | The pattern tick heard at the part's start. |
| `loop` | object or null | no | `null` | `{start, end}` in pattern ticks, `start < end`: the pattern plays from `offset`, and on reaching `end` continues from `start`, for the part's length. `null` plays once. |
| `mute` | boolean | no | `false` | |
| `name` | string or null | no | `null` | A name shown in place of the pattern's. |
| `extensions` | object | no | `{}` | |

Resizing a part changes how much of the pattern plays; it never changes the pattern.

### 6.7 Events

Every event carries `type`, `id` and `at` (pattern ticks, 0 or more). Fallback for an unknown
`type`: keep the object whole, play nothing.

**`note`**

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `length` | integer ticks | yes | | Above 0. |
| `pitch` | integer 0 to 127 | yes | | MIDI note number; 60 is middle C. |
| `velocity` | number 0 to 1 | yes | | |
| `release` | number 0 to 1 | no | `0.5` | Release velocity. |
| `expressions` | object | no | none | Per-note curves, each `[{at, value, curve}]` with `at` relative to the note's start: `pitch` (semitones, any sign), `pressure`, `timbre`, `volume`, `pan` (0 to 1, `pan` −1 to 1). `curve` as 6.8. |
| `audio` | object | no | none | An audio piece (below): the note sounds frames of a file instead of the track's instrument. |

A note without `audio` sounds from the track's instrument (7.1). A note with `audio` sounds
frames of a file: `at` and `length` are the **window** in the pattern during which it sounds, and
the file's frames are placed by the map (5.5); frames that fall outside the window are not
heard, but for a fade's handle (below). Its `pitch` is informational (the row it is drawn on), its
`velocity` applies as a gain of `velocity` (so `1.0` plays as recorded).

The audio piece:

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `file` | string | yes | | `audio/<name>` (2.2). |
| `sha256` | string | yes | | The file's SHA-256, 64 lower-case hex digits. |
| `sample_rate` | integer | yes | | The file's rate, 1 to 2^24. |
| `channels` | integer | yes | | 1 to 256. |
| `frames` | integer | yes | | The file's length in frames. |
| `name` | string | no | the file name | The name it was imported with. |
| `from` | integer | yes | | The first frame that sounds. |
| `to` | integer | yes | | One past the last frame that sounds; `from < to ≤ frames`. |
| `map` | array of point | yes | | `[{frame, tick}, …]` (5.5). |
| `bpm` | number or null | yes | | (5.5) |
| `gain` | number ≥ 0 | no | `1.0` | Linear. |
| `polarity` | boolean | no | `false` | `true` inverts the signal. |
| `channel_mode` | string | no | `as_recorded` | `as_recorded`, `swap`, `mono` (both summed to each side), `left`, `right`. Fallback: `as_recorded`. |
| `fade_in` | object or null | no | `null` | `{length (ticks), outside (ticks, default 0), shape}`: a fade whose first `outside` ticks lie before the piece's start, in the file beyond `from` (a handle), so two pieces butted at a cut crossfade. `shape`: `linear`, `equal_power`, `s_curve`; fallback `linear`. |
| `fade_out` | object or null | no | `null` | Likewise at the end. |
| `reverse` | boolean | no | `false` | Plays the file backwards: `from`, `to` and the map's frames then count the file reversed, frame `k` being the file's `frames − 1 − k`. |
| `extensions` | object | no | `{}` | |

The file's facts (`sample_rate`, `channels`, `frames`, `sha256`) are written inline so a reader
knows what the piece means without opening the file; a file that disagrees with them is a replaced
file, and the reader reports it.

**`chord`**

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `length` | integer ticks | yes | | |
| `root` | integer 0 to 11 | yes | | |
| `quality` | string | yes | | `maj`, `min`, `dim`, `aug`, `maj7`, `min7`, `7`, `min7b5`, `dim7`, `sus2`, `sus4`, `6`, `min6`, `9`, `maj9`, `min9`, `add9`, `11`, `13`, `5`. Fallback: play `notes`. |
| `bass` | integer 0 to 11 or null | no | `null` | A slash chord's bass. |
| `symbol` | string | no | none | As typed, `Cmaj7`. |
| `notes` | array of integer 0 to 127 | yes | | The voicing to play, at least one note. |
| `velocity` | number 0 to 1 | no | `0.8` | |

A chord carries its meaning *and* its rendered voicing, so a reader without a chord engine
sounds the right thing. This is the rule for every event that carries musical meaning: it also
carries what a reader plays if it understands nothing else.

**`param`**

| Key | Type | Required | Meaning |
|---|---|---|---|
| `param` | string | yes | A promoted parameter's name on the track (6.8). An unknown name is kept and ignored. |
| `value` | number | yes | The plain value, as `value` in the parameter's `params` entry (7.3); never normalized. |

A parameter event sets a parameter at a point in the pattern, with or without a note.

### 6.8 Parameters and automation

A track's `parameters` promotes device parameters under short names:
`[{name (string, required), device (id, required), param (string, required)}]`, `param` being
the device parameter's id as in 7.3. Names are unique within the track. Patterns' parameter events
and automation targets may use the name, so a plugin swap is one re-pointing.

An automation lane:

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `id` | id | yes | | |
| `target` | object | yes | | One of: `{"device": id, "param": string}`; `{"channel": "gain" \| "pan" \| "mute"}`; `{"channel": "send", "to": id}`; `{"parameter": name}`. |
| `points` | array of point | no | `[]` | `[{at, value, curve}]`, strictly increasing in `at` (absolute ticks on the track). |
| `extensions` | object | no | `{}` | |

`curve` on a point is how the value runs to the next point: `hold` or `linear` (the default);
fallback `linear`. A device parameter's value is the plain value, as `value` in its `params`
entry (7.3), never normalized: a plugin's mapping between the two is its own, and a CLAP host
has no normalized value to write; `gain` is linear, `pan` −1 to 1, `mute` is muted at 0.5 and
above, a send's value is its gain. A target that resolves to nothing is dangling (5.1).

### 6.9 Markers

`markers: [{id, at, name, kind, length}]`, sorted by id: `kind` is `marker` (a point; the
default) or `section` (a range: `length` in ticks, required then, so a program can name the
second verse). Fallback for an unknown kind: `marker`.

### 6.10 Scenes and slots

A clip launcher is optional. The document's `scenes` lists `[{id, name}]` in order; a track's
`slots` lists `[{scene (id), pattern (id of its own pattern), loop (as 6.6, default: the whole
pattern)}]`, at most one per scene. A reader without a launcher ignores them and keeps them.

### 6.11 Drum maps

A `drum_map` names a track's drum lanes: `[{pitch (0 to 127), name (string)}]` in display
order. It changes no note; a note's pitch picks its lane. An empty name means "as the instrument
names the key, else the General MIDI name".

## 7. Devices, states and dependencies

### 7.1 The chain

`devices` is one ordered list per track and on the master. Each device has a `role`:
`instrument`, `note_effect`, `audio_effect` or `analyzer` (fallback `audio_effect`, bypassed).
Notes flow through the note effects into the instrument, in list order; audio flows through the
audio effects in list order; an analyzer passes audio through unchanged. A track without an
instrument sounds its notes nowhere (but its audio pieces sound).

| Key | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `id` | id | yes | | |
| `role` | string | yes | | As above. |
| `name` | string | no | `""` | For display when the plugin is missing. |
| `plugin` | string | yes | | The plugin id in scheme form (7.2). |
| `aliases` | array of object | no | `[]` | `[{plugin, state_compatible (default false)}]`: the same plugin in other formats. |
| `state` | object or null | no | `null` | `{sha256, kind}`: the sidecar (7.4). |
| `params` | array of object | no | `[]` | The parameter snapshot (7.3), sorted by `id`. |
| `overrides` | array of object | no | `[]` | `[{param, value}]` applied after the state (7.5), sorted by `param`. |
| `enabled` | boolean | no | `true` | `false` bypasses the device. |
| `extensions` | object | no | `{}` | |

### 7.2 Plugin ids

One string, scheme first: `clap:<CLAP id>`; `vst3:<class id, 32 upper-case hex digits>`;
`au:<type>/<subtype>/<manufacturer>` (four-character codes); `lv2:<URI>`; `vst2:<decimal id>`;
`builtin:<vendor>:<name>` for a program's own device. An `aliases` entry names the same plugin
reachable through another scheme; `state_compatible: true` says its state bytes are the same (a
clap-wrapper build's VST3 chunk is its CLAP state).

### 7.3 The parameter snapshot

`params` is a readable snapshot of every parameter at save time, so that a stranger, or the same
plugin in another format, can set it up without the state:

| Key | Type | Required | Meaning |
|---|---|---|---|
| `id` | string | yes | The parameter's id in its plugin (a CLAP or VST3 id as decimal text, an LV2 symbol). |
| `name` | string | no | |
| `value` | number | yes | The plain value, authoritative. |
| `normalized` | number 0 to 1 | no | An independent fallback for a host that only takes normalized values; the mapping between the two is the plugin's, so neither is derived from the other. |
| `min`, `max` | number | no | The plain range. |
| `unit` | string | no | `linear`, `normalized`, `percent`, `decibel`, `hertz`, `semitones`, `seconds`, `beats`, `bpm`, `bool`, `enum`, `other` (the fallback). |
| `text` | string | no | The plugin's own reading, `"218 Hz"`. |

Writers that host the plugin SHOULD write every parameter.

### 7.4 States

A device's state is a sidecar file `plugins/states/<sha256>.<kind>`, named by the SHA-256 of its
content, so an unchanged state is never written twice and a state is found by its hash. `kind`
names the bytes' convention: `clap` (the raw stream `clap.state` saves), `vstpreset` (Steinberg's
container, which carries the class id and the component and controller chunks), `fxp`, `aupreset`
(a property list), `lv2` (a state bundle's Turtle, reserved). A state of kind `K` MUST be fed only
to a plugin reached through `K`'s own ABI, or through an alias marked `state_compatible`; anything
else is garbage, usually silently.

`plugins/files/<device id>/` is reserved for a plugin's own files (what CLAP's resource directory
collects); a later minor version defines it.

### 7.5 Authority, in order

1. A loadable `state` is the truth.
2. `params` is a snapshot of it, used only when the state can't be loaded: missing, or a kind the
   reached ABI can't take.
3. `overrides` apply after whichever was used: `[{param (the id as in 7.3), value (plain),
   normalized (optional)}]`. They are the only place a reader that could not load the plugin
   writes an edit.

### 7.6 A missing plugin

A reader that cannot load a device's plugin:

1. keeps the device object, its state file and its snapshot untouched, and the state file
   stays referred to;
2. bypasses an effect; for an instrument, plays the track from a current render if the track
   carries one (7.7), else silence;
3. tells the user by name, from the device's `dependencies` entry (vendor, version, where to
   get it);
4. writes any edit it makes to that device only into `overrides`.

### 7.7 Renders

A track MAY carry renders of its own chain's output, so that a project plays where its plugins
are missing: `renders: [{file, sha256, from, to, fingerprint}]`, `file` in `renders/`, `from`
and `to` in absolute ticks on the track, the render being the output of the track's devices
(before the fader and before any send, never including what other tracks feed it), at the file's
rate. A folder track's render and a return's render are out of 1.0: their inputs come from other
tracks.

`fingerprint` is the SHA-256 of the JCS bytes of one descriptor object:

```json
{"track": <the track object with "renders", "channel" and "children" removed>,
 "from": <from>, "to": <to>,
 "ppq": <ppq>, "tempo": <the tempo map>, "meter": <the meter map>, "key": <the key map>}
```

with defaults applied as in 4. A render whose fingerprint no longer matches the track is
*stale*: a reader may still play it but says so.

## 8. Extensions, unknown data and versions

### 8.1 Extensions

Every object with an `extensions` key may carry vendor data under it:
`"extensions": {"<namespace>": <any JSON value>}`, the namespace a reverse-DNS name matching
`^[a-z0-9]+(\.[a-z0-9-]+)+$` (`dk.mxm.newdawn`). The document's `extensions_used` lists every
namespace that appears anywhere in it; `extensions_required` lists those without which the
project does not sound right. **A required extension's data MUST leave a core fallback**: the
object it extends plays acceptably without it. The registry of known namespaces, and how an
extension is adopted into the core, are in `extensions.md`. There is no free-form `extras`: one
container, namespaced.

A reader that lacks a required extension opens the project, says by name what it cannot honour,
plays the fallbacks, and preserves everything.

### 8.2 Preservation

**Preservation is structural, not byte for byte.** A writer re-emits every `extensions` value and
every key it does not know, as the same JSON value, on the same object (found by id, or by its
place under its parent when it has no id), whatever the formatting. An object of an unknown
`type` or `kind` is kept whole. Data belonging to an object the user deleted goes with it. A
reader never refuses a project for data it does not understand.

### 8.3 Versions

`version` is `major.minor`. A **minor** version only adds: a new optional key with a stated
default, a new value of an enumeration with its stated fallback, an extension adopted into the
core. A **major** version may change a meaning, a type, a default or add a required key; it is a
new format with a converter. A key is never removed within a major version, only deprecated.

A reader opens any minor of its major, keeping what it does not know. It MUST refuse a higher
major by name, with the document's `spec` URL. A higher `min_version` within its major means the
document plays correctly only from that minor on: the reader opens it **read-only** (play, render
and export allowed; nothing written back) and says why. `extensions_required` has the same
effect on a reader that lacks one of them, but for the right to write: it may write, preserving.

## 9. Robustness

What a reader MUST check, and what is invalid:

- File references match 2.1 and resolve inside the folder without following links; a link in
  place of a file or folder the reader reads is refused; `private` paths and the reserved folders
  are never traversed.
- `bytes` in the index is verified against the file, never used to allocate; audio is never read
  whole on trust of the document.
- Ids are unique across the document; every reference resolves to an object of the expected
  kind (a dangling one is reported, 5.1); the track tree is a tree; routing is acyclic (6.2).
- JSON is I-JSON (3.1): duplicate keys, `NaN`, `Infinity`, an integer beyond ±2^53, a number
  outside its bounds, a tick beyond 2^40, a negative gain, a map or point list not strictly
  increasing, are each invalid, named by their JSON pointer.
- A hash that does not match plays silence for that file and warns; it never deletes.

The conformance fixtures include hostile projects for each of these (`conformance.md`).

## 10. Reading and writing, in short

A **reader**: opens the one `.tutti` file; checks 3.1 and 9; applies defaults (4); resolves ids;
keeps everything it does not know; plays what it can, telling the user what it does not honour
(a missing plugin, a required extension, a feature above its level); never deletes.

A **writer**: emits the pretty form (3.1) with defaults left out (4); keeps every id it read and
gives its new objects ids of its own form (5.1); writes every file it refers to inside the folder
before the document that refers to it, and the document last, to a temporary name renamed into
place; fills `files`, `dependencies`, `extensions_used` and `generator`; writes `README.md`;
re-emits what it did not understand (8.2).
