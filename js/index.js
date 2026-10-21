// @mxm-audio/tutti for Node: the Rust core compiled to WebAssembly for Node (`pkg/`), wrapped by
// `core.mjs` as a browser's build is, plus a store over Node's files (`nodeStore`) and the
// one-liners over it. A browser imports `browser.mjs` instead.

'use strict';

const wasm = require('./pkg/tutti.js');
const { make } = require('./core.mjs');

const api = make(wasm);
const { storeError, Project } = api;

/**
 * Node only: a store over a folder on disk, through synchronous file calls. References resolve
 * inside the folder, each part a plain name, no part a link; a new file is flushed before it
 * counts; a rename replaces in one step.
 */
function nodeStore(root) {
  const fs = require('node:fs');
  const path = require('node:path');
  const pathOf = (reference) => {
    let p = root;
    if (reference === '') return p;
    for (const part of reference.split('/')) {
      if (part === '' || part === '.' || part === '..' || part.includes('\\')) {
        throw storeError('EINVAL', `${reference}: not a reference`);
      }
      p = path.join(p, part);
      let stat;
      try {
        stat = fs.lstatSync(p);
      } catch {
        continue;
      }
      if (stat.isSymbolicLink()) throw storeError('LINK', `${reference}: a link, which a reader refuses`);
    }
    return p;
  };
  const fsync = (p, flags) => {
    let fd;
    try {
      fd = fs.openSync(p, flags);
    } catch {
      return;
    }
    try {
      fs.fsyncSync(fd);
    } catch {
      // A folder that can't be flushed (Windows): the rename has happened by now.
    } finally {
      fs.closeSync(fd);
    }
  };
  return {
    list(folder) {
      const p = pathOf(folder);
      const entries = fs.readdirSync(p, { withFileTypes: true }).map((e) => ({
        name: e.name,
        kind: e.isSymbolicLink() ? 'other' : e.isDirectory() ? 'folder' : e.isFile() ? 'file' : 'other',
      }));
      return JSON.stringify(entries);
    },
    read(reference) {
      return fs.readFileSync(pathOf(reference));
    },
    size(reference) {
      return fs.statSync(pathOf(reference)).size;
    },
    modified(reference) {
      return fs.statSync(pathOf(reference), { bigint: true }).mtimeNs;
    },
    exists(reference) {
      const p = pathOf(reference);
      try {
        return fs.lstatSync(p).isFile();
      } catch (e) {
        if (e.code === 'ENOENT') return false;
        throw e;
      }
    },
    makeFolder(folder) {
      fs.mkdirSync(pathOf(folder), { recursive: true });
    },
    writeNew(reference, bytes) {
      const p = pathOf(reference);
      fs.mkdirSync(path.dirname(p), { recursive: true });
      fs.writeFileSync(p, bytes, { flag: 'wx' });
      fsync(p, 'r+');
    },
    renameOver(from, to) {
      fs.renameSync(pathOf(from), pathOf(to));
    },
    syncFolder(folder) {
      if (process.platform !== 'win32') fsync(pathOf(folder), 'r');
    },
    remove(reference) {
      fs.unlinkSync(pathOf(reference));
    },
  };
}

/** Node only: opens the project in a folder on disk. */
const openFolder = (folder) => Project.open(nodeStore(folder));

/** Node only: creates a project in a folder on disk that holds no document. */
const createFolder = (folder, title, generator) => Project.create(nodeStore(folder), title, generator);

module.exports = {
  ...api,
  nodeStore,
  openFolder,
  createFolder,
};
