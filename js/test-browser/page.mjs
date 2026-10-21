// The checks `run.mjs` runs in a browser: the package's browser entry started, a project in
// memory saved, packed into one file and unpacked the same, a hostile file refused with
// nothing written, and a project in the browser's own storage created, flushed, read back,
// changed and flushed again. Each result goes into `window.results`.

import { load } from '../browser.mjs';

const results = [];
const generator = { name: 'tutti-browser-test', version: '0.1', url: 'https://github.com/mxm-audio/tutti' };

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

/** A check that can't run in this browser, said so with its reason: never taken for a pass. */
class Skip extends Error {}

async function check(name, run) {
  try {
    await run();
    results.push({ name, ok: true });
  } catch (error) {
    if (error instanceof Skip) {
      results.push({ name, ok: true, skipped: error.message });
      return;
    }
    const message = error && error.message ? `${error.name}: ${error.message}` : String(error);
    results.push({ name, ok: false, error: `${message}\n${(error && error.stack) || ''}` });
  }
}

let tutti = null;
await check('the WebAssembly starts and the core reads a document', async () => {
  tutti = await load();
  assert(tutti.version() === '1.0', `version ${tutti.version()}`);
  const errors = tutti.validate(tutti.newDocument(generator)).filter((issue) => issue.severity === 'error');
  assert(errors.length === 0, JSON.stringify(errors));
});

if (tutti) {
  await check('a project in memory is saved, packed into one file and unpacked the same', () => {
    const store = tutti.memoryStore();
    const project = tutti.Project.create(store, 'Round Trip', generator);
    project.putAudio('Take', 'wav', new TextEncoder().encode('RIFF made audio'));
    project.save(generator);
    const into = tutti.memoryStore();
    const written = tutti.unpackZip(tutti.packZip(store), into);
    assert(written === store.files().size, `${written} of ${store.files().size} files`);
    assert(tutti.equal(tutti.Project.open(into).text, project.text), 'the documents differ');
  });

  await check('a hostile project as one file is refused, nothing written', async () => {
    const response = await fetch('../../examples/conformance/hostile/zip-slip/Slip.tutti.zip');
    assert(response.ok, `the fixture: ${response.status}`);
    const into = tutti.memoryStore();
    let refused = null;
    try {
      tutti.unpackZip(new Uint8Array(await response.arrayBuffer()), into);
    } catch (error) {
      refused = error.message;
    }
    assert(refused && refused.includes('leaves the folder'), `refused with ${refused}`);
    assert(into.files().size === 0, 'something was written');
  });

  await check("a project in the browser's storage is flushed, read back, changed and flushed again", async () => {
    const root = await navigator.storage.getDirectory();
    // Playwright's WebKit for Windows keeps nothing `createWritable` writes; iOS's WebKit does.
    const probe = await root.getFileHandle('tutti-probe.txt', { create: true });
    const writable = await probe.createWritable();
    await writable.write('kept');
    await writable.close();
    const kept = (await (await root.getFileHandle('tutti-probe.txt')).getFile()).size > 0;
    await root.removeEntry('tutti-probe.txt');
    if (!kept) throw new Skip('this browser keeps nothing createWritable writes');
    const name = `tutti-test-${Date.now()}`;
    const directory = await root.getDirectoryHandle(name, { create: true });
    try {
      const folder = await tutti.browserFolder(directory);
      const project = tutti.Project.create(folder.store, 'Stored Song', generator);
      const first = await folder.flush();
      assert(first.written === 2, `the first flush wrote ${first.written} files, not the document and the README`);

      const again = await tutti.browserFolder(directory);
      const reopened = tutti.Project.open(again.store);
      assert(tutti.equal(reopened.text, project.text), 'the document read back differs');
      const unchanged = await again.flush();
      assert(unchanged.written === 0, `a flush with nothing changed wrote ${unchanged.written}`);

      const document = reopened.document;
      document.tempo[0].bpm = 99;
      reopened.document = document;
      reopened.save(generator);
      await again.flush();

      const third = await tutti.browserFolder(directory);
      const bpm = tutti.Project.open(third.store).document.tempo[0].bpm;
      assert(bpm === 99, `the change read back as ${bpm}`);
      for await (const [entry] of directory.entries()) {
        assert(!entry.endsWith('.tmp'), `${entry} left behind`);
      }
    } finally {
      await root.removeEntry(name, { recursive: true });
    }
  });
}

window.results = results;
