// @mxm-audio/tutti's wrapper, the same in Node and in a browser: the Rust core compiled to
// WebAssembly, given here by the entry that loaded it (`index.js` for Node, `browser.mjs` for
// a browser), wrapped so JavaScript sees plain objects. Documents go in and out as strings (the
// file's text) or as the objects the schema describes; points, maps and generators as objects.
// A project lives in a store the host supplies; `Project` runs the format's own procedure over it.

/** The package's functions over a loaded build of the core. */
export function make(wasm) {
  const json = (fn) => (...args) => JSON.parse(fn(...args));
  const orUndefined = (x) => (x === null ? undefined : x);

  /** The format's version this build writes. */
  const version = () => wasm.version();

  /** The `about` sentence a writer puts in the manifest. */
  const about = () => wasm.about();

  /** A document in any form, returned in the pretty form; throws with the reader's reason. */
  const fmt = (text) => wasm.fmt(text);

  /** The document's JCS bytes; `equal` compares them with `generator` left out. */
  const jcs = (text) => wasm.jcs(text);

  /** Whether two documents are equal as the spec means it: `generator` left out. */
  const equal = (a, b) => wasm.equal(a, b);

  /** Whether a document opens read-only on this version (a later `min_version`). */
  const isReadOnly = (text) => wasm.is_read_only(text);

  /** Validation: an array of `{severity, pointer, message}`, errors and warnings. */
  const validate = json(wasm.validate);

  /**
   * The keys the document carries that this version doesn't know, once each: an array of
   * `{key, pointer, count}`. They are kept, never an error; a misspelt field is one of them.
   */
  const unknownKeys = json(wasm.unknown_keys);

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

  // --- stores ------------------------------------------------------------------------------------

  /** An error a store throws, with the `code` the core reads: ENOENT, EEXIST or LINK. */
  function storeError(code, message) {
    const error = new Error(message);
    error.code = code;
    return error;
  }

  /**
   * A store in memory, shared by every reference to it: a project a host builds without a file
   * system, written out afterwards with `files()`. Modification times are a clock that ticks once
   * per write; `put` changes a file behind the project's back, `putUnseen` does so keeping its time.
   */
  function memoryStore() {
    const files = new Map();
    const stamps = new Map();
    const folders = new Set();
    let clock = 0;
    const stamp = (reference) => stamps.set(reference, BigInt(++clock));
    const missing = (reference) => storeError('ENOENT', `${reference}: missing`);
    return {
      list(folder) {
        const prefix = folder === '' ? '' : `${folder}/`;
        const names = new Map();
        for (const path of files.keys()) {
          if (!path.startsWith(prefix)) continue;
          const rest = path.slice(prefix.length);
          const slash = rest.indexOf('/');
          if (slash < 0) names.set(rest, 'file');
          else names.set(rest.slice(0, slash), 'folder');
        }
        for (const path of folders) {
          if (!path.startsWith(prefix)) continue;
          const first = path.slice(prefix.length).split('/')[0];
          if (first !== '' && !names.has(first)) names.set(first, 'folder');
        }
        if (names.size === 0 && folder !== '' && !folders.has(folder)) throw missing(folder);
        return JSON.stringify([...names].map(([name, kind]) => ({ name, kind })));
      },
      read(reference) {
        const bytes = files.get(reference);
        if (bytes === undefined) throw missing(reference);
        return bytes;
      },
      size(reference) {
        const bytes = files.get(reference);
        if (bytes === undefined) throw missing(reference);
        return bytes.length;
      },
      modified(reference) {
        if (!files.has(reference)) throw missing(reference);
        return stamps.get(reference) ?? null;
      },
      exists(reference) {
        return files.has(reference);
      },
      makeFolder(folder) {
        if (folder !== '') folders.add(folder);
      },
      writeNew(reference, bytes) {
        if (files.has(reference)) throw storeError('EEXIST', `${reference}: already there`);
        files.set(reference, Uint8Array.from(bytes));
        stamp(reference);
      },
      renameOver(from, to) {
        const bytes = files.get(from);
        if (bytes === undefined) throw missing(from);
        files.delete(from);
        stamps.delete(from);
        files.set(to, bytes);
        stamp(to);
      },
      syncFolder() {},
      remove(reference) {
        if (!files.delete(reference)) throw missing(reference);
        stamps.delete(reference);
      },
      /** Every file, by reference. */
      files() {
        return new Map(files);
      },
      /** Puts a file whatever is there, as a change the clock shows. */
      put(reference, bytes) {
        files.set(reference, Uint8Array.from(bytes));
        stamp(reference);
      },
      /** Puts a file whatever is there, keeping its time: a forged change a save can't see. */
      putUnseen(reference, bytes) {
        files.set(reference, Uint8Array.from(bytes));
      },
    };
  }

  // --- the project -----------------------------------------------------------------------------

  /** A new file being written into a project: `write` bytes, then `finish` for its facts. */
  class Incoming {
    constructor(inner) {
      this.inner = inner;
    }

    /** The reference the file will have. */
    get reference() {
      return this.inner.reference();
    }

    write(bytes) {
      this.inner.write(bytes);
    }

    /** Closes the file: `{reference, sha256, bytes}`. */
    finish() {
      return JSON.parse(this.inner.finish());
    }
  }

  /**
   * A project in a store: opened, created or adopted; its document an object you edit and set
   * back; `save` runs the format's own procedure (every referenced file present and verified, the
   * files index built from the files, the document validated, the README written, the document
   * renamed into place) and throws with the reason when it refuses.
   */
  class Project {
    constructor(inner) {
      this.inner = inner;
    }

    /** Opens the project in a store; throws when there is no document, or two, or it can't be read. */
    static open(store) {
      return new Project(wasm.Project.open(store));
    }

    /** Creates a project in a store with no document: saved at once with its README. */
    static create(store, title, generator) {
      return new Project(wasm.Project.create(store, title, JSON.stringify(generator)));
    }

    /** A project over a document you built, to be saved under `documentName` (`<Name>.tutti`). */
    static adopt(store, documentName, document) {
      const text = typeof document === 'string' ? document : JSON.stringify(document);
      return new Project(wasm.Project.adopt(store, documentName, text));
    }

    /** Validation's findings at open, errors included. */
    get report() {
      return JSON.parse(this.inner.report());
    }

    /** Whether the document needs a later minor version than this build writes: `save` refuses. */
    get readOnly() {
      return this.inner.readOnly();
    }

    /** The document, in the pretty form. */
    get text() {
      return this.inner.document();
    }

    /** The document as an object (a fresh copy each time; set it back to change it). */
    get document() {
      return JSON.parse(this.inner.document());
    }

    set document(value) {
      this.inner.setDocument(typeof value === 'string' ? value : JSON.stringify(value));
    }

    get documentName() {
      return this.inner.documentName();
    }

    /** The SHA-256 of the document's bytes as last read or written. */
    get documentHash() {
      return this.inner.documentHash();
    }

    get title() {
      return this.inner.title();
    }

    /** Saves: `{warnings, readme_written, document_hash}`; throws with the refusal's reason. */
    save(generator) {
      return JSON.parse(this.inner.save(JSON.stringify(generator)));
    }

    /** `save`, hashing every indexed file again: for an export. */
    saveVerified(generator) {
      return JSON.parse(this.inner.saveVerified(JSON.stringify(generator)));
    }

    /** Rewrites the document as held, in the pretty form, and nothing else. */
    rewrite() {
      return this.inner.rewrite();
    }

    /** Validation and the files check together: `{severity, pointer, message}` each. */
    check() {
      return JSON.parse(this.inner.check());
    }

    /** A file of the project, by reference. */
    read(reference) {
      return this.inner.read(reference);
    }

    /** Puts audio in `audio/` whole, the same content once: `{reference, sha256, bytes}`. */
    putAudio(name, extension, bytes) {
      return JSON.parse(this.inner.putAudio(name, extension, bytes));
    }

    /** Puts a render in `renders/` whole: `{reference, sha256, bytes}`. */
    putRender(name, extension, bytes) {
      return JSON.parse(this.inner.putRender(name, extension, bytes));
    }

    /** Puts a plugin state in `plugins/states/`, named by its content: `{sha256, kind, path}`. */
    putState(kind, bytes) {
      return JSON.parse(this.inner.putState(kind, bytes));
    }

    /** The file in `audio/` with exactly these bytes, or `undefined`. */
    findAudio(bytes) {
      const found = this.inner.findAudio(bytes);
      return found === undefined ? undefined : JSON.parse(found);
    }

    /** A new audio file to stream into `audio/`. */
    newAudio(name, extension) {
      return new Incoming(this.inner.newAudio(name, extension));
    }

    /** A new file to stream into `renders/`. */
    newRender(name, extension) {
      return new Incoming(this.inner.newRender(name, extension));
    }

    /** Drops an index entry no known field claims whose file is gone, on purpose. */
    forgetFile(path) {
      this.inner.forgetFile(path);
    }

    /** Every file verified so far, `{reference: {sha256, bytes, modified}}`: keep it, give it back with `trust`. */
    verified() {
      return JSON.parse(this.inner.verified());
    }

    /** Takes records kept from an earlier session (what `verified` gave). */
    trust(records) {
      this.inner.trust(typeof records === 'string' ? records : JSON.stringify(records));
    }

    /** How many files are being written or stored and not yet named by the document. */
    get unnamed() {
      return this.inner.unnamed();
    }

    /** Copies the project into another store, complete and verified. */
    copyInto(store) {
      return new Project(this.inner.copyInto(store));
    }
  }

  // --- one file --------------------------------------------------------------------------------

  /**
   * Packs the project in a store into one file, `<Name>.tutti.zip` (spec 2.5): its bytes, the
   * document first and stored. Throws with the reason when the store holds no valid project.
   */
  const packZip = (store) => wasm.pack(store);

  /**
   * Unpacks one file's bytes into a store that holds no project, refusing what a reader must
   * refuse (spec 2.5) with nothing written left behind: the number of files written.
   */
  const unpackZip = (bytes, store) => wasm.unpack(bytes, store);

  return {
    version,
    about,
    fmt,
    jcs,
    equal,
    isReadOnly,
    validate,
    unknownKeys,
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
    storeError,
    memoryStore,
    packZip,
    unpackZip,
    Project,
    Incoming,
  };
}
