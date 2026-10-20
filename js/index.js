// @mxm-audio/tutti: the Rust core compiled to WebAssembly, wrapped so JavaScript sees plain
// objects. Documents go in and out as strings (the file's text); points, maps and generators as
// the objects the schema describes. File IO is the host's: `checkFolder` is a Node helper that
// reads a project folder and hands the core what it needs.

'use strict';

const wasm = require('./pkg/tutti.js');

const json = (fn) => (...args) => JSON.parse(fn(...args));
const orUndefined = (x) => (x === null ? undefined : x);

/** The format's version this build writes. */
const version = () => wasm.version();

/** The `about` sentence a writer puts in the manifest. */
const about = () => wasm.about();

/** A document in any form, returned in the pretty form; throws with the reader's reason. */
const fmt = (text) => wasm.fmt(text);

/** The JCS bytes equality and fingerprints are over. */
const jcs = (text) => wasm.jcs(text);

/** Whether two documents are equal as the spec means it. */
const equal = (a, b) => wasm.equal(a, b);

/** Whether a document opens read-only on this version (a later `min_version`). */
const isReadOnly = (text) => wasm.is_read_only(text);

/** Validation: an array of `{severity, pointer, message}`, errors and warnings. */
const validate = json(wasm.validate);

/** The features a document uses: an array of `{name, level}`. */
const features = json(wasm.features);

/** The lowest conformance level a reader needs to play the document in full. */
const levelNeeded = (text) => wasm.level_needed(text);

/** Every file the document refers to, as references (`audio/x.wav`). */
const referencedFiles = json(wasm.referenced_files);

/**
 * The files check with what the host found: `files` maps each reference in `audio/`,
 * `plugins/states/` and `renders/` to `{sha256, bytes}`.
 */
const checkFiles = (text, files) => JSON.parse(wasm.check_files_with(text, JSON.stringify(files)));

/** Seconds from tick 0 to a tick through a tempo map (the document's `tempo` array). */
const secondsAt = (ppq, tempo, tick) => wasm.seconds_at(ppq, JSON.stringify(tempo), tick);

/** The tick at a number of seconds from tick 0. */
const tickAt = (ppq, tempo, seconds) => wasm.tick_at(ppq, JSON.stringify(tempo), seconds);

/** The sample a tick falls on at a rate. */
const samplesAt = (ppq, tempo, tick, rate) => wasm.samples_at(ppq, JSON.stringify(tempo), tick, rate);

/** The tempo in force at a tick. */
const bpmAt = (ppq, tempo, tick) => wasm.bpm_at(ppq, JSON.stringify(tempo), tick);

/** The pattern tick a source frame sits at, or `undefined` for an unwarped piece. */
const tickOfFrame = (ppq, rate, map, bpm, frame) =>
  wasm.tick_of_frame(ppq, rate, JSON.stringify(map), orUndefined(bpm), frame);

/** The source frame at a pattern tick, or `undefined` for an unwarped piece. */
const frameAtTick = (ppq, rate, map, bpm, tick) =>
  wasm.frame_at_tick(ppq, rate, JSON.stringify(map), orUndefined(bpm), tick);

/** Source frames per output second at a tick, given the project's bpm there. */
const rateAt = (ppq, rate, map, bpm, tick, projectBpm) =>
  wasm.rate_at(ppq, rate, JSON.stringify(map), orUndefined(bpm), tick, projectBpm);

/** A render's fingerprint for a track of the document. */
const renderFingerprint = (text, trackId, from, to) => wasm.render_fingerprint(text, trackId, from, to);

/** SHA-256 of bytes, 64 lower-case hexadecimal digits. */
const sha256Hex = (bytes) => wasm.sha256_hex(bytes);

/** An id in the recommended form: a kind prefix and a ULID (now, and ten random bytes). */
const ulid = (prefix, unixMillis = Date.now(), random = randomBytes()) => wasm.ulid(prefix, unixMillis, random);

function randomBytes() {
  const bytes = new Uint8Array(10);
  globalThis.crypto.getRandomValues(bytes);
  return bytes;
}

/** Whether a text is an id of the spec's form. */
const isId = (text) => wasm.is_id(text);

/** The README for a project folder: the template filled in. */
const readme = (title, generator, documentName) => wasm.readme(title, JSON.stringify(generator), documentName);

/** The smallest valid document, in the pretty form. */
const newDocument = (generator) => wasm.new_document(JSON.stringify(generator));

/**
 * Node only: reads a project folder, validates its document and checks its files. Returns
 * `{path, text, issues, features, level}`; throws when the folder has no document, or two.
 */
async function checkFolder(folder) {
  const fs = require('node:fs/promises');
  const path = require('node:path');
  const crypto = require('node:crypto');
  const names = (await fs.readdir(folder)).filter((n) => n.endsWith('.tutti')).sort();
  if (names.length !== 1) {
    throw new Error(`${folder}: ${names.length === 0 ? 'no .tutti document' : `${names.length} documents (${names.join(', ')})`}`);
  }
  const documentPath = path.join(folder, names[0]);
  const text = await fs.readFile(documentPath, 'utf8');
  const issues = validate(text);
  const files = {};
  for (const sub of ['audio', 'plugins/states', 'renders']) {
    let entries = [];
    try {
      entries = await fs.readdir(path.join(folder, sub), { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      if (entry.isSymbolicLink()) {
        issues.push({ severity: 'error', pointer: '/files', message: `${sub}/${entry.name}: a link, which a reader refuses` });
        continue;
      }
      if (!entry.isFile()) continue;
      const bytes = await fs.readFile(path.join(folder, sub, entry.name));
      files[`${sub}/${entry.name}`] = { sha256: crypto.createHash('sha256').update(bytes).digest('hex'), bytes: bytes.length };
    }
  }
  issues.push(...checkFiles(text, files));
  return { path: documentPath, text, issues, features: features(text), level: levelNeeded(text) };
}

module.exports = {
  version,
  about,
  fmt,
  jcs,
  equal,
  isReadOnly,
  validate,
  features,
  levelNeeded,
  referencedFiles,
  checkFiles,
  secondsAt,
  tickAt,
  samplesAt,
  bpmAt,
  tickOfFrame,
  frameAtTick,
  rateAt,
  renderFingerprint,
  sha256Hex,
  ulid,
  isId,
  readme,
  newDocument,
  checkFolder,
};
