/* Generated from schema/1.0/tutti.schema.json by npm run types; do not edit. */

/**
 * A reverse-DNS extension namespace (spec 8.1).
 */
export type Namespace = string;
/**
 * A bare file name in a fixed folder (spec 2.2).
 */
export type Reference = string;
export type Sha256 = string;
/**
 * A position or length in ticks (spec 5.2).
 */
export type Ticks = number;
export type Bpm = number;
/**
 * 0 is C.
 */
export type PitchClass = number;
export type Scale =
  | string
  | {
      /**
       * Semitones above the tonic, ascending, 0 first.
       *
       * @minItems 1
       */
      custom: [PitchClass, ...PitchClass[]];
      [k: string]: unknown;
    };
/**
 * An opaque id, unique across the document (spec 5.1).
 */
export type Id = string;
export type UnitInterval = number;
/**
 * A MIDI note number; 60 is middle C.
 */
export type Pitch = number;
/**
 * A note, a chord or a param (spec 6.7); an unknown type is kept whole and plays nothing.
 */
export type Event = {
  type: string;
  id: Id;
  at: Ticks;
  [k: string]: unknown;
};

/**
 * An open audio project: one JSON document in a plain folder. The normative text is spec/1.0/tutti.md; this schema holds its types, bounds and forms. Every object is open: a key the schema does not name is kept and written back by every reader (spec 8.2). What the schema cannot say (ids unique, references resolving, routing acyclic) is validate's.
 */
export interface Tutti10 {
  /**
   * Always "tutti".
   */
  format: "tutti";
  /**
   * The spec version the document follows, major.minor.
   */
  version: string;
  /**
   * The lowest minor of this major that plays the document correctly (spec 8.3). Default: the major's .0.
   */
  min_version?: string;
  /**
   * One sentence for a stranger (spec 3.3).
   */
  about?: string;
  /**
   * Where this version's normative text is.
   */
  spec?: string;
  /**
   * Where this version's JSON Schema is.
   */
  schema?: string;
  generator: Generator;
  /**
   * Every extension namespace that appears anywhere in the document (spec 8.1).
   */
  extensions_used?: Namespace[];
  /**
   * The subset without which the project does not sound right (spec 8.1).
   */
  extensions_required?: Namespace[];
  /**
   * Paths at the top of the folder that are the writer's own (spec 2.3).
   */
  private?: string[];
  /**
   * The index of files the project refers to, sorted by path (spec 3.4).
   */
  files?: FileEntry[];
  /**
   * What the project needs beyond its folder, sorted by id (spec 3.5).
   */
  dependencies?: Dependency[];
  metadata?: Metadata;
  /**
   * Ticks per quarter note; 960 recommended (spec 5.2).
   */
  ppq: number;
  /**
   * The tempo map, strictly increasing in at, the first point at 0 (spec 5.2).
   *
   * @minItems 1
   */
  tempo: [TempoPoint, ...TempoPoint[]];
  /**
   * The meter map, strictly increasing in at, the first point at 0 (spec 5.3).
   *
   * @minItems 1
   */
  meter: [MeterPoint, ...MeterPoint[]];
  /**
   * The key map (spec 5.4).
   */
  key?: KeyPoint[];
  /**
   * Named points and sections, sorted by id (spec 6.9).
   */
  markers?: Marker[];
  /**
   * The loop range (spec 3.6).
   */
  loop?: null | Loop;
  master?: Master;
  /**
   * The top-level tracks in display order (spec 6.1).
   */
  tracks?: Track[];
  /**
   * The clip launcher's scenes, in order (spec 6.10).
   */
  scenes?: Scene[];
  extensions?: Extensions;
  [k: string]: unknown;
}
export interface Generator {
  /**
   * The last program that wrote the document.
   */
  name: string;
  version?: string;
  url?: string;
  [k: string]: unknown;
}
export interface FileEntry {
  path: Reference;
  sha256: Sha256;
  bytes: number;
  [k: string]: unknown;
}
export interface Dependency {
  /**
   * The plugin id in scheme form (spec 7.2).
   */
  id: string;
  /**
   * plugin; content is reserved.
   */
  kind: string;
  name: string;
  vendor?: string;
  /**
   * The plugin version the project was saved with.
   */
  version?: string;
  /**
   * Where to get it, as the plugin declares it.
   */
  url?: string;
  /**
   * A git URL, so a future program can rebuild the plugin.
   */
  repository?: string;
  commit?: string;
  /**
   * An SPDX identifier.
   */
  license?: string;
  /**
   * clap, vst3, au, lv2, vst2.
   */
  formats?: string[];
  /**
   * windows, macos, linux, web.
   */
  platforms?: string[];
  [k: string]: unknown;
}
export interface Metadata {
  title?: string;
  artist?: string;
  album?: string;
  year?: string;
  genre?: string;
  comment?: string;
  copyright?: string;
  /**
   * An ISO 8601 date, set once.
   */
  created?: string;
  [k: string]: unknown;
}
export interface TempoPoint {
  at: Ticks;
  bpm: Bpm;
  /**
   * How the tempo runs to the next point: hold (the default) or linear in ticks.
   */
  curve?: string;
  [k: string]: unknown;
}
export interface MeterPoint {
  at: Ticks;
  numerator: number;
  denominator: 1 | 2 | 4 | 8 | 16 | 32 | 64 | 128;
  [k: string]: unknown;
}
export interface KeyPoint {
  at: Ticks;
  tonic: PitchClass;
  scale: Scale;
  [k: string]: unknown;
}
export interface Marker {
  id: Id;
  at: Ticks;
  name?: string;
  /**
   * marker (a point) or section (a range with a length); an unknown kind reads as marker.
   */
  kind?: string;
  /**
   * A position or length in ticks (spec 5.2).
   */
  length?: number;
  extensions?: Extensions;
  [k: string]: unknown;
}
/**
 * Vendor data by reverse-DNS namespace (spec 8.1).
 */
export interface Extensions {
  [k: string]: unknown;
}
export interface Loop {
  start: Ticks;
  end: Ticks;
  enabled?: boolean;
  [k: string]: unknown;
}
/**
 * The master channel (spec 6.3); its id is the reserved master.
 */
export interface Master {
  /**
   * Linear; 1 is 0 dB (spec 6.2).
   */
  gain?: number;
  pan?: number;
  mute?: boolean;
  devices?: Device[];
  extensions?: Extensions;
  [k: string]: unknown;
}
/**
 * A device in a chain (spec 7.1).
 */
export interface Device {
  id: Id;
  /**
   * instrument, note_effect, audio_effect or analyzer; an unknown role reads as audio_effect, bypassed.
   */
  role: string;
  name?: string;
  /**
   * The plugin id in scheme form: clap:, vst3:, au:, lv2:, vst2:, builtin: (spec 7.2).
   */
  plugin: string;
  aliases?: Alias[];
  state?: null | StateRef;
  /**
   * The parameter snapshot, sorted by id (spec 7.3).
   */
  params?: Param[];
  /**
   * Values applied after the state, sorted by param (spec 7.5).
   */
  overrides?: Override[];
  enabled?: boolean;
  extensions?: Extensions;
  [k: string]: unknown;
}
export interface Alias {
  plugin: string;
  /**
   * Whether the state bytes are the same through this alias.
   */
  state_compatible?: boolean;
  [k: string]: unknown;
}
/**
 * The sidecar plugins/states/<sha256>.<kind> (spec 7.4).
 */
export interface StateRef {
  sha256: Sha256;
  /**
   * clap, vstpreset, fxp, aupreset or lv2.
   */
  kind: string;
  [k: string]: unknown;
}
export interface Param {
  id: string;
  name?: string;
  /**
   * The plain value, authoritative.
   */
  value: number;
  /**
   * An independent fallback for a host that only takes normalized values.
   */
  normalized?: number;
  min?: number;
  max?: number;
  /**
   * linear, normalized, percent, decibel, hertz, semitones, seconds, beats, bpm, bool, enum, other.
   */
  unit?: string;
  text?: string;
  [k: string]: unknown;
}
export interface Override {
  param: string;
  value: number;
  normalized?: UnitInterval;
  [k: string]: unknown;
}
/**
 * A track and its channel (spec 6.1); a track with children is a folder.
 */
export interface Track {
  id: Id;
  name?: string;
  color?: string;
  children?: Track[];
  channel?: Channel;
  /**
   * The chain in signal order (spec 7.1).
   */
  devices?: Device[];
  /**
   * Promoted parameters (spec 6.8).
   */
  parameters?: Promoted[];
  /**
   * A lane per drum in display order (spec 6.11).
   */
  drum_map?: null | Lane[];
  /**
   * The track's own patterns, sorted by id.
   */
  patterns?: Pattern[];
  /**
   * Where its patterns play, sorted by id.
   */
  parts?: Part[];
  /**
   * Automation lanes, sorted by id (spec 6.8).
   */
  automation?: AutomationLane[];
  /**
   * Bounces of this track's chain, sorted by file (spec 7.7).
   */
  renders?: Render[];
  /**
   * Clip-launcher slots (spec 6.10).
   */
  slots?: Slot[];
  extensions?: Extensions;
  [k: string]: unknown;
}
/**
 * The mixer strip (spec 6.2).
 */
export interface Channel {
  /**
   * Linear; 1 is 0 dB (spec 6.2).
   */
  gain?: number;
  pan?: number;
  mute?: boolean;
  solo?: boolean;
  channels?: 1 | 2;
  /**
   * null: the parent folder's channel, or master at the top; a track id; or master.
   */
  destination?: null | Id;
  /**
   * At most one per destination, sorted by to.
   */
  sends?: Send[];
  [k: string]: unknown;
}
export interface Send {
  to: Id;
  /**
   * Linear; 1 is 0 dB (spec 6.2).
   */
  gain?: number;
  pre_fader?: boolean;
  enabled?: boolean;
  [k: string]: unknown;
}
export interface Promoted {
  /**
   * The short name patterns and automation use.
   */
  name: string;
  device: Id;
  /**
   * The device parameter's id (spec 7.3).
   */
  param: string;
  [k: string]: unknown;
}
export interface Lane {
  pitch: Pitch;
  name?: string;
  [k: string]: unknown;
}
export interface Pattern {
  id: Id;
  name?: string;
  length: number;
  key?: null | PatternKey;
  /**
   * Sorted by id.
   */
  events?: Event[];
  extensions?: Extensions;
  [k: string]: unknown;
}
export interface PatternKey {
  tonic: PitchClass;
  scale: Scale;
  [k: string]: unknown;
}
export interface Part {
  id: Id;
  /**
   * An opaque id, unique across the document (spec 5.1).
   */
  pattern: string;
  at: Ticks;
  length: number;
  /**
   * A position or length in ticks (spec 5.2).
   */
  offset?: number;
  loop?: null | LoopRange;
  mute?: boolean;
  name?: null | string;
  extensions?: Extensions;
  [k: string]: unknown;
}
export interface LoopRange {
  start: Ticks;
  end: Ticks;
  [k: string]: unknown;
}
export interface AutomationLane {
  id: Id;
  target: Target;
  /**
   * Strictly increasing in at, absolute ticks on the track.
   */
  points?: Point[];
  extensions?: Extensions;
  [k: string]: unknown;
}
/**
 * Exactly one of: device and param; channel (gain, pan, mute, or send with to); parameter (spec 6.8).
 */
export interface Target {
  device?: Id;
  param?: string;
  channel?: string;
  to?: Id;
  parameter?: string;
  [k: string]: unknown;
}
export interface Point {
  at: Ticks;
  value: number;
  /**
   * hold or linear; an unknown name reads as linear (spec 5.2, 6.8).
   */
  curve?: string;
  [k: string]: unknown;
}
export interface Render {
  file: Reference;
  sha256: Sha256;
  from: Ticks;
  to: Ticks;
  /**
   * The SHA-256 of the JCS bytes of the descriptor the spec defines (7.7).
   */
  fingerprint: string;
  [k: string]: unknown;
}
export interface Slot {
  scene: Id;
  /**
   * An opaque id, unique across the document (spec 5.1).
   */
  pattern: string;
  loop?: null | LoopRange;
  [k: string]: unknown;
}
export interface Scene {
  id: Id;
  name?: string;
  extensions?: Extensions;
  [k: string]: unknown;
}
