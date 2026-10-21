//! The core's `Project` for JavaScript, over a store the host supplies: an object with
//! synchronous methods over its files (`js/index.js` ships one over Node's `fs` and one in
//! memory; a browser host prepares its own first, since WebAssembly can't block on a promise).
//! Documents cross as their text, results as JSON; a recording streams through a handle.

use std::collections::BTreeMap;
use std::io::{self, Write};

use js_sys::{Reflect, Uint8Array};
use serde_json::{Value, json};
use tutti::folder::{Entry, Folder, FolderError, Kind, Store};
use tutti::{Generator, ProjectError, Stored, Verified};
use wasm_bindgen::prelude::*;

use crate::issues_json;

#[wasm_bindgen]
extern "C" {
    /// A store the host supplies. Every method is synchronous and throws an `Error` whose
    /// `code` says what happened: `ENOENT` (missing), `EEXIST` (there already), `LINK` (a link
    /// where a file or folder was expected); any other error is the store's own reason.
    pub type JsStore;

    /// The entries directly in `folder` (`""` the root), as JSON: `[{"name", "kind"}]` with
    /// `kind` `"file"`, `"folder"` or `"other"`; throws `ENOENT` for a folder that isn't there.
    #[wasm_bindgen(method, catch)]
    fn list(this: &JsStore, folder: &str) -> Result<String, JsValue>;

    #[wasm_bindgen(method, catch)]
    fn read(this: &JsStore, reference: &str) -> Result<Uint8Array, JsValue>;

    #[wasm_bindgen(method, catch)]
    fn size(this: &JsStore, reference: &str) -> Result<f64, JsValue>;

    /// When the file was last changed: a `BigInt` of nanoseconds since the Unix epoch, a number
    /// of milliseconds, or `null` when the store can't say.
    #[wasm_bindgen(method, catch)]
    fn modified(this: &JsStore, reference: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method, catch)]
    fn exists(this: &JsStore, reference: &str) -> Result<bool, JsValue>;

    #[wasm_bindgen(method, catch, js_name = makeFolder)]
    fn make_folder(this: &JsStore, folder: &str) -> Result<(), JsValue>;

    /// Writes a new file whole and durably; throws `EEXIST` when a file is there already.
    #[wasm_bindgen(method, catch, js_name = writeNew)]
    fn write_new(this: &JsStore, reference: &str, bytes: &[u8]) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch, js_name = renameOver)]
    fn rename_over(this: &JsStore, from: &str, to: &str) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch, js_name = syncFolder)]
    fn sync_folder(this: &JsStore, folder: &str) -> Result<(), JsValue>;

    #[wasm_bindgen(method, catch)]
    fn remove(this: &JsStore, reference: &str) -> Result<(), JsValue>;
}

/// What a thrown value says, as the core's error.
fn folder_error(thrown: JsValue) -> FolderError {
    let property = |name: &str| {
        Reflect::get(&thrown, &JsValue::from_str(name))
            .ok()
            .and_then(|v| v.as_string())
    };
    match property("code").as_deref() {
        Some("ENOENT" | "MISSING") => FolderError::Missing,
        Some("EEXIST" | "EXISTS") => FolderError::Exists,
        Some("LINK" | "ELOOP") => FolderError::Link,
        _ => FolderError::Other(
            property("message")
                .or_else(|| thrown.as_string())
                .unwrap_or_else(|| "the store refused".to_owned()),
        ),
    }
}

/// The host's store as the core's: a cheap handle on the host's object.
#[derive(Clone)]
pub struct HostStore {
    js: JsValue,
}

impl HostStore {
    fn new(store: JsStore) -> Self {
        Self { js: store.into() }
    }

    fn js(&self) -> JsStore {
        self.js.clone().unchecked_into()
    }
}

impl Folder for HostStore {
    fn read(&self, reference: &str) -> Result<Vec<u8>, FolderError> {
        self.js()
            .read(reference)
            .map(|bytes| bytes.to_vec())
            .map_err(folder_error)
    }

    fn size(&self, reference: &str) -> Result<u64, FolderError> {
        self.js()
            .size(reference)
            .map(|size| size as u64)
            .map_err(folder_error)
    }

    fn modified(&self, reference: &str) -> Result<Option<u64>, FolderError> {
        let value = self.js().modified(reference).map_err(folder_error)?;
        if value.is_null() || value.is_undefined() {
            return Ok(None);
        }
        if let Ok(nanos) = u64::try_from(value.clone()) {
            return Ok(Some(nanos));
        }
        Ok(value.as_f64().map(|millis| (millis * 1e6) as u64))
    }
}

/// A new file's bytes, held until it is closed: the host's store writes whole files.
pub struct BufferWriter {
    reference: String,
    bytes: Vec<u8>,
}

impl Write for BufferWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(serde::Deserialize)]
struct JsEntry {
    name: String,
    kind: String,
}

impl Store for HostStore {
    type Writer = BufferWriter;

    fn list(&self, folder: &str) -> Result<Vec<Entry>, FolderError> {
        let text = self.js().list(folder).map_err(folder_error)?;
        let entries: Vec<JsEntry> =
            serde_json::from_str(&text).map_err(|e| FolderError::Other(e.to_string()))?;
        Ok(entries
            .into_iter()
            .map(|e| Entry {
                name: e.name,
                kind: match e.kind.as_str() {
                    "file" => Kind::File,
                    "folder" => Kind::Folder,
                    _ => Kind::Other,
                },
            })
            .collect())
    }

    fn exists(&self, reference: &str) -> Result<bool, FolderError> {
        self.js().exists(reference).map_err(folder_error)
    }

    fn make_folder(&self, folder: &str) -> Result<(), FolderError> {
        self.js().make_folder(folder).map_err(folder_error)
    }

    fn write_new(&self, reference: &str, bytes: &[u8]) -> Result<(), FolderError> {
        self.js().write_new(reference, bytes).map_err(folder_error)
    }

    fn open_new(&self, reference: &str) -> Result<Self::Writer, FolderError> {
        if self.exists(reference)? {
            return Err(FolderError::Exists);
        }
        Ok(BufferWriter {
            reference: reference.to_owned(),
            bytes: Vec::new(),
        })
    }

    fn close(&self, writer: Self::Writer) -> Result<(), FolderError> {
        self.write_new(&writer.reference, &writer.bytes)
    }

    fn rename_over(&self, from: &str, to: &str) -> Result<(), FolderError> {
        self.js().rename_over(from, to).map_err(folder_error)
    }

    fn sync_folder(&self, folder: &str) -> Result<(), FolderError> {
        self.js().sync_folder(folder).map_err(folder_error)
    }

    fn remove(&self, reference: &str) -> Result<(), FolderError> {
        self.js().remove(reference).map_err(folder_error)
    }
}

fn js_error(e: ProjectError) -> JsError {
    JsError::new(&e.to_string())
}

fn generator_of(text: &str) -> Result<Generator, JsError> {
    serde_json::from_str(text).map_err(|e| JsError::new(&e.to_string()))
}

fn stored_json(stored: &Stored) -> String {
    json!({
        "reference": stored.reference,
        "sha256": stored.sha256,
        "bytes": stored.bytes,
    })
    .to_string()
}

/// A project in a host's store: the core's `Project`, with documents as their text and results
/// as JSON. Edit the document as an object and set it back; `save` validates and writes it.
#[wasm_bindgen]
pub struct Project {
    inner: tutti::Project<HostStore>,
    report: String,
    read_only: bool,
}

#[wasm_bindgen]
impl Project {
    /// Opens the project in a store: the one document, read and validated (`report` holds the
    /// findings, errors included); throws when there is no document, or two, or it can't be read.
    pub fn open(store: JsStore) -> Result<Project, JsError> {
        let opened = tutti::Project::open(HostStore::new(store)).map_err(js_error)?;
        Ok(Project {
            inner: opened.project,
            report: issues_json(&opened.report.issues),
            read_only: opened.read_only,
        })
    }

    /// Creates a project in a store with no document: the folders, an empty document named
    /// after the title, saved at once with its README. `generator` as JSON.
    pub fn create(store: JsStore, title: &str, generator: &str) -> Result<Project, JsError> {
        let generator = generator_of(generator)?;
        let inner =
            tutti::Project::create(HostStore::new(store), title, &generator).map_err(js_error)?;
        Ok(Project {
            inner,
            report: "[]".to_owned(),
            read_only: false,
        })
    }

    /// A project over a document the host built (its text), to be saved into the store under
    /// `documentName` (`<Name>.tutti`): nothing is read or written until `save`.
    pub fn adopt(store: JsStore, document_name: &str, text: &str) -> Result<Project, JsError> {
        let document = tutti::read(text.as_bytes()).map_err(|e| JsError::new(&e.to_string()))?;
        let inner = tutti::Project::adopt(HostStore::new(store), document_name, document)
            .map_err(js_error)?;
        Ok(Project {
            inner,
            report: "[]".to_owned(),
            read_only: false,
        })
    }

    /// Validation's findings at open, as `validate` gives them (JSON).
    pub fn report(&self) -> String {
        self.report.clone()
    }

    /// Whether the document needs a later minor version than this build writes: `save` refuses.
    #[wasm_bindgen(js_name = readOnly)]
    pub fn read_only(&self) -> bool {
        self.read_only
    }

    /// The document, in the pretty form.
    pub fn document(&self) -> String {
        tutti::write(self.inner.document())
    }

    /// Replaces the document with one read from `text` (any form); what it refers to must be in
    /// the store by the time of `save`.
    #[wasm_bindgen(js_name = setDocument)]
    pub fn set_document(&mut self, text: &str) -> Result<(), JsError> {
        let document = tutti::read(text.as_bytes()).map_err(|e| JsError::new(&e.to_string()))?;
        *self.inner.document_mut() = document;
        Ok(())
    }

    #[wasm_bindgen(js_name = documentName)]
    pub fn document_name(&self) -> String {
        self.inner.document_name().to_owned()
    }

    /// The SHA-256 of the document's bytes as last read or written; empty before either.
    #[wasm_bindgen(js_name = documentHash)]
    pub fn document_hash(&self) -> String {
        self.inner.document_hash().to_owned()
    }

    pub fn title(&self) -> String {
        self.inner.title()
    }

    /// Saves the project, `generator` as JSON: the save procedure of the spec. Gives
    /// `{warnings, readme_written, document_hash}` as JSON; throws with the refusal's reason.
    pub fn save(&mut self, generator: &str) -> Result<String, JsError> {
        let generator = generator_of(generator)?;
        let saved = self.inner.save(&generator).map_err(js_error)?;
        Ok(saved_json(&saved))
    }

    /// `save`, hashing every indexed file again: for an export.
    #[wasm_bindgen(js_name = saveVerified)]
    pub fn save_verified(&mut self, generator: &str) -> Result<String, JsError> {
        let generator = generator_of(generator)?;
        let saved = self.inner.save_verified(&generator).map_err(js_error)?;
        Ok(saved_json(&saved))
    }

    /// Rewrites the document as held, in the pretty form, and nothing else; gives its hash.
    pub fn rewrite(&mut self) -> Result<String, JsError> {
        self.inner.rewrite().map_err(js_error)
    }

    /// Validation and the files check together, as `validate` gives them (JSON).
    pub fn check(&self) -> String {
        issues_json(&self.inner.check().issues)
    }

    /// A file of the project, by reference.
    pub fn read(&self, reference: &str) -> Result<Vec<u8>, JsError> {
        self.inner.read(reference).map_err(js_error)
    }

    /// Puts audio in `audio/` whole, the same content once: `{reference, sha256, bytes}`.
    #[wasm_bindgen(js_name = putAudio)]
    pub fn put_audio(&self, name: &str, extension: &str, bytes: &[u8]) -> Result<String, JsError> {
        let stored = self
            .inner
            .put_audio(name, extension, bytes)
            .map_err(js_error)?;
        Ok(stored_json(&stored))
    }

    /// Puts a render in `renders/` whole: `{reference, sha256, bytes}`.
    #[wasm_bindgen(js_name = putRender)]
    pub fn put_render(&self, name: &str, extension: &str, bytes: &[u8]) -> Result<String, JsError> {
        let stored = self
            .inner
            .put_render(name, extension, bytes)
            .map_err(js_error)?;
        Ok(stored_json(&stored))
    }

    /// Puts a plugin state in `plugins/states/`, named by its content: `{sha256, kind, path}`.
    #[wasm_bindgen(js_name = putState)]
    pub fn put_state(&self, kind: &str, bytes: &[u8]) -> Result<String, JsError> {
        let state = self.inner.put_state(kind, bytes).map_err(js_error)?;
        Ok(json!({"sha256": state.sha256, "kind": state.kind, "path": state.path()}).to_string())
    }

    /// The file in `audio/` with exactly these bytes, if any: `{reference, sha256, bytes}` or
    /// `undefined`.
    #[wasm_bindgen(js_name = findAudio)]
    pub fn find_audio(&self, bytes: &[u8]) -> Result<JsValue, JsError> {
        Ok(match self.inner.find_audio(bytes).map_err(js_error)? {
            Some(stored) => JsValue::from_str(&stored_json(&stored)),
            None => JsValue::UNDEFINED,
        })
    }

    /// A new audio file to stream into `audio/`: write to the handle, then `finish`.
    #[wasm_bindgen(js_name = newAudio)]
    pub fn new_audio(&self, name: &str, extension: &str) -> Result<Incoming, JsError> {
        let inner = self.inner.new_audio(name, extension).map_err(js_error)?;
        Ok(Incoming { inner: Some(inner) })
    }

    /// A new file to stream into `renders/`, as `newAudio`.
    #[wasm_bindgen(js_name = newRender)]
    pub fn new_render(&self, name: &str, extension: &str) -> Result<Incoming, JsError> {
        let inner = self.inner.new_render(name, extension).map_err(js_error)?;
        Ok(Incoming { inner: Some(inner) })
    }

    /// Drops an index entry no known field claims whose file is gone, on purpose.
    #[wasm_bindgen(js_name = forgetFile)]
    pub fn forget_file(&mut self, path: &str) {
        self.inner.forget_file(path);
    }

    /// Every file verified so far, as JSON `{reference: {sha256, bytes, modified}}`: for the host
    /// to keep between sessions and give back with `trust`.
    pub fn verified(&self) -> String {
        let records: BTreeMap<String, Verified> = self.inner.verified().into_iter().collect();
        serde_json::to_string(&records).unwrap_or_else(|_| "{}".to_owned())
    }

    /// Takes records the host kept (the JSON `verified` gave).
    pub fn trust(&self, records: &str) -> Result<(), JsError> {
        let records: BTreeMap<String, Verified> =
            serde_json::from_str(records).map_err(|e| JsError::new(&e.to_string()))?;
        self.inner.trust(records);
        Ok(())
    }

    /// How many files are being written or stored and not yet named by the document.
    pub fn unnamed(&self) -> usize {
        self.inner.unnamed()
    }

    /// Copies the project into another store: every indexed file, then the document and its
    /// README through a save.
    #[wasm_bindgen(js_name = copyInto)]
    pub fn copy_into(&self, store: JsStore) -> Result<Project, JsError> {
        let inner = self
            .inner
            .copy_into(HostStore::new(store))
            .map_err(js_error)?;
        Ok(Project {
            inner,
            report: "[]".to_owned(),
            read_only: false,
        })
    }
}

fn saved_json(saved: &tutti::Saved) -> String {
    let warnings: Value =
        serde_json::from_str(&issues_json(&saved.warnings)).unwrap_or(Value::Null);
    json!({
        "warnings": warnings,
        "readme_written": saved.readme_written,
        "document_hash": saved.document_hash,
    })
    .to_string()
}

/// A new file being written into the project: bytes stream through `write`, hashed as they go;
/// `finish` gives `{reference, sha256, bytes}`. Dropped unfinished, the file goes.
#[wasm_bindgen]
pub struct Incoming {
    inner: Option<tutti::Incoming<HostStore>>,
}

#[wasm_bindgen]
impl Incoming {
    /// The reference the file will have.
    pub fn reference(&self) -> String {
        self.inner
            .as_ref()
            .map(|i| i.reference().to_owned())
            .unwrap_or_default()
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), JsError> {
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| JsError::new("finished already"))?;
        inner
            .write_all(bytes)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Closes the file and gives its facts; the handle is spent.
    pub fn finish(&mut self) -> Result<String, JsError> {
        let inner = self
            .inner
            .take()
            .ok_or_else(|| JsError::new("finished already"))?;
        let stored = inner.finish().map_err(js_error)?;
        Ok(stored_json(&stored))
    }
}
