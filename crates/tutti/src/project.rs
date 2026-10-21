//! A project in a store: open, create, save, and the files a program puts in. The procedure
//! every writer follows (spec 2, 3.4, 10; conformance 2), written once over the [`Store`] trait,
//! so the disk (`tutti-fs`), a test's memory and a JavaScript host all run the same one and no
//! program writes file-system code against the spec.
//!
//! What [`Project::save`] guarantees, exactly: every file the document refers to is there; the
//! files index names each of them with its length as read at this save and its SHA-256 as
//! verified when the file was last seen to change: a file's length and modification time are
//! read at every save and compared with the record of its last verification, and a file
//! whose either differs is hashed again (a file the crate wrote itself is known; a store
//! without modification times is trusted by length within the session; a program keeps the
//! records between sessions with [`Project::verified`] and [`Project::trust`]);
//! an entry of the index that no known field claims stays, verified the same way, as long as
//! the document mentions the file anywhere else (its path, its name or its hash: an unknown
//! extension's sidecar), and goes when nothing does; a disagreement refuses the save by
//! reference before the document is touched; the document is valid; `README.md` is current;
//! the document is written to a temporary name and renamed over the old one, so a crash
//! leaves the old document or the new, never a torn one, and a failure after that rename is
//! reported as [`ProjectError::Unconfirmed`], the save being on disk. A file changed behind
//! the project's back at the same length with its modification time put back, which is
//! forgery and a breach of the folder's rules (spec 1), is not seen by `save`: `check` finds
//! it, and a reader plays silence and warns (spec 9), which is what the hash rule is for. [`Project::save_verified`] rehashes everything and refuses a mismatch; it is
//! what an exporting program's guarantee (conformance 4) rests on.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::io::{self, Write};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::folder::{FolderError, Kind, Store, check_files};
use crate::model::{Document, Event, FileEntry, Generator, StateRef, Unknown, version_parts};
use crate::names::{clean_name, free_name};
use crate::read::{ReadError, is_read_only, read};
use crate::render::sha256_hex;
use crate::validate::{Issue, Report, check_device_file_path, device_file, is_file_name, validate};

/// The document's extension, without the dot.
pub const DOCUMENT_EXTENSION: &str = "tutti";
const AUDIO: &str = "audio";
const STATES: &str = "plugins/states";
const RENDERS: &str = "renders";
const README: &str = "README.md";
const TEMPORARY: &str = ".tmp";

/// Why a project could not be opened, created or saved.
#[derive(Debug)]
pub enum ProjectError {
    /// The folder holds no `.tutti` document.
    NoDocument,
    /// The folder holds more than one `.tutti` document, named here (spec 3.1).
    SeveralDocuments(Vec<String>),
    /// A document is there already where a project was to be created.
    DocumentExists(String),
    /// The document's bytes are not a document this crate opens.
    Read(ReadError),
    /// The document would be invalid: the errors, each at its JSON pointer.
    Invalid(Vec<Issue>),
    /// The document needs a later minor version than this crate writes (spec 8.3).
    ReadOnly,
    /// A file the document refers to, or the index lists, is not in the folder.
    MissingFile(String),
    /// A file is not what the document says it is: its hash or its length.
    Mismatch {
        reference: String,
        expected: String,
        found: String,
    },
    /// A name or an extension a writer may not give (spec 2.1).
    Name(String),
    /// The document was committed (renamed into place) and a step after that failed, the
    /// folder's flush: the save is on disk, and [`Project::document_hash`] is the new one.
    Unconfirmed(FolderError),
    /// The store refused an operation.
    Store {
        reference: String,
        error: FolderError,
    },
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectError::NoDocument => write!(f, "no .tutti document"),
            ProjectError::SeveralDocuments(names) => {
                write!(f, "{} documents ({})", names.len(), names.join(", "))
            }
            ProjectError::DocumentExists(name) => write!(f, "{name} is there already"),
            ProjectError::Read(e) => write!(f, "{e}"),
            ProjectError::Invalid(issues) => {
                write!(f, "the document would be invalid:")?;
                for issue in issues {
                    write!(f, " {issue};")?;
                }
                Ok(())
            }
            ProjectError::ReadOnly => write!(
                f,
                "read-only on this version: the document's min_version is later"
            ),
            ProjectError::MissingFile(r) => write!(f, "{r}: missing"),
            ProjectError::Mismatch {
                reference,
                expected,
                found,
            } => write!(f, "{reference}: expected {expected}, found {found}"),
            ProjectError::Name(name) => write!(f, "{name:?}: not a name a writer may give"),
            ProjectError::Unconfirmed(error) => write!(
                f,
                "the document was written, and flushing the folder failed: {error}"
            ),
            ProjectError::Store { reference, error } if reference.is_empty() => {
                write!(f, "the folder: {error}")
            }
            ProjectError::Store { reference, error } => write!(f, "{reference}: {error}"),
        }
    }
}

impl std::error::Error for ProjectError {}

impl From<ReadError> for ProjectError {
    fn from(e: ReadError) -> Self {
        ProjectError::Read(e)
    }
}

fn store_error(reference: &str) -> impl FnOnce(FolderError) -> ProjectError + '_ {
    move |error| ProjectError::Store {
        reference: reference.to_owned(),
        error,
    }
}

/// A file as last verified: its hash, and the length and modification time it had then,
/// which a save compares with the file's to trust the hash. A program may keep these between
/// sessions ([`Project::verified`], [`Project::trust`]) in a place of its own, never in the
/// document: a modification time is this file system's, and churns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verified {
    pub sha256: String,
    pub bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified: Option<u64>,
}

/// What a project and its handles share: the files verified (reference to [`Verified`]), and
/// the references reserved by a file being written or not yet named by the document.
#[derive(Debug, Default)]
struct Shared {
    verified: HashMap<String, Verified>,
    reserved: HashSet<String>,
}

type SharedRef = Arc<Mutex<Shared>>;

fn lock(shared: &SharedRef) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Records a file as verified, with its modification time as the store tells it now.
fn remember<S: Store>(store: &S, shared: &SharedRef, reference: &str, sha256: &str, bytes: u64) {
    let modified = store.modified(reference).ok().flatten();
    lock(shared).verified.insert(
        reference.to_owned(),
        Verified {
            sha256: sha256.to_owned(),
            bytes,
            modified,
        },
    );
}

/// The verified hash of a file whose length and modification time are still the record's.
fn trusted<S: Store>(store: &S, shared: &SharedRef, reference: &str, size: u64) -> Option<String> {
    let record = lock(shared).verified.get(reference).cloned()?;
    if record.bytes != size {
        return None;
    }
    let modified = store.modified(reference).ok().flatten();
    (record.modified == modified).then_some(record.sha256)
}

/// A project opened from a store.
#[derive(Debug)]
pub struct Opened<S: Store> {
    pub project: Project<S>,
    /// Every finding of validation, errors included: a program tells its user at once.
    pub report: Report,
    /// Whether the document needs a later minor version than this crate writes: it plays, and
    /// `save` refuses (spec 8.3).
    pub read_only: bool,
}

/// What a save did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    /// Validation's warnings (there were no errors, or the save was refused).
    pub warnings: Vec<Issue>,
    /// Whether `README.md` was written (it is, when it differs from the template's text).
    pub readme_written: bool,
    /// The SHA-256 of the document's bytes as written.
    pub document_hash: String,
}

/// A file put in the project, with the guard that keeps it reserved while the program's
/// document does not name it yet: a recording may finish on another thread before the command
/// that names it runs, and a program's cleanup consults [`Project::unnamed`] before touching
/// `audio/`.
#[derive(Debug)]
pub struct Stored {
    pub reference: String,
    pub sha256: String,
    pub bytes: u64,
    _guard: Guard,
}

#[derive(Debug)]
struct Guard {
    reference: String,
    shared: SharedRef,
}

impl Drop for Guard {
    fn drop(&mut self) {
        lock(&self.shared).reserved.remove(&self.reference);
    }
}

/// A new file being written into the project through a [`Write`]: the bytes stream through,
/// hashed and counted, into the store's own writer (a file on disk, a buffer in memory).
/// [`Incoming::finish`] makes it a [`Stored`]; dropped before that, the file goes.
#[derive(Debug)]
pub struct Incoming<S: Store> {
    store: S,
    reference: String,
    writer: Option<S::Writer>,
    hasher: Sha256,
    bytes: u64,
    shared: SharedRef,
    finished: bool,
}

impl<S: Store> Incoming<S> {
    /// The reference the file will have, `audio/<name>.<extension>`.
    pub fn reference(&self) -> &str {
        &self.reference
    }

    /// Bytes written so far.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Closes the file, durably, and gives its facts.
    pub fn finish(mut self) -> Result<Stored, ProjectError> {
        let mut writer = self.writer.take().expect("not finished");
        writer.flush().map_err(|e| ProjectError::Store {
            reference: self.reference.clone(),
            error: FolderError::Other(e.to_string()),
        })?;
        self.store
            .close(writer)
            .map_err(store_error(&self.reference))?;
        let sha256 = format!("{:x}", std::mem::take(&mut self.hasher).finalize());
        self.finished = true;
        remember(
            &self.store,
            &self.shared,
            &self.reference,
            &sha256,
            self.bytes,
        );
        Ok(Stored {
            reference: self.reference.clone(),
            sha256,
            bytes: self.bytes,
            _guard: Guard {
                reference: self.reference.clone(),
                shared: self.shared.clone(),
            },
        })
    }
}

impl<S: Store> Write for Incoming<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let writer = self.writer.as_mut().expect("not finished");
        let n = writer.write(buf)?;
        self.hasher.update(&buf[..n]);
        self.bytes += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.as_mut().expect("not finished").flush()
    }
}

impl<S: Store> Drop for Incoming<S> {
    fn drop(&mut self) {
        if !self.finished {
            drop(self.writer.take());
            let _ = self.store.remove(&self.reference);
            lock(&self.shared).reserved.remove(&self.reference);
        }
    }
}

/// The one `.tutti` document at the top of a store, by name (spec 3.1).
pub fn document_in<S: Store>(store: &S) -> Result<String, ProjectError> {
    let entries = store.list("").map_err(store_error(""))?;
    let suffix = format!(".{DOCUMENT_EXTENSION}");
    let mut found: Vec<String> = entries
        .into_iter()
        .filter(|e| e.kind == Kind::File && e.name.ends_with(&suffix))
        .map(|e| e.name)
        .collect();
    found.sort();
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(ProjectError::NoDocument),
        _ => Err(ProjectError::SeveralDocuments(found)),
    }
}

/// A project: its document and the store it lives in.
#[derive(Debug)]
pub struct Project<S: Store> {
    store: S,
    document_name: String,
    document: Document,
    document_hash: String,
    shared: SharedRef,
}

impl<S: Store> Project<S> {
    /// Opens the project in a store: the one document, read and validated.
    pub fn open(store: S) -> Result<Opened<S>, ProjectError> {
        let document_name = document_in(&store)?;
        let bytes = store
            .read(&document_name)
            .map_err(store_error(&document_name))?;
        let document = read(&bytes)?;
        let report = validate(&document);
        let read_only = is_read_only(&document);
        Ok(Opened {
            project: Project {
                store,
                document_name,
                document_hash: sha256_hex(&bytes),
                document,
                shared: SharedRef::default(),
            },
            report,
            read_only,
        })
    }

    /// Creates a project in a store with no document: the folders, an empty document named
    /// after the title, saved at once with its README.
    pub fn create(store: S, title: &str, generator: &Generator) -> Result<Self, ProjectError> {
        store.make_folder("").map_err(store_error(""))?;
        match document_in(&store) {
            Ok(name) => return Err(ProjectError::DocumentExists(name)),
            Err(ProjectError::SeveralDocuments(names)) => {
                return Err(ProjectError::DocumentExists(names.join(", ")));
            }
            Err(ProjectError::NoDocument) => {}
            Err(e) => return Err(e),
        }
        for folder in [AUDIO, STATES] {
            store.make_folder(folder).map_err(store_error(folder))?;
        }
        let mut document = Document::new(generator.clone());
        document.metadata.title = Some(title.to_owned());
        let document_name = format!("{}.{DOCUMENT_EXTENSION}", clean_name(title, "Project"));
        let mut project = Self::adopt(store, &document_name, document)?;
        project.save(generator)?;
        Ok(project)
    }

    /// A project over a document a program built, to be saved into `store` under
    /// `document_name` (`<Name>.tutti`): nothing is read or written until [`Project::save`].
    pub fn adopt(store: S, document_name: &str, document: Document) -> Result<Self, ProjectError> {
        let suffix = format!(".{DOCUMENT_EXTENSION}");
        if !document_name.ends_with(&suffix) || !is_file_name(document_name) {
            return Err(ProjectError::Name(document_name.to_owned()));
        }
        Ok(Project {
            store,
            document_name: document_name.to_owned(),
            document,
            document_hash: String::new(),
            shared: SharedRef::default(),
        })
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    /// `<Name>.tutti`.
    pub fn document_name(&self) -> &str {
        &self.document_name
    }

    /// The SHA-256 of the document's bytes as last read or written; empty before either.
    pub fn document_hash(&self) -> &str {
        &self.document_hash
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    /// The project's title: `metadata.title`, else the document's name without its extension.
    pub fn title(&self) -> String {
        match &self.document.metadata.title {
            Some(t) if !t.is_empty() => t.clone(),
            _ => self
                .document_name
                .strip_suffix(&format!(".{DOCUMENT_EXTENSION}"))
                .unwrap_or(&self.document_name)
                .to_owned(),
        }
    }

    /// How many files are reserved: being written, or stored and not yet named by a command.
    pub fn unnamed(&self) -> usize {
        lock(&self.shared).reserved.len()
    }

    /// Every file verified so far, by reference: for a program to keep between sessions, in a
    /// place of its own, and give back with [`Project::trust`].
    pub fn verified(&self) -> Vec<(String, Verified)> {
        let mut out: Vec<(String, Verified)> = lock(&self.shared)
            .verified
            .iter()
            .map(|(r, v)| (r.clone(), v.clone()))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// Takes records a program kept: a file whose length and modification time are still the
    /// record's is trusted at the next save without being hashed again.
    pub fn trust(&self, records: impl IntoIterator<Item = (String, Verified)>) {
        let mut shared = lock(&self.shared);
        for (reference, record) in records {
            shared.verified.insert(reference, record);
        }
    }

    /// Validation and the files check together (conformance 1).
    pub fn check(&self) -> Report {
        let mut report = validate(&self.document);
        report
            .issues
            .extend(check_files(&self.document, &self.store));
        report
    }

    /// A file of the project, by reference.
    pub fn read(&self, reference: &str) -> Result<Vec<u8>, ProjectError> {
        self.store.read(reference).map_err(store_error(reference))
    }

    /// Drops an index entry no known field claims but the document mentions (an unknown
    /// extension's file that is gone): a save refuses such an entry until the program, having
    /// asked its user, forgets it.
    pub fn forget_file(&mut self, path: &str) {
        self.document.files.retain(|f| f.path != path);
        lock(&self.shared).verified.remove(path);
    }

    /// Saves the project: the procedure in this module's doc, with `generator` as the last
    /// writer (spec 3.2).
    pub fn save(&mut self, generator: &Generator) -> Result<Saved, ProjectError> {
        self.save_with(generator, false)
    }

    /// [`Project::save`], rehashing every indexed file whatever this session verified before:
    /// for an export, or a "save and verify".
    pub fn save_verified(&mut self, generator: &Generator) -> Result<Saved, ProjectError> {
        self.save_with(generator, true)
    }

    fn save_with(&mut self, generator: &Generator, rehash: bool) -> Result<Saved, ProjectError> {
        if is_read_only(&self.document) {
            return Err(ProjectError::ReadOnly);
        }
        self.document.generator = generator.clone();
        self.bring_manifest();
        self.document.files = self.index(rehash)?;
        let mut used: BTreeSet<String> = self.document.extensions_used.iter().cloned().collect();
        used.extend(used_namespaces(&self.document));
        self.document.extensions_used = used.into_iter().collect();
        self.document.normalise();
        let report = validate(&self.document);
        if !report.is_valid() {
            return Err(ProjectError::Invalid(report.errors().cloned().collect()));
        }
        let readme_written = self.write_readme()?;
        let document_hash = self.write_document()?;
        Ok(Saved {
            warnings: report.warnings().cloned().collect(),
            readme_written,
            document_hash,
        })
    }

    /// Rewrites the document as held, in the pretty form, and nothing else: no index, no
    /// validation, no README (the `fmt` command).
    pub fn rewrite(&mut self) -> Result<String, ProjectError> {
        self.document.normalise();
        self.write_document()
    }

    /// Copies the project into another store: every indexed file, then the document and its
    /// README through a save, so the copy is complete and verified. The program's own private
    /// paths are its own to copy.
    pub fn copy_into<T: Store>(&self, store: T) -> Result<Project<T>, ProjectError> {
        store.make_folder("").map_err(store_error(""))?;
        if let Ok(name) = document_in(&store) {
            return Err(ProjectError::DocumentExists(name));
        }
        for folder in [AUDIO, STATES] {
            store.make_folder(folder).map_err(store_error(folder))?;
        }
        for entry in &self.document.files {
            let bytes = self.read(&entry.path)?;
            store
                .write_new(&entry.path, &bytes)
                .map_err(store_error(&entry.path))?;
        }
        let mut copy = Project::adopt(store, &self.document_name, self.document.clone())?;
        copy.document_hash = String::new();
        let generator = self.document.generator.clone();
        copy.save(&generator)?;
        Ok(copy)
    }

    /// A device's own files, `plugins/files/<device id>/` (spec 7.8): put, list and read them by
    /// their path inside the folder. They are indexed by the device's id at the next save, and
    /// go from the index with the device; the folder itself is the program's to remove.
    pub fn device_files(&self, device: &str) -> Result<DeviceFiles<S>, ProjectError> {
        crate::id::check(device).map_err(|_| ProjectError::Name(device.to_owned()))?;
        Ok(DeviceFiles {
            store: self.store.clone(),
            prefix: format!("plugins/files/{device}"),
            shared: self.shared.clone(),
        })
    }

    /// Copies one device's own files to another's folder, for a duplicated device (spec 7.8):
    /// how many files were copied.
    pub fn copy_device_files(&self, from: &str, to: &str) -> Result<usize, ProjectError> {
        let source = self.device_files(from)?;
        let target = self.device_files(to)?;
        let mut copied = 0;
        for relative in source.list()? {
            let bytes = source.read(&relative)?;
            target.put(&relative, &bytes)?;
            copied += 1;
        }
        Ok(copied)
    }

    /// A new audio file to stream into `audio/`: `name` cleaned to the characters a name may
    /// have and made free, `extension` the file's kind (`wav`). The handle is independent of
    /// the project, so a recorder writes on its own thread.
    pub fn new_audio(&self, name: &str, extension: &str) -> Result<Incoming<S>, ProjectError> {
        self.new_file(AUDIO, name, extension)
    }

    /// A new file to stream into `renders/`, as [`Project::new_audio`].
    pub fn new_render(&self, name: &str, extension: &str) -> Result<Incoming<S>, ProjectError> {
        self.new_file(RENDERS, name, extension)
    }

    fn new_file(
        &self,
        folder: &str,
        name: &str,
        extension: &str,
    ) -> Result<Incoming<S>, ProjectError> {
        if !is_file_name(&format!("x.{extension}")) {
            return Err(ProjectError::Name(format!(".{extension}")));
        }
        self.store
            .make_folder(folder)
            .map_err(store_error(folder))?;
        let stem = clean_name(name, "Untitled");
        let reference = {
            let mut shared = lock(&self.shared);
            let mut taken: Vec<String> = self
                .store
                .list(folder)
                .map_err(store_error(folder))?
                .into_iter()
                .map(|e| e.name)
                .collect();
            let prefix = format!("{folder}/");
            taken.extend(
                shared
                    .reserved
                    .iter()
                    .filter_map(|r| r.strip_prefix(&prefix).map(str::to_owned)),
            );
            let file = free_name(taken.iter().map(String::as_str), &stem, extension);
            let reference = format!("{prefix}{file}");
            shared.reserved.insert(reference.clone());
            reference
        };
        let writer = match self.store.open_new(&reference) {
            Ok(w) => w,
            Err(e) => {
                lock(&self.shared).reserved.remove(&reference);
                return Err(store_error(&reference)(e));
            }
        };
        Ok(Incoming {
            store: self.store.clone(),
            reference,
            writer: Some(writer),
            hasher: Sha256::new(),
            bytes: 0,
            shared: self.shared.clone(),
            finished: false,
        })
    }

    /// The file in `audio/` with exactly these bytes, if any, reserved: what [`Project::put_audio`]
    /// uses again rather than writing a copy; for a program that streams a recording in itself
    /// and keeps the same content once.
    pub fn find_audio(&self, bytes: &[u8]) -> Result<Option<Stored>, ProjectError> {
        self.same_content(AUDIO, bytes)
    }

    /// Puts audio in `audio/` whole. The same content is stored once: a file of it already
    /// there is used again, under its own name.
    pub fn put_audio(
        &self,
        name: &str,
        extension: &str,
        bytes: &[u8],
    ) -> Result<Stored, ProjectError> {
        if let Some(stored) = self.find_audio(bytes)? {
            return Ok(stored);
        }
        let mut incoming = self.new_audio(name, extension)?;
        incoming.write_all(bytes).map_err(|e| ProjectError::Store {
            reference: incoming.reference().to_owned(),
            error: FolderError::Other(e.to_string()),
        })?;
        incoming.finish()
    }

    /// Puts a render in `renders/` whole.
    pub fn put_render(
        &self,
        name: &str,
        extension: &str,
        bytes: &[u8],
    ) -> Result<Stored, ProjectError> {
        let mut incoming = self.new_render(name, extension)?;
        incoming.write_all(bytes).map_err(|e| ProjectError::Store {
            reference: incoming.reference().to_owned(),
            error: FolderError::Other(e.to_string()),
        })?;
        incoming.finish()
    }

    /// Puts a plugin state in `plugins/states/`, named by its content (spec 7.4); one that is
    /// there already is left as it is.
    pub fn put_state(&self, kind: &str, bytes: &[u8]) -> Result<StateRef, ProjectError> {
        if !is_file_name(&format!("x.{kind}")) {
            return Err(ProjectError::Name(format!(".{kind}")));
        }
        let state = StateRef {
            sha256: sha256_hex(bytes),
            kind: kind.to_owned(),
            unknown: Unknown::new(),
        };
        let reference = state.path();
        self.store
            .make_folder(STATES)
            .map_err(store_error(STATES))?;
        match self.store.write_new(&reference, bytes) {
            Ok(()) | Err(FolderError::Exists) => {}
            Err(e) => return Err(store_error(&reference)(e)),
        }
        remember(
            &self.store,
            &self.shared,
            &reference,
            &state.sha256,
            bytes.len() as u64,
        );
        Ok(state)
    }

    /// A file in `folder` with these bytes, if any, reserved.
    fn same_content(&self, folder: &str, bytes: &[u8]) -> Result<Option<Stored>, ProjectError> {
        let entries = match self.store.list(folder) {
            Ok(entries) => entries,
            Err(FolderError::Missing) => return Ok(None),
            Err(e) => return Err(store_error(folder)(e)),
        };
        let length = bytes.len() as u64;
        let sha256 = sha256_hex(bytes);
        for entry in entries.into_iter().filter(|e| e.kind == Kind::File) {
            let reference = format!("{folder}/{}", entry.name);
            if self
                .store
                .size(&reference)
                .map_err(store_error(&reference))?
                != length
            {
                continue;
            }
            let hash = match trusted(&self.store, &self.shared, &reference, length) {
                Some(hash) => hash,
                None => {
                    self.store
                        .sha256(&reference)
                        .map_err(store_error(&reference))?
                        .0
                }
            };
            if hash == sha256 {
                remember(&self.store, &self.shared, &reference, &hash, length);
                lock(&self.shared).reserved.insert(reference.clone());
                return Ok(Some(Stored {
                    reference: reference.clone(),
                    sha256: hash,
                    bytes: length,
                    _guard: Guard {
                        reference,
                        shared: self.shared.clone(),
                    },
                }));
            }
        }
        Ok(None)
    }

    /// The manifest's own keys: the format, this crate's version unless the document's is a
    /// later minor of the same major (kept, with its own `about`, `spec` and `schema`), and the
    /// three texts when they are missing.
    fn bring_manifest(&mut self) {
        let doc = &mut self.document;
        doc.format = crate::FORMAT.to_owned();
        let own = version_parts(crate::VERSION).expect("the crate's version");
        let later = version_parts(&doc.version).is_some_and(|v| v.0 == own.0 && v.1 > own.1);
        if !later {
            doc.version = crate::VERSION.to_owned();
            doc.about = Some(crate::ABOUT.to_owned());
            doc.spec = Some(crate::SPEC_URL.to_owned());
            doc.schema = Some(crate::SCHEMA_URL.to_owned());
        } else {
            doc.about.get_or_insert_with(|| crate::ABOUT.to_owned());
            doc.spec.get_or_insert_with(|| crate::SPEC_URL.to_owned());
            doc.schema
                .get_or_insert_with(|| crate::SCHEMA_URL.to_owned());
        }
    }

    /// The files index: what the known references name, each verified, then the entries the
    /// index held that no known field claims but the document mentions (a device's own file is
    /// claimed by its device, spec 7.8), verified as they stand; one nothing mentions goes.
    fn index(&self, rehash: bool) -> Result<Vec<FileEntry>, ProjectError> {
        let mentions = mentions(&self.document);
        let devices: HashSet<String> = self
            .document
            .master
            .devices
            .iter()
            .chain(
                self.document
                    .all_tracks()
                    .into_iter()
                    .flat_map(|t| t.devices.iter()),
            )
            .map(|d| d.id.as_str().to_owned())
            .collect();
        let existing: HashMap<&str, &FileEntry> = self
            .document
            .files
            .iter()
            .map(|f| (f.path.as_str(), f))
            .collect();
        let mut entries: Vec<FileEntry> = Vec::new();
        let mut done: HashSet<String> = HashSet::new();
        for (reference, claimed) in claims(&self.document) {
            let was = existing.get(reference.as_str()).copied();
            let (sha256, bytes) =
                self.verify(&reference, &claimed, was.map(|f| f.bytes), rehash)?;
            entries.push(FileEntry {
                path: reference.clone(),
                sha256,
                bytes,
                unknown: was.map(|f| f.unknown.clone()).unwrap_or_default(),
            });
            done.insert(reference);
        }
        for entry in &self.document.files {
            if done.contains(&entry.path) {
                continue;
            }
            let claimed = match device_file(&entry.path) {
                Some((device, _)) => devices.contains(device),
                None => mentioned(&mentions, entry),
            };
            if !claimed {
                continue;
            }
            let (sha256, bytes) =
                self.verify(&entry.path, &entry.sha256, Some(entry.bytes), rehash)?;
            entries.push(FileEntry {
                path: entry.path.clone(),
                sha256,
                bytes,
                unknown: entry.unknown.clone(),
            });
        }
        Ok(entries)
    }

    /// A referenced file, present with the expected hash (and length, when the index had one):
    /// its hash and length.
    fn verify(
        &self,
        reference: &str,
        expected: &str,
        length: Option<u64>,
        rehash: bool,
    ) -> Result<(String, u64), ProjectError> {
        match self.store.exists(reference) {
            Ok(true) => {}
            Ok(false) => return Err(ProjectError::MissingFile(reference.to_owned())),
            Err(FolderError::Missing) => {
                return Err(ProjectError::MissingFile(reference.to_owned()));
            }
            Err(e) => return Err(store_error(reference)(e)),
        }
        let size = self.store.size(reference).map_err(store_error(reference))?;
        if let Some(length) = length {
            if length != size {
                return Err(ProjectError::Mismatch {
                    reference: reference.to_owned(),
                    expected: format!("{length} bytes"),
                    found: format!("{size} bytes"),
                });
            }
        }
        let known = if rehash {
            None
        } else {
            trusted(&self.store, &self.shared, reference, size)
        };
        let sha256 = match known {
            Some(hash) => hash,
            None => {
                self.store
                    .sha256(reference)
                    .map_err(store_error(reference))?
                    .0
            }
        };
        if sha256 != expected {
            return Err(ProjectError::Mismatch {
                reference: reference.to_owned(),
                expected: expected.to_owned(),
                found: sha256,
            });
        }
        remember(&self.store, &self.shared, reference, &sha256, size);
        Ok((sha256, size))
    }

    /// `README.md` from the template, written when it differs from what is there.
    fn write_readme(&self) -> Result<bool, ProjectError> {
        let text = crate::readme(&self.title(), &self.document.generator, &self.document_name);
        match self.store.read(README) {
            Ok(bytes) if bytes == text.as_bytes() => return Ok(false),
            Ok(_) | Err(FolderError::Missing) => {}
            Err(e) => return Err(store_error(README)(e)),
        }
        self.replace(README, text.as_bytes())?;
        Ok(true)
    }

    /// The document in the pretty form, to a temporary name renamed over the old one: the
    /// commit point, after which the hash is the new one and a failure is `Unconfirmed`.
    fn write_document(&mut self) -> Result<String, ProjectError> {
        let text = crate::write(&self.document);
        self.replace(&self.document_name, text.as_bytes())?;
        self.document_hash = sha256_hex(text.as_bytes());
        self.store
            .sync_folder("")
            .map_err(ProjectError::Unconfirmed)?;
        Ok(self.document_hash.clone())
    }

    /// Writes `bytes` to `<reference>.tmp`, then renames it over `reference`: the old file or
    /// the new one is there at every moment.
    fn replace(&self, reference: &str, bytes: &[u8]) -> Result<(), ProjectError> {
        let temporary = format!("{reference}{TEMPORARY}");
        match self.store.remove(&temporary) {
            Ok(()) | Err(FolderError::Missing) => {}
            Err(e) => return Err(store_error(&temporary)(e)),
        }
        self.store
            .write_new(&temporary, bytes)
            .map_err(store_error(&temporary))?;
        self.store
            .rename_over(&temporary, reference)
            .map_err(store_error(reference))
    }
}

/// A device's own files (spec 7.8), by their path inside `plugins/files/<device id>/`.
#[derive(Debug)]
pub struct DeviceFiles<S: Store> {
    store: S,
    prefix: String,
    shared: SharedRef,
}

impl<S: Store> DeviceFiles<S> {
    /// The reference of a file inside the folder.
    pub fn reference(&self, relative: &str) -> String {
        format!("{}/{relative}", self.prefix)
    }

    /// Puts a file whole, never replacing one: `relative` a path inside the folder, checked
    /// for safety (refused) and portability (allowed). Its index entry, for the next save.
    pub fn put(&self, relative: &str, bytes: &[u8]) -> Result<FileEntry, ProjectError> {
        check_device_file_path(relative)
            .map_err(|reason| ProjectError::Name(format!("{relative}: {reason}")))?;
        let reference = self.reference(relative);
        self.store
            .write_new(&reference, bytes)
            .map_err(store_error(&reference))?;
        let sha256 = sha256_hex(bytes);
        remember(
            &self.store,
            &self.shared,
            &reference,
            &sha256,
            bytes.len() as u64,
        );
        Ok(FileEntry {
            path: reference,
            sha256,
            bytes: bytes.len() as u64,
            unknown: Unknown::new(),
        })
    }

    /// Every file in the folder, by its path inside it, folders walked; none when the folder
    /// isn't there.
    pub fn list(&self) -> Result<Vec<String>, ProjectError> {
        let mut out = Vec::new();
        self.walk("", &mut out)?;
        out.sort();
        Ok(out)
    }

    fn walk(&self, inside: &str, out: &mut Vec<String>) -> Result<(), ProjectError> {
        let folder = if inside.is_empty() {
            self.prefix.clone()
        } else {
            format!("{}/{inside}", self.prefix)
        };
        let entries = match self.store.list(&folder) {
            Ok(entries) => entries,
            Err(FolderError::Missing) => return Ok(()),
            Err(e) => return Err(store_error(&folder)(e)),
        };
        for entry in entries {
            let relative = if inside.is_empty() {
                entry.name
            } else {
                format!("{inside}/{}", entry.name)
            };
            match entry.kind {
                Kind::File => out.push(relative),
                Kind::Folder => self.walk(&relative, out)?,
                Kind::Other => {}
            }
        }
        Ok(())
    }

    /// A file of the folder, whole.
    pub fn read(&self, relative: &str) -> Result<Vec<u8>, ProjectError> {
        let reference = self.reference(relative);
        self.store.read(&reference).map_err(store_error(&reference))
    }
}

/// Every file a known field refers to, with the hash it claims, in order of first use and each
/// once: devices' states, audio pieces' files, renders.
fn claims(doc: &Document) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut push = |reference: String, sha256: String| {
        if !out.iter().any(|(r, _)| *r == reference) {
            out.push((reference, sha256));
        }
    };
    for d in &doc.master.devices {
        if let Some(s) = &d.state {
            push(s.path(), s.sha256.clone());
        }
    }
    for t in doc.all_tracks() {
        for d in &t.devices {
            if let Some(s) = &d.state {
                push(s.path(), s.sha256.clone());
            }
        }
        for p in &t.patterns {
            for e in &p.events {
                if let Event::Note(n) = e {
                    if let Some(a) = &n.audio {
                        push(a.file.clone(), a.sha256.clone());
                    }
                }
            }
        }
        for r in &t.renders {
            push(r.file.clone(), r.sha256.clone());
        }
    }
    out
}

/// The text of everything in the document but its files index, in which a file an unknown
/// extension or a later field refers to is mentioned by path, by name or by hash.
fn mentions(doc: &Document) -> String {
    let mut value = crate::to_value(doc);
    if let Value::Object(map) = &mut value {
        map.remove("files");
    }
    value.to_string()
}

/// Whether the document's text mentions an index entry: its path, its bare name or its hash.
fn mentioned(text: &str, entry: &FileEntry) -> bool {
    let name = entry.path.rsplit('/').next().unwrap_or(&entry.path);
    text.contains(&entry.path) || text.contains(name) || text.contains(&entry.sha256)
}

/// Every namespace under an `extensions` key anywhere in the document, unknown objects
/// included (they are in the document's value as read).
pub fn used_namespaces(doc: &Document) -> BTreeSet<String> {
    fn walk(value: &Value, out: &mut BTreeSet<String>) {
        match value {
            Value::Object(map) => {
                for (key, inner) in map {
                    if key == "extensions" {
                        if let Value::Object(namespaces) = inner {
                            out.extend(namespaces.keys().cloned());
                        }
                    } else {
                        walk(inner, out);
                    }
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, out);
                }
            }
            _ => {}
        }
    }
    let mut out = BTreeSet::new();
    walk(&crate::to_value(doc), &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folder::{Folder, MemoryStore};
    use crate::model::{AudioPiece, MapPoint, Note, Pattern, Track};
    use crate::validate::Severity;
    use crate::{Id, equal};

    fn generator() -> Generator {
        Generator::new("test", "0.1", "https://example.org")
    }

    fn piece(stored: &Stored) -> AudioPiece {
        AudioPiece {
            file: stored.reference.clone(),
            sha256: stored.sha256.clone(),
            sample_rate: 48_000,
            channels: 1,
            frames: 4,
            name: None,
            from: 0,
            to: 4,
            map: vec![MapPoint {
                frame: 0,
                tick: 0,
                unknown: Unknown::new(),
            }],
            bpm: None,
            gain: 1.0,
            polarity: false,
            channel_mode: "as_recorded".to_owned(),
            fade_in: None,
            fade_out: None,
            reverse: false,
            extensions: Default::default(),
            unknown: Unknown::new(),
        }
    }

    fn track_with_piece(stored: &Stored) -> Track {
        let mut track = Track::new(Id::new("trk_1").unwrap(), "Vox");
        let mut pattern = Pattern {
            id: Id::new("pat_1").unwrap(),
            name: String::new(),
            length: 960,
            key: None,
            events: Vec::new(),
            extensions: Default::default(),
            unknown: Unknown::new(),
        };
        pattern.events.push(Event::Note(Note {
            id: Id::new("evt_1").unwrap(),
            at: 0,
            length: 960,
            pitch: 60,
            velocity: 0.8,
            release: 0.5,
            expressions: None,
            audio: Some(piece(stored)),
            extensions: Default::default(),
            unknown: Unknown::new(),
        }));
        track.patterns.push(pattern);
        track
    }

    #[test]
    fn a_project_is_created_filled_saved_and_reopened_complete() {
        let store = MemoryStore::new();
        let mut project = Project::create(store.clone(), "My Song: take 1", &generator()).unwrap();
        assert_eq!(project.document_name(), "My-Song-take-1.tutti");
        let files = store.files();
        assert!(files.contains_key("My-Song-take-1.tutti"), "{files:?}");
        assert!(files.contains_key("README.md"));
        assert!(!files.keys().any(|k| k.ends_with(".tmp")));
        assert!(store.list("audio").unwrap().is_empty());

        // A recording streams in through the handle; the document names it; a state goes in by
        // content.
        let mut take = project.new_audio("Vox: take 1/2", "wav").unwrap();
        take.write_all(b"RI").unwrap();
        take.write_all(b"FF").unwrap();
        assert_eq!(take.reference(), "audio/Vox-take-1-2.wav");
        let stored = take.finish().unwrap();
        assert_eq!(stored.bytes, 4);
        assert_eq!(stored.sha256, sha256_hex(b"RIFF"));
        assert_eq!(project.unnamed(), 1);
        let state = project.put_state("clap", b"state").unwrap();
        assert_eq!(
            state.path(),
            format!("plugins/states/{}.clap", sha256_hex(b"state"))
        );
        project
            .document_mut()
            .tracks
            .push(track_with_piece(&stored));
        let saved = project.save(&generator()).unwrap();
        assert!(saved.warnings.is_empty(), "{:?}", saved.warnings);
        assert!(!saved.readme_written, "the README did not change");
        drop(stored);
        assert_eq!(project.unnamed(), 0);

        let opened = Project::open(store.clone()).unwrap();
        assert!(opened.report.is_valid());
        assert!(!opened.read_only);
        assert!(equal(opened.project.document(), project.document()));
        assert_eq!(opened.project.document_hash(), project.document_hash());
        assert_eq!(opened.project.document().files.len(), 1);
        assert_eq!(
            opened.project.document().files[0].path,
            "audio/Vox-take-1-2.wav"
        );
        assert_eq!(opened.project.document().files[0].bytes, 4);
        assert!(opened.project.check().is_valid());
        assert_eq!(
            opened.project.read("audio/Vox-take-1-2.wav").unwrap(),
            b"RIFF"
        );
    }

    #[test]
    fn an_unfinished_recording_goes_and_the_same_content_is_stored_once() {
        let store = MemoryStore::new();
        let project = Project::create(store.clone(), "A", &generator()).unwrap();
        {
            let mut take = project.new_audio("Take", "wav").unwrap();
            take.write_all(b"half").unwrap();
            assert_eq!(project.unnamed(), 1);
        }
        assert_eq!(project.unnamed(), 0);
        assert!(!store.files().contains_key("audio/Take.wav"));

        let a = project.put_audio("Loop", "wav", b"RIFF").unwrap();
        let b = project.put_audio("Other name", "wav", b"RIFF").unwrap();
        assert_eq!(a.reference, "audio/Loop.wav");
        assert_eq!(b.reference, "audio/Loop.wav", "the same content once");
        let c = project.put_audio("Loop", "wav", b"RIFX").unwrap();
        assert_eq!(
            c.reference, "audio/Loop_2.wav",
            "a free name, never overwritten"
        );
        // Two handles for the same name get different names, before either file is closed.
        let x = project.new_audio("Pair", "wav").unwrap();
        let y = project.new_audio("Pair", "wav").unwrap();
        assert_ne!(x.reference(), y.reference());
    }

    #[test]
    fn a_save_refuses_a_missing_or_changed_file_and_states_what_it_does_not_see() {
        let store = MemoryStore::new();
        let mut project = Project::create(store.clone(), "A", &generator()).unwrap();
        let stored = project.put_audio("Loop", "wav", b"RIFF").unwrap();
        project
            .document_mut()
            .tracks
            .push(track_with_piece(&stored));
        project.save(&generator()).unwrap();

        // A change the store's clock shows, whatever the length: hashed again, and refused.
        store.put("audio/Loop.wav", b"RIFFx".to_vec());
        match project.save(&generator()) {
            Err(ProjectError::Mismatch { reference, .. }) => {
                assert_eq!(reference, "audio/Loop.wav")
            }
            other => panic!("{other:?}"),
        }
        store.put("audio/Loop.wav", b"RIFX".to_vec());
        assert!(matches!(
            project.save(&generator()),
            Err(ProjectError::Mismatch { .. })
        ));
        store.put("audio/Loop.wav", b"RIFF".to_vec());
        project.save(&generator()).unwrap();

        // A forged change, the same length with the time put back: `save` does not see it,
        // `check` and `save_verified` do.
        store.put_unseen("audio/Loop.wav", b"RIFX".to_vec());
        project.save(&generator()).unwrap();
        assert!(!project.check().is_valid());
        assert!(matches!(
            project.save_verified(&generator()),
            Err(ProjectError::Mismatch { .. })
        ));
        store.remove("audio/Loop.wav").unwrap();
        assert!(matches!(
            project.save(&generator()),
            Err(ProjectError::MissingFile(r)) if r == "audio/Loop.wav"
        ));
        // A fresh session verifies the hash on its first save, unless it was given the records
        // of the last: then a file whose length and time are still the record's is trusted
        // without being hashed, a forged change included, and one whose time differs is hashed.
        store.put("audio/Loop.wav", b"RIFX".to_vec());
        let mut again = Project::open(store.clone()).unwrap().project;
        assert!(matches!(
            again.save(&generator()),
            Err(ProjectError::Mismatch { .. })
        ));
        store.put("audio/Loop.wav", b"RIFF".to_vec());
        let mut trusting = Project::open(store.clone()).unwrap().project;
        trusting.trust(project.verified());
        trusting.save(&generator()).unwrap();
        let records = trusting.verified();
        assert!(
            records
                .iter()
                .any(|(r, v)| r == "audio/Loop.wav" && v.modified.is_some())
        );
        store.put_unseen("audio/Loop.wav", b"RIFX".to_vec());
        let mut trusting = Project::open(store.clone()).unwrap().project;
        trusting.trust(records);
        trusting.save(&generator()).unwrap();
        assert!(
            !trusting.check().is_valid(),
            "trusted by length and time, forged"
        );
        let mut fresh = Project::open(store.clone()).unwrap().project;
        assert!(matches!(
            fresh.save(&generator()),
            Err(ProjectError::Mismatch { .. })
        ));
    }

    #[test]
    fn an_index_entry_the_document_mentions_is_kept_verified_and_refused_when_gone() {
        let store = MemoryStore::new();
        let mut project = Project::create(store.clone(), "A", &generator()).unwrap();
        store.put("renders/sidecar.bin", b"abc".to_vec());
        store.put("renders/stale.bin", b"old".to_vec());
        project.document_mut().files.push(FileEntry {
            path: "renders/sidecar.bin".to_owned(),
            sha256: sha256_hex(b"abc"),
            bytes: 3,
            unknown: Unknown::new(),
        });
        // An entry nothing mentions is stale, and goes.
        project.document_mut().files.push(FileEntry {
            path: "renders/stale.bin".to_owned(),
            sha256: sha256_hex(b"old"),
            bytes: 3,
            unknown: Unknown::new(),
        });
        project.document_mut().extensions.insert(
            "org.example.sidecar".to_owned(),
            serde_json::json!({"file": "renders/sidecar.bin"}),
        );
        project.save(&generator()).unwrap();
        let reopened = Project::open(store.clone()).unwrap().project;
        assert_eq!(reopened.document().files.len(), 1);
        assert_eq!(
            reopened.document().extensions_used,
            vec!["org.example.sidecar"]
        );

        store.remove("renders/sidecar.bin").unwrap();
        let mut project = reopened;
        assert!(matches!(
            project.save(&generator()),
            Err(ProjectError::MissingFile(r)) if r == "renders/sidecar.bin"
        ));
        project.forget_file("renders/sidecar.bin");
        project.save(&generator()).unwrap();
        assert!(project.document().files.is_empty());
    }

    #[test]
    fn a_later_minor_opens_read_only_and_keeps_its_version_when_it_can_be_written() {
        let store = MemoryStore::new();
        let later = br#"{"format": "tutti", "version": "1.7", "min_version": "1.7", "generator": {"name": "x"},
            "ppq": 960, "tempo": [{"at": 0, "bpm": 120}], "meter": [{"at": 0, "numerator": 4, "denominator": 4}],
            "swing": {"later": true}}"#;
        store.write_new("Later.tutti", later).unwrap();
        let opened = Project::open(store.clone()).unwrap();
        assert!(opened.read_only);
        let mut project = opened.project;
        assert!(matches!(
            project.save(&generator()),
            Err(ProjectError::ReadOnly)
        ));

        let store = MemoryStore::new();
        let later = br#"{"format": "tutti", "version": "1.7", "generator": {"name": "x"},
            "ppq": 960, "tempo": [{"at": 0, "bpm": 120}], "meter": [{"at": 0, "numerator": 4, "denominator": 4}],
            "swing": {"later": true}}"#;
        store.write_new("Later.tutti", later).unwrap();
        let opened = Project::open(store.clone()).unwrap();
        assert!(!opened.read_only);
        let mut project = opened.project;
        project.save(&generator()).unwrap();
        let text = String::from_utf8(store.read("Later.tutti").unwrap()).unwrap();
        assert!(text.contains("\"version\": \"1.7\""), "{text}");
        assert!(text.contains("\"swing\""), "{text}");
        assert!(text.contains("\"about\""), "{text}");
    }

    #[test]
    fn no_document_two_documents_and_an_invalid_one_are_refused_by_name() {
        let store = MemoryStore::new();
        assert!(matches!(
            Project::open(store.clone()),
            Err(ProjectError::NoDocument)
        ));
        store.write_new("A.tutti", b"{}").unwrap();
        store.write_new("B.tutti", b"{}").unwrap();
        assert!(matches!(
            Project::open(store.clone()),
            Err(ProjectError::SeveralDocuments(names)) if names == ["A.tutti", "B.tutti"]
        ));
        assert!(matches!(
            Project::create(store.clone(), "C", &generator()),
            Err(ProjectError::DocumentExists(_))
        ));

        let store = MemoryStore::new();
        let mut project = Project::create(store.clone(), "A", &generator()).unwrap();
        project.document_mut().ppq = 0;
        match project.save(&generator()) {
            Err(ProjectError::Invalid(issues)) => {
                assert!(issues.iter().all(|i| i.severity == Severity::Error));
                assert!(issues.iter().any(|i| i.pointer == "/ppq"), "{issues:?}");
            }
            other => panic!("{other:?}"),
        }
        // Nothing was written: the saved document still has its ppq.
        let text = String::from_utf8(store.read("A.tutti").unwrap()).unwrap();
        assert!(text.contains("\"ppq\": 960"), "{text}");
    }

    #[test]
    fn a_copy_is_complete_and_checks_out() {
        let store = MemoryStore::new();
        let mut project = Project::create(store.clone(), "A", &generator()).unwrap();
        let stored = project.put_audio("Loop", "wav", b"RIFF").unwrap();
        project
            .document_mut()
            .tracks
            .push(track_with_piece(&stored));
        project.put_state("clap", b"state").unwrap();
        project.save(&generator()).unwrap();

        let other = MemoryStore::new();
        let copy = project.copy_into(other.clone()).unwrap();
        assert!(equal(copy.document(), project.document()));
        assert!(copy.check().is_valid());
        assert_eq!(other.read("audio/Loop.wav").unwrap(), b"RIFF");
        assert!(other.files().contains_key("README.md"));
        assert!(matches!(
            project.copy_into(other.clone()),
            Err(ProjectError::DocumentExists(_))
        ));
    }

    #[test]
    fn a_devices_own_files_are_indexed_by_its_id_copied_with_it_and_dropped_with_it() {
        let store = MemoryStore::new();
        let mut project = Project::create(store.clone(), "A", &generator()).unwrap();
        let mut track = Track::new(Id::new("trk_1").unwrap(), "Synth");
        let device = crate::model::Device {
            id: Id::new("dev_1").unwrap(),
            role: "instrument".to_owned(),
            name: "sampler".to_owned(),
            plugin: "clap:org.example.sampler".to_owned(),
            aliases: Vec::new(),
            state: None,
            params: Vec::new(),
            overrides: Vec::new(),
            enabled: true,
            extensions: Default::default(),
            unknown: Unknown::new(),
        };
        track.devices.push(device);
        project.document_mut().tracks.push(track);
        let files = project.device_files("dev_1").unwrap();
        let entry = files.put("kits/Kick.wav", b"RIFF").unwrap();
        assert_eq!(entry.path, "plugins/files/dev_1/kits/Kick.wav");
        files.put("map.json", b"{}").unwrap();
        assert!(matches!(
            files.put("../out.txt", b"x"),
            Err(ProjectError::Name(_))
        ));
        assert_eq!(files.list().unwrap(), ["kits/Kick.wav", "map.json"]);
        assert_eq!(files.read("kits/Kick.wav").unwrap(), b"RIFF");
        // Indexed at the save, by the device's id; the index entry verified like any other.
        project.document_mut().files.push(entry);
        project.save(&generator()).unwrap();
        let paths: Vec<&str> = project
            .document()
            .files
            .iter()
            .map(|f| f.path.as_str())
            .collect();
        assert_eq!(
            paths,
            ["plugins/files/dev_1/kits/Kick.wav"],
            "map.json was not indexed"
        );
        project.document_mut().files.push(FileEntry {
            path: "plugins/files/dev_1/map.json".to_owned(),
            sha256: sha256_hex(b"{}"),
            bytes: 2,
            unknown: Unknown::new(),
        });
        project.save(&generator()).unwrap();
        assert!(project.check().is_valid());
        assert_eq!(project.document().files.len(), 2);
        // Duplicated: the files go with the copy.
        assert_eq!(project.copy_device_files("dev_1", "dev_2").unwrap(), 2);
        assert_eq!(
            project.device_files("dev_2").unwrap().list().unwrap(),
            ["kits/Kick.wav", "map.json"]
        );
        // The device gone, its entries go from the index; the files stay for the program.
        project.document_mut().tracks[0].devices.clear();
        project.save(&generator()).unwrap();
        assert!(project.document().files.is_empty());
        assert!(store.files().contains_key("plugins/files/dev_1/map.json"));
    }

    #[test]
    fn the_namespaces_used_are_found_everywhere_and_a_bad_name_is_refused() {
        let text = br#"{"format": "tutti", "version": "1.0", "generator": {"name": "x"}, "ppq": 960,
            "tempo": [{"at": 0, "bpm": 120}], "meter": [{"at": 0, "numerator": 4, "denominator": 4}],
            "extensions": {"org.example.root": {}},
            "tracks": [{"id": "t", "patterns": [{"id": "p", "length": 960, "events": [
              {"type": "glide", "id": "e", "at": 0, "extensions": {"org.example.unknown-event": 1}}]}],
              "extensions": {"dk.mxm.newdawn": {"armed": true}}}]}"#;
        let doc = read(text).unwrap();
        let used: Vec<String> = used_namespaces(&doc).into_iter().collect();
        assert_eq!(
            used,
            [
                "dk.mxm.newdawn",
                "org.example.root",
                "org.example.unknown-event"
            ]
        );
        let store = MemoryStore::new();
        let project = Project::create(store, "A", &generator()).unwrap();
        assert!(matches!(
            project.new_audio("x", "wav/../"),
            Err(ProjectError::Name(_))
        ));
        assert!(matches!(
            Project::adopt(
                MemoryStore::new(),
                "no-extension",
                Document::new(generator())
            ),
            Err(ProjectError::Name(_))
        ));
    }
}
