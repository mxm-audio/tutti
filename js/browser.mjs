// @mxm-audio/tutti for a browser: the Rust core compiled to WebAssembly for the web
// (`pkg-web/`), wrapped by `core.mjs` as Node's build is, plus `browserFolder`, a project in the
// browser's own file storage. `load()` fetches and starts the WebAssembly once and gives the
// package's functions.
//
// WebAssembly can't wait on a promise, and the browser's storage answers only with promises, so
// a project is read whole into a `memoryStore` before its `Project` opens it, and what a save
// changed is written back afterwards (`flush`), in the format's own order: new and changed files
// first, the document last.

import init, * as wasm from './pkg-web/tutti.js';
import { make } from './core.mjs';

let loading = null;

/**
 * Starts the WebAssembly (once) and gives the package's functions. `source` is where its
 * `tutti_bg.wasm` is, when not beside `pkg-web/tutti.js`: a URL, a `Response` or the bytes.
 */
export function load(source) {
  if (!loading) {
    loading = init(source === undefined ? undefined : { module_or_path: source }).then(() => {
      const api = make(wasm);
      return { ...api, browserFolder: (directory) => browserFolder(api, directory) };
    });
  }
  return loading;
}

/** Whether a reference names a project's document: `<Name>.tutti` at the top. */
const isDocument = (reference) => !reference.includes('/') && reference.endsWith('.tutti');

/**
 * A project folder in the browser's own storage (a `FileSystemDirectoryHandle`, such as one
 * from `navigator.storage.getDirectory()`), read whole into a `memoryStore`: give `store` to
 * `Project.open`, `create` or `adopt`, and after a save `await flush()`.
 *
 * `flush` writes back every file the store holds that changed since it was read or last
 * flushed: every other file first, each whole, then the document under a temporary name, put
 * in place of the old one in one step (`move`). So the folder always holds a complete
 * document, the old one until the new one is in place, and a flush that fails, for a full
 * storage or anything else, leaves the last good version as it was. `flush` never removes a
 * file: a save in the store removes none of the project's.
 */
async function browserFolder(api, directory) {
  const store = api.memoryStore();
  // Each file as last read or flushed, by the bytes the store held then: a write in the store
  // puts new bytes, so a file whose bytes are not these has changed.
  const written = new Map();

  async function read(folder, prefix) {
    for await (const [name, handle] of folder.entries()) {
      const reference = prefix === '' ? name : `${prefix}/${name}`;
      if (handle.kind === 'directory') {
        await read(handle, reference);
      } else {
        const file = await handle.getFile();
        store.put(reference, new Uint8Array(await file.arrayBuffer()));
        written.set(reference, store.read(reference));
      }
    }
  }
  await read(directory, '');

  async function folderOf(reference) {
    const parts = reference.split('/');
    const name = parts.pop();
    let folder = directory;
    for (const part of parts) folder = await folder.getDirectoryHandle(part, { create: true });
    return { folder, name };
  }

  async function writeWhole(folder, name, bytes) {
    const handle = await folder.getFileHandle(name, { create: true });
    const writable = await handle.createWritable();
    try {
      await writable.write(bytes);
      await writable.close();
    } catch (error) {
      await writable.abort().catch(() => {});
      throw error;
    }
    return handle;
  }

  async function flush() {
    const changed = [...store.files()].filter(([reference, bytes]) => written.get(reference) !== bytes);
    const files = changed.filter(([reference]) => !isDocument(reference));
    const documents = changed.filter(([reference]) => isDocument(reference));
    for (const [reference, bytes] of files) {
      const { folder, name } = await folderOf(reference);
      await writeWhole(folder, name, bytes);
      written.set(reference, bytes);
    }
    for (const [reference, bytes] of documents) {
      const temporary = await writeWhole(directory, `${reference}.tmp`, bytes);
      // The folder and the name: WebKit takes only this form of `move`, every browser takes it.
      await temporary.move(directory, reference);
      written.set(reference, bytes);
    }
    return { written: files.length + documents.length };
  }

  return { store, flush };
}
