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

export interface Feature {
  name: string;
  level: 0 | 1 | 2;
}

export interface FolderCheck {
  path: string;
  text: string;
  issues: Issue[];
  features: Feature[];
  level: 0 | 1 | 2;
}

/** The format's version this build writes. */
export function version(): string;
/** The `about` sentence a writer puts in the manifest. */
export function about(): string;
/** A document in any form, returned in the pretty form; throws with the reader's reason. */
export function fmt(text: string): string;
/** The JCS bytes equality and fingerprints are over. */
export function jcs(text: string): string;
/** Whether two documents are equal as the spec means it. */
export function equal(a: string, b: string): boolean;
/** Whether a document opens read-only on this version (a later `min_version`). */
export function isReadOnly(text: string): boolean;
/** Validation: errors make the document invalid; warnings are reported and it opens. */
export function validate(text: string): Issue[];
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
/** Node only: reads a project folder, validates its document and checks its files. */
export function checkFolder(folder: string): Promise<FolderCheck>;
