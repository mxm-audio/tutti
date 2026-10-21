// A project as one file, `<Name>.tutti.zip`, in memory: packed from one store and unpacked into
// another, the same files; and the hostile fixtures refused with their named errors, nothing
// written.

'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');
const tutti = require('..');

const generator = { name: 'tutti-js-tests', version: '0.1', url: 'https://github.com/mxm-audio/tutti' };
const hostile = path.join(__dirname, '..', '..', 'examples', 'conformance', 'hostile');

test('a project in memory packs into bytes and unpacks into another store the same', () => {
  const store = tutti.memoryStore();
  const project = tutti.Project.create(store, 'Round Trip', generator);
  const stored = project.putAudio('Take', 'wav', new TextEncoder().encode('RIFF made audio'));
  project.save(generator);

  const bytes = tutti.packZip(store);
  assert.ok(bytes instanceof Uint8Array && bytes.length > 0);
  // The document is the first entry: a local header, then its name (spec 2.5).
  const first = new TextDecoder().decode(bytes.subarray(30, 30 + 'Round-Trip.tutti'.length));
  assert.equal(first, 'Round-Trip.tutti');

  const into = tutti.memoryStore();
  assert.equal(tutti.unpackZip(bytes, into), 3);
  for (const reference of ['Round-Trip.tutti', 'README.md', stored.reference]) {
    assert.deepEqual(into.read(reference), store.read(reference), reference);
  }
  const reopened = tutti.Project.open(into);
  assert.ok(tutti.equal(reopened.text, project.text));
});

test('a store that holds a project is not unpacked into', () => {
  const store = tutti.memoryStore();
  tutti.Project.create(store, 'One', generator);
  assert.throws(() => tutti.unpackZip(tutti.packZip(store), store), /holds a project already/);
});

test('every hostile project as one file is refused with its named error, nothing written', () => {
  let seen = 0;
  for (const name of fs.readdirSync(hostile)) {
    const folder = path.join(hostile, name);
    const zip = fs.readdirSync(folder).find((file) => file.toLowerCase().endsWith('.tutti.zip'));
    if (!zip) continue;
    seen++;
    const expected = JSON.parse(fs.readFileSync(path.join(folder, 'expected.json'), 'utf8'));
    const into = tutti.memoryStore();
    assert.throws(
      () => tutti.unpackZip(fs.readFileSync(path.join(folder, zip)), into),
      (error) => error.message.includes(expected.error),
      name,
    );
    assert.equal(into.files().size, 0, `${name}: something was written`);
  }
  assert.ok(seen >= 2, 'the hostile fixtures include projects as one file');
});
