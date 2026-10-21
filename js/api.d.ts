// The package's API. The document types are in schema.d.ts, generated from the schema.

import type { Tutti10 } from './schema';

/** A document as the schema describes it. */
export type Document = Tutti10;
export type TempoPoint = Document['tempo'][number];
export type MapPoint = { frame: number; tick: number };
export type Generator = Document['generator'];

export interface Issue {
  severity: 'error' | 'warning';
  pointer: string;
  message: string;
}

/** A key this version doesn't know, once for every object it was found in. */
export interface UnknownKey {
  key: string;
  /** The JSON pointer of the first place it is in. */
  pointer: string;
  /** How many objects carry it. */
  count: number;
}

export interface Feature {
  name: string;
  level: 0 | 1 | 2;
}

/** What `save` did. */
export interface Saved {
  warnings: Issue[];
  readme_written: boolean;
  document_hash: string;
}

/** A file put in the project. */
export interface Stored {
  reference: string;
  sha256: string;
  bytes: number;
}

/** A plugin state put in the project (spec 7.4). */
export interface StoredState {
  sha256: string;
  kind: string;
  path: string;
}

/**
 * A file as last verified: its hash, and the length and time it had then (nanoseconds since
 * the Unix epoch as a decimal string: they overflow a number).
 */
export interface Verified {
  sha256: string;
  bytes: number;
  modified?: string;
}

/** One entry of a folder, as a store lists it. */
export interface Entry {
  name: string;
  kind: 'file' | 'folder' | 'other';
}

/**
 * A store the host supplies: synchronous methods over its files, references relative to the
 * project's folder with `/` between the parts. A method throws an `Error` whose `code` says
 * what happened: `ENOENT` (missing), `EEXIST` (there already), `LINK` (a link where a file or
 * folder was expected); any other error is the store's own reason.
 */
export interface Store {
  /** The entries directly in `folder` (`''` the root), as JSON text of `Entry[]`. */
  list(folder: string): string;
  read(reference: string): Uint8Array;
  size(reference: string): number;
  /** Nanoseconds since the Unix epoch as a `bigint`, milliseconds as a number, or `null`. */
  modified(reference: string): bigint | number | null;
  exists(reference: string): boolean;
  makeFolder(folder: string): void;
  /** Writes a new file whole and durably; throws `EEXIST` when a file is there already. */
  writeNew(reference: string, bytes: Uint8Array): void;
  renameOver(from: string, to: string): void;
  syncFolder(folder: string): void;
  remove(reference: string): void;
}

/** A store in memory, for a host without a file system or a test. */
export interface MemoryStore extends Store {
  files(): Map<string, Uint8Array>;
  put(reference: string, bytes: Uint8Array): void;
  putUnseen(reference: string, bytes: Uint8Array): void;
}

/** A new file being written into a project. */
export declare class Incoming {
  readonly reference: string;
  write(bytes: Uint8Array): void;
  finish(): Stored;
}

/** A project in a store (spec 2, 3.4, 10): the format's own save procedure, run by the core. */
export declare class Project {
  static open(store: Store): Project;
  static create(store: Store, title: string, generator: Generator): Project;
  static adopt(store: Store, documentName: string, document: Document | string): Project;
  readonly report: Issue[];
  readonly readOnly: boolean;
  /** The document in the pretty form. */
  readonly text: string;
  /** The document as an object; set it back to change it. */
  document: Document;
  readonly documentName: string;
  readonly documentHash: string;
  readonly title: string;
  readonly unnamed: number;
  save(generator: Generator): Saved;
  saveVerified(generator: Generator): Saved;
  rewrite(): string;
  check(): Issue[];
  read(reference: string): Uint8Array;
  putAudio(name: string, extension: string, bytes: Uint8Array): Stored;
  putRender(name: string, extension: string, bytes: Uint8Array): Stored;
  putState(kind: string, bytes: Uint8Array): StoredState;
  findAudio(bytes: Uint8Array): Stored | undefined;
  newAudio(name: string, extension: string): Incoming;
  newRender(name: string, extension: string): Incoming;
  forgetFile(path: string): void;
  verified(): Record<string, Verified>;
  trust(records: Record<string, Verified> | string): void;
  copyInto(store: Store): Project;
}

/** The format's version this build writes. */
export function version(): string;
/** The `about` sentence a writer puts in the manifest. */
export function about(): string;
/** A document in any form, returned in the pretty form; throws with the reader's reason. */
export function fmt(text: string): string;
/** The document's JCS bytes; `equal` compares them with `generator` left out. */
export function jcs(text: string): string;
/** Whether two documents are equal as the spec means it: `generator` left out. */
export function equal(a: string, b: string): boolean;
/** Whether a document opens read-only on this version (a later `min_version`). */
export function isReadOnly(text: string): boolean;
/** Validation: errors make the document invalid; warnings are reported and it opens. */
export function validate(text: string): Issue[];
/** The keys the document carries that this version doesn't know, once each; kept, never an error. */
export function unknownKeys(text: string): UnknownKey[];
/** The features a document uses, each with the level that first covers it. */
export function features(text: string): Feature[];
/** The lowest conformance level a reader needs to play the document in full. */
export function levelNeeded(text: string): 0 | 1 | 2;
/** Every file the document refers to, as references (`audio/x.wav`). */
export function referencedFiles(text: string): string[];
/** The files check with what the host found: reference to `{sha256, bytes}`. */
export function checkFiles(text: string, files: Record<string, { sha256: string; bytes: number }>): Issue[];
/** Seconds from tick 0 to a tick through a tempo map (spec 5.2). */
export function secondsAt(ppq: number, tempo: TempoPoint[], tick: number): number;
/** The tick at a number of seconds from tick 0. */
export function tickAt(ppq: number, tempo: TempoPoint[], seconds: number): number;
/** The sample a tick falls on at a rate (`round_half_up(seconds · rate)`). */
export function samplesAt(ppq: number, tempo: TempoPoint[], tick: number, rate: number): number;
/** The tempo in force at a tick. */
export function bpmAt(ppq: number, tempo: TempoPoint[], tick: number): number;
/** The pattern tick a source frame sits at (spec 5.5), or `undefined` for an unwarped piece. */
export function tickOfFrame(ppq: number, rate: number, map: MapPoint[], bpm: number | null, frame: number): number | undefined;
/** The source frame at a pattern tick, or `undefined` for an unwarped piece. */
export function frameAtTick(ppq: number, rate: number, map: MapPoint[], bpm: number | null, tick: number): number | undefined;
/** Source frames per output second at a tick, given the project's bpm there. */
export function rateAt(ppq: number, rate: number, map: MapPoint[], bpm: number | null, tick: number, projectBpm: number): number;
/** A render's fingerprint for a track of the document (spec 7.7). */
export function renderFingerprint(text: string, trackId: string, from: number, to: number): string;
/** SHA-256 of bytes, 64 lower-case hexadecimal digits. */
export function sha256Hex(bytes: Uint8Array): string;
/** An id in the recommended form: a kind prefix (`trk`, `pat`, `prt`, `evt`, `dev`, `aut`, `mrk`, `scn`) and a ULID. */
export function ulid(prefix: string, unixMillis?: number, random?: Uint8Array): string;
/** Whether a text is an id of the spec's form. */
export function isId(text: string): boolean;
/** The README for a project folder (spec 2.4): the template filled in. */
export function readme(title: string, generator: Generator, documentName: string): string;
/** The smallest valid document, in the pretty form. */
export function newDocument(generator: Generator): string;
/** A store in memory: a project built without a file system, written out afterwards. */
export function memoryStore(): MemoryStore;
/** An error a store throws, with the `code` the core reads: `ENOENT`, `EEXIST` or `LINK`. */
export function storeError(code: string, message: string): Error & { code: string };
/** Packs the project in a store into one file, `<Name>.tutti.zip` (spec 2.5): its bytes, the document first and stored. */
export function packZip(store: Store): Uint8Array;
/** Unpacks one file's bytes into a store that holds no project, refusing what a reader must refuse (spec 2.5): the number of files written. */
export function unpackZip(bytes: Uint8Array, store: Store): number;
/** Node only: a store over a folder on disk, through synchronous file calls. */
export function nodeStore(root: string): Store;
/** Node only: the project in a folder on disk. */
export function openFolder(folder: string): Project;
/** Node only: a new project in a folder on disk that holds no document. */
export function createFolder(folder: string, title: string, generator: Generator): Project;
