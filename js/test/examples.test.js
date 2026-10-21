// The package over the repository's examples: every example is the pretty form, validates and
// checks out; the hostile fixtures are refused; the tempo vectors of the ramp fixture hold; the
// smaller functions do what they say.

'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');
const tutti = require('..');

const examples = path.join(__dirname, '..', '..', 'examples');
const folders = (dir) =>
  fs
    .readdirSync(dir, { withFileTypes: true })
    .filter((e) => e.isDirectory())
    .map((e) => path.join(dir, e.name))
    .sort();

const exampleFolders = [
  path.join(examples, 'minimal'),
  path.join(examples, 'typical'),
  path.join(examples, 'every-feature'),
  ...folders(path.join(examples, 'conformance', 'l0')),
];

test('the version and the about sentence', () => {
  assert.equal(tutti.version(), '1.0');
  assert.match(tutti.about(), /tutti 1\.0/);
});

test('every example is the pretty form, validates and checks out', () => {
  for (const folder of exampleFolders) {
    const project = tutti.openFolder(folder);
    assert.deepEqual(project.report.filter((i) => i.severity === 'error'), [], folder);
    assert.equal(project.readOnly, false);
    const errors = project.check().filter((i) => i.severity === 'error');
    assert.deepEqual(errors, [], folder);
    const text = fs.readFileSync(path.join(folder, project.documentName), 'utf8');
    assert.equal(project.text, text, `${folder}: not the pretty form`);
    assert.equal(tutti.equal(text, tutti.fmt(text)), true);
  }
  const every = tutti.openFolder(path.join(examples, 'every-feature'));
  assert.equal(tutti.levelNeeded(every.text), 2);
  assert.ok(tutti.features(every.text).some((f) => f.name === 'warps' && f.level === 1));
  const minimal = tutti.openFolder(path.join(examples, 'minimal'));
  assert.deepEqual(tutti.features(minimal.text), []);
  assert.equal(tutti.levelNeeded(minimal.text), 0);
});

test('every hostile fixture is refused as its expectation says', () => {
  for (const folder of folders(path.join(examples, 'conformance', 'hostile'))) {
    // A project as one file is the command's to unpack; the package reads folders.
    if (fs.readdirSync(folder).some((name) => name.endsWith('.tutti.zip'))) continue;
    const expected = JSON.parse(fs.readFileSync(path.join(folder, 'expected.json'), 'utf8'));
    let message;
    try {
      const project = tutti.openFolder(folder);
      const first = project.report.find((i) => i.severity === 'error') || project.check().find((i) => i.severity === 'error');
      message = first ? `error ${first.pointer}: ${first.message}` : 'accepted';
    } catch (e) {
      message = e.message;
    }
    assert.ok(message.includes(expected.error), `${folder}: expected ${JSON.stringify(expected.error)}, got ${JSON.stringify(message)}`);
  }
});

test('the tempo vectors of the ramp fixture', () => {
  const folder = path.join(examples, 'conformance', 'l0', 'ramp');
  const doc = JSON.parse(fs.readFileSync(path.join(folder, 'Ramp.tutti'), 'utf8'));
  const expected = JSON.parse(fs.readFileSync(path.join(folder, 'expected.json'), 'utf8'));
  for (const beat of expected.beats) {
    assert.equal(tutti.samplesAt(doc.ppq, doc.tempo, beat.tick, expected.rate), beat.sample_48000);
    assert.ok(Math.abs(tutti.secondsAt(doc.ppq, doc.tempo, beat.tick) - beat.seconds) < 1e-9);
    assert.ok(Math.abs(tutti.tickAt(doc.ppq, doc.tempo, beat.seconds) - beat.tick) < 1e-6);
  }
  assert.equal(tutti.bpmAt(doc.ppq, doc.tempo, 1920), 90);
});

test('audio placement: a loop at 95 in a project at 87', () => {
  const map = [{ frame: 0, tick: 0 }];
  assert.equal(Math.round(tutti.tickOfFrame(960, 44100, map, 95, 222821)), 7680);
  assert.ok(Math.abs(tutti.rateAt(960, 44100, map, 95, 100, 87) / 44100 - 87 / 95) < 1e-9);
  assert.equal(tutti.tickOfFrame(960, 44100, map, null, 10), undefined);
  assert.equal(tutti.rateAt(960, 44100, map, null, 0, 87), 44100);
  assert.ok(Math.abs(tutti.frameAtTick(960, 44100, map, 95, 7680) - 222821) < 1);
});

test('ids, hashes, the README and a new document', () => {
  const id = tutti.ulid('trk');
  assert.match(id, /^trk_[0-9A-HJKMNP-TV-Z]{26}$/);
  assert.equal(tutti.isId(id), true);
  assert.equal(tutti.isId('a b'), false);
  assert.equal(tutti.sha256Hex(new TextEncoder().encode('abc')), 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
  const readme = tutti.readme('My Project', { name: 'my-daw', version: '0.1' }, 'My-Project.tutti');
  assert.ok(readme.startsWith('# My Project') && readme.includes('my-daw 0.1') && !readme.includes('{title}'));
  const fresh = tutti.newDocument({ name: 'my-daw' });
  assert.deepEqual(tutti.validate(fresh), []);
  const parsed = JSON.parse(fresh);
  assert.equal(parsed.format, 'tutti');
  assert.equal(parsed.ppq, 960);
  assert.equal(tutti.fmt(fresh), fresh);
});

test('a render fingerprint follows the track and the maps', () => {
  const text = fs.readFileSync(path.join(examples, 'typical', 'Typical.tutti'), 'utf8');
  const doc = JSON.parse(text);
  const bass = doc.tracks.find((t) => t.name === 'Bass');
  const a = tutti.renderFingerprint(text, bass.id, 0, 3840);
  assert.equal(a.length, 64);
  assert.notEqual(tutti.renderFingerprint(text, bass.id, 0, 7680), a);
  assert.throws(() => tutti.renderFingerprint(text, 'trk_nope', 0, 1));
});
