import type { MemoryStore } from './api';

export type * from './api';
export type * from './schema';

/** A project folder in the browser's own storage, read whole into a memory store. */
export interface BrowserFolder {
  /** The store a `Project` opens, creates or adopts. */
  store: MemoryStore;
  /** Writes back what changed since the folder was read or last flushed: other files first, the document last, put in place in one step. */
  flush(): Promise<{ written: number }>;
}

/** The package's functions in a browser: all but Node's own, and `browserFolder`. */
export type Tutti = Omit<typeof import('./api'), 'nodeStore' | 'openFolder' | 'createFolder'> & {
  /** A project folder in the browser's own storage (`navigator.storage.getDirectory()`, or a folder in it). */
  browserFolder(directory: FileSystemDirectoryHandle): Promise<BrowserFolder>;
};

/** Starts the WebAssembly, once, and gives the package's functions. `source`: where `tutti_bg.wasm` is, when not beside `pkg-web/tutti.js`. */
export function load(source?: string | URL | Response | BufferSource): Promise<Tutti>;
