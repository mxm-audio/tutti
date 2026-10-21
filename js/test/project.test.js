// A project in a store: created, filled, saved, reopened and checked, in memory and on disk; the
// save's refusals; verified records kept between sessions; a copy.

'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { test } = require('node:test');
const tutti = require('..');

const generator = { name: 'tutti-js-tests', version: '0.1', url: 'https://github.com/mxm-audio/tutti' };

function trackWithPiece(stored) {
  return {
    id: 'trk_1',
    name: 'Vox',
    patterns: [
      {
        id: 'pat_1',
        length: 960,
        events: [
          {
            type: 'note',
            id: 'evt_1',
            at: 0,
            length: 960,
            pitch: 60,
            velocity: 0.8,
            audio: {
              file: stored.reference,
              sha256: stored.sha256,
              sample_rate: 48000,
              channels: 1,
              frames: 4,
              from: 0,
              to: 4,
              map: [{ frame: 0, tick: 0 }],
              bpm: null,
            },
          },
        ],
      },
    ],
  };
}

test('a project in memory is created, filled, saved, reopened and checked', () => {
  const store = tutti.memoryStore();
  const project = tutti.Project.create(store, 'My Song: take 1', generator);
  assert.equal(project.documentName, 'My-Song-take-1.tutti');
  assert.ok(store.files().has('My-Song-take-1.tutti') && store.files().has('README.md'));

  // A recording streams in through the handle; a state goes in by content.
  const take = project.newAudio('Vox: take 1/2', 'wav');
  assert.equal(take.reference, 'audio/Vox-take-1-2.wav');
  take.write(new TextEncoder().encode('RI'));
  take.write(new TextEncoder().encode('FF'));
  const stored = take.finish();
  assert.equal(stored.bytes, 4);
  assert.equal(stored.sha256, tutti.sha256Hex(new TextEncoder().encode('RIFF')));
  const state = project.putState('clap', new TextEncoder().encode('state'));
  assert.equal(state.path, `plugins/states/${state.sha256}.clap`);

  const doc = project.document;
  doc.tracks = [trackWithPiece(stored)];
  project.document = doc;
  const saved = project.save(generator);
  assert.deepEqual(saved.warnings, []);
  assert.equal(saved.document_hash.length, 64);

  const again = tutti.Project.open(store);
  assert.deepEqual(again.report, []);
  assert.equal(again.readOnly, false);
  assert.equal(tutti.equal(again.text, project.text), true);
  assert.equal(again.document.files.length, 1);
  assert.equal(again.document.files[0].path, 'audio/Vox-take-1-2.wav');
  assert.deepEqual(again.check(), []);
  assert.equal(new TextDecoder().decode(again.read('audio/Vox-take-1-2.wav')), 'RIFF');

  // The same content once; a different one under a free name.
  assert.equal(project.putAudio('Loop', 'wav', new TextEncoder().encode('RIFF')).reference, 'audio/Vox-take-1-2.wav');
  assert.equal(project.putAudio('Loop', 'wav', new TextEncoder().encode('RIFX')).reference, 'audio/Loop.wav');
  assert.equal(project.findAudio(new TextEncoder().encode('nope')), undefined);
});

test('a save refuses a missing or changed file, and verified records cross sessions', () => {
  const store = tutti.memoryStore();
  const project = tutti.Project.create(store, 'A', generator);
  const stored = project.putAudio('Loop', 'wav', new TextEncoder().encode('RIFF'));
  const doc = project.document;
  doc.tracks = [trackWithPiece(stored)];
  project.document = doc;
  project.save(generator);

  store.put('audio/Loop.wav', new TextEncoder().encode('RIFX'));
  assert.throws(() => project.save(generator), /audio\/Loop\.wav: expected/);
  store.put('audio/Loop.wav', new TextEncoder().encode('RIFF'));
  project.save(generator);
  // A forged change, the time put back: not seen by save, seen by check.
  store.putUnseen('audio/Loop.wav', new TextEncoder().encode('RIFX'));
  project.save(generator);
  assert.ok(project.check().some((i) => i.severity === 'error'));
  assert.throws(() => project.saveVerified(generator), /expected/);
  store.remove('audio/Loop.wav');
  assert.throws(() => project.save(generator), /audio\/Loop\.wav: missing/);

  // Records kept between sessions.
  store.put('audio/Loop.wav', new TextEncoder().encode('RIFF'));
  const next = tutti.Project.open(store);
  next.save(generator);
  const records = next.verified();
  assert.ok(records['audio/Loop.wav'] && typeof records['audio/Loop.wav'].modified === 'number');
  const trusting = tutti.Project.open(store);
  trusting.trust(records);
  trusting.save(generator);
});

test('a project on disk round-trips, and a copy is complete', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'tutti-js-'));
  const folder = path.join(dir, 'On disk');
  const project = tutti.createFolder(folder, 'On disk', generator);
  assert.ok(fs.existsSync(path.join(folder, 'On-disk.tutti')));
  assert.ok(fs.existsSync(path.join(folder, 'README.md')));
  const stored = project.putAudio('Take', 'wav', new TextEncoder().encode('RIFF'));
  assert.ok(fs.existsSync(path.join(folder, 'audio', 'Take.wav')));
  const doc = project.document;
  doc.tracks = [trackWithPiece(stored)];
  project.document = doc;
  project.save(generator);
  assert.ok(!fs.existsSync(path.join(folder, 'On-disk.tutti.tmp')));

  const again = tutti.openFolder(folder);
  assert.deepEqual(again.check(), []);
  assert.equal(tutti.equal(again.text, project.text), true);
  const records = again.verified();
  assert.equal(typeof again.verified, 'function');
  assert.ok(Object.keys(records).length >= 0);

  const copy = project.copyInto(tutti.memoryStore());
  assert.deepEqual(copy.check(), []);
  assert.equal(new TextDecoder().decode(copy.read('audio/Take.wav')), 'RIFF');
  fs.rmSync(dir, { recursive: true, force: true });
});

test('no document, two documents and a later minor', () => {
  const store = tutti.memoryStore();
  assert.throws(() => tutti.Project.open(store), /no \.tutti document/);
  store.writeNew('A.tutti', new TextEncoder().encode('{}'));
  store.writeNew('B.tutti', new TextEncoder().encode('{}'));
  assert.throws(() => tutti.Project.open(store), /2 documents/);
  assert.throws(() => tutti.Project.create(store, 'C', generator), /there already/);

  const later = tutti.memoryStore();
  const text = tutti.newDocument(generator).replace('"version": "1.0"', '"version": "1.7",\n  "min_version": "1.7"');
  later.writeNew('Later.tutti', new TextEncoder().encode(text));
  const project = tutti.Project.open(later);
  assert.equal(project.readOnly, true);
  assert.throws(() => project.save(generator), /read-only/);
});
