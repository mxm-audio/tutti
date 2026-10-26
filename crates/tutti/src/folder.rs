//! A project folder as the core sees it: `Folder`, what a reader needs (read a file by
//! reference, its size, its hash), and `Store`, what a writer needs besides (list, make a
//! folder, write a new file, stream one, rename over, remove), each a trait the host implements:
//! `tutti-fs` with `std::fs`, a JavaScript host with its own files, `MemoryStore` here for tests
//! and hosts that keep a project in memory. And the checks that need the folder: the files index
//! complete and valid (spec 3.4, 9), and what the document refers to being there.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::model::{Document, Event};
use crate::render::sha256_hex;
use crate::validate::{Issue, Severity};

/// Why a folder could not give a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderError {
    /// No such file.
    Missing,
    /// A link where a file or folder was expected (spec 9): refused.
    Link,
    /// A file is already there where a new one was to be written: nothing is overwritten.
    Exists,
    /// The folder's own reason.
    Other(String),
}

impl fmt::Display for FolderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FolderError::Missing => write!(f, "missing"),
            FolderError::Link => write!(f, "a link, which a reader refuses"),
            FolderError::Exists => write!(f, "already there"),
            FolderError::Other(s) => f.write_str(s),
        }
    }
}

/// The files of a project folder, by reference (`audio/<name>`, spec 2.2). An implementation
/// resolves inside the folder only, follows no link, and reads only the folders a reader reads.
pub trait Folder {
    /// The whole file.
    fn read(&self, reference: &str) -> Result<Vec<u8>, FolderError>;

    /// The file's length in bytes, without reading it when the folder can tell.
    fn size(&self, reference: &str) -> Result<u64, FolderError> {
        self.read(reference).map(|b| b.len() as u64)
    }

    /// The file's SHA-256 and length. An implementation that can stream overrides this so a
    /// large file is never held whole.
    fn sha256(&self, reference: &str) -> Result<(String, u64), FolderError> {
        let bytes = self.read(reference)?;
        Ok((sha256_hex(&bytes), bytes.len() as u64))
    }

    /// When the file was last changed, as the store tells it: nanoseconds since the Unix
    /// epoch, as precise as it knows; `None` when it can't say. With the length, what a save
    /// trusts a verified hash by.
    fn modified(&self, reference: &str) -> Result<Option<u64>, FolderError> {
        let _ = reference;
        Ok(None)
    }

    /// Copies the file into `out`, giving its length: how a project is packed into one file
    /// (`tutti-zip`). An implementation that can stream overrides this so a large file is never
    /// held whole.
    fn copy_to(&self, reference: &str, out: &mut dyn io::Write) -> Result<u64, FolderError> {
        let bytes = self.read(reference)?;
        out.write_all(&bytes)
            .map_err(|e| FolderError::Other(e.to_string()))?;
        Ok(bytes.len() as u64)
    }
}

/// What an entry of a folder is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    File,
    Folder,
    /// A link or anything else: a reader resolves nothing through it (spec 9).
    Other,
}

/// One entry of a folder, by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
}

/// A project folder a writer can change: the primitive operations the save procedure
/// (`Project`) is composed of, each one a point where a crash can happen and nothing is half
/// done as far as the store is concerned. References and folders are relative paths with `/`,
/// the root being `""`; a file's parents are made as needed. An implementation is a cheap
/// handle (`Clone`): a recording handle carries one. It resolves inside the root only, follows
/// no link, and refuses a link where a file or folder is expected.
pub trait Store: Folder + Clone {
    /// Where a new file's bytes go while it is being written.
    type Writer: io::Write;

    /// The entries directly in `folder`, in any order; `Missing` when the folder is not there.
    fn list(&self, folder: &str) -> Result<Vec<Entry>, FolderError>;

    /// Whether a file is there; `Link` for a link in its place.
    fn exists(&self, reference: &str) -> Result<bool, FolderError>;

    /// Makes a folder and its parents; nothing when it is there.
    fn make_folder(&self, folder: &str) -> Result<(), FolderError>;

    /// Writes a new file whole and durably; `Exists` when a file is there already.
    fn write_new(&self, reference: &str, bytes: &[u8]) -> Result<(), FolderError>;

    /// Opens a new file for streaming; `Exists` when a file is there already. The file is
    /// there, and durable, only once [`Store::close`] returns.
    fn open_new(&self, reference: &str) -> Result<Self::Writer, FolderError>;

    /// Finishes a streamed file: flushed and durable.
    fn close(&self, writer: Self::Writer) -> Result<(), FolderError>;

    /// Renames `from` over `to`, replacing it in one step: the commit point of a save.
    fn rename_over(&self, from: &str, to: &str) -> Result<(), FolderError>;

    /// Flushes a folder's entries (`""` the root), so a rename in it survives a power cut;
    /// nothing where the system needs none.
    fn sync_folder(&self, folder: &str) -> Result<(), FolderError>;

    /// Removes a file; `Missing` when it is not there.
    fn remove(&self, reference: &str) -> Result<(), FolderError>;
}

/// A folder held in memory: references to bytes. For tests, and for a host that unpacked an
/// archive.
#[derive(Clone, Debug, Default)]
pub struct MemoryFolder {
    pub files: BTreeMap<String, Vec<u8>>,
}

impl Folder for MemoryFolder {
    fn read(&self, reference: &str) -> Result<Vec<u8>, FolderError> {
        self.files
            .get(reference)
            .cloned()
            .ok_or(FolderError::Missing)
    }

    fn size(&self, reference: &str) -> Result<u64, FolderError> {
        self.files
            .get(reference)
            .map(|b| b.len() as u64)
            .ok_or(FolderError::Missing)
    }
}

#[derive(Debug, Default)]
struct Memory {
    files: BTreeMap<String, Vec<u8>>,
    /// When each file was last written, on a clock that ticks once per write.
    stamps: BTreeMap<String, u64>,
    folders: BTreeSet<String>,
    clock: u64,
}

impl Memory {
    fn stamp(&mut self, reference: &str) {
        self.clock += 1;
        self.stamps.insert(reference.to_owned(), self.clock);
    }
}

/// A store held in memory, shared by every clone: a project a test or a host builds without a
/// file system, written out afterwards with [`MemoryStore::files`]. The root is always there.
/// Its modification times are a clock that ticks once per write.
#[derive(Clone, Debug, Default)]
pub struct MemoryStore {
    memory: Arc<Mutex<Memory>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Memory> {
        self.memory.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Every file, by reference.
    pub fn files(&self) -> BTreeMap<String, Vec<u8>> {
        self.lock().files.clone()
    }

    /// Puts a file whatever is there, as a change behind a project's back the store's clock
    /// shows.
    pub fn put(&self, reference: &str, bytes: Vec<u8>) {
        let mut memory = self.lock();
        memory.files.insert(reference.to_owned(), bytes);
        memory.stamp(reference);
    }

    /// Puts a file whatever is there, keeping its modification time: a change forged behind
    /// a project's back, which a save can't see and `check` can.
    pub fn put_unseen(&self, reference: &str, bytes: Vec<u8>) {
        self.lock().files.insert(reference.to_owned(), bytes);
    }
}

/// The bytes of a file being streamed into a [`MemoryStore`].
#[derive(Debug)]
pub struct MemoryWriter {
    reference: String,
    buffer: Vec<u8>,
}

impl io::Write for MemoryWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Folder for MemoryStore {
    fn read(&self, reference: &str) -> Result<Vec<u8>, FolderError> {
        self.lock()
            .files
            .get(reference)
            .cloned()
            .ok_or(FolderError::Missing)
    }

    fn size(&self, reference: &str) -> Result<u64, FolderError> {
        self.lock()
            .files
            .get(reference)
            .map(|b| b.len() as u64)
            .ok_or(FolderError::Missing)
    }

    fn modified(&self, reference: &str) -> Result<Option<u64>, FolderError> {
        let memory = self.lock();
        if !memory.files.contains_key(reference) {
            return Err(FolderError::Missing);
        }
        Ok(memory.stamps.get(reference).copied())
    }
}

impl Store for MemoryStore {
    type Writer = MemoryWriter;

    fn list(&self, folder: &str) -> Result<Vec<Entry>, FolderError> {
        let memory = self.lock();
        let prefix = if folder.is_empty() {
            String::new()
        } else {
            format!("{folder}/")
        };
        let mut names: BTreeMap<String, Kind> = BTreeMap::new();
        for path in memory.files.keys() {
            if let Some(rest) = path.strip_prefix(&prefix) {
                match rest.split_once('/') {
                    Some((first, _)) => names.insert(first.to_owned(), Kind::Folder),
                    None => names.insert(rest.to_owned(), Kind::File),
                };
            }
        }
        for path in &memory.folders {
            if let Some(rest) = path.strip_prefix(&prefix) {
                let first = rest.split('/').next().unwrap_or(rest);
                if !first.is_empty() {
                    names.entry(first.to_owned()).or_insert(Kind::Folder);
                }
            }
        }
        if names.is_empty() && !folder.is_empty() && !memory.folders.contains(folder) {
            return Err(FolderError::Missing);
        }
        Ok(names
            .into_iter()
            .map(|(name, kind)| Entry { name, kind })
            .collect())
    }

    fn exists(&self, reference: &str) -> Result<bool, FolderError> {
        Ok(self.lock().files.contains_key(reference))
    }

    fn make_folder(&self, folder: &str) -> Result<(), FolderError> {
        if !folder.is_empty() {
            self.lock().folders.insert(folder.to_owned());
        }
        Ok(())
    }

    fn write_new(&self, reference: &str, bytes: &[u8]) -> Result<(), FolderError> {
        let mut memory = self.lock();
        if memory.files.contains_key(reference) {
            return Err(FolderError::Exists);
        }
        memory.files.insert(reference.to_owned(), bytes.to_vec());
        memory.stamp(reference);
        Ok(())
    }

    fn open_new(&self, reference: &str) -> Result<Self::Writer, FolderError> {
        if self.lock().files.contains_key(reference) {
            return Err(FolderError::Exists);
        }
        Ok(MemoryWriter {
            reference: reference.to_owned(),
            buffer: Vec::new(),
        })
    }

    fn close(&self, writer: Self::Writer) -> Result<(), FolderError> {
        self.write_new(&writer.reference, &writer.buffer)
    }

    fn rename_over(&self, from: &str, to: &str) -> Result<(), FolderError> {
        let mut memory = self.lock();
        let bytes = memory.files.remove(from).ok_or(FolderError::Missing)?;
        memory.stamps.remove(from);
        memory.files.insert(to.to_owned(), bytes);
        memory.stamp(to);
        Ok(())
    }

    fn sync_folder(&self, _folder: &str) -> Result<(), FolderError> {
        Ok(())
    }

    fn remove(&self, reference: &str) -> Result<(), FolderError> {
        let mut memory = self.lock();
        memory.stamps.remove(reference);
        memory
            .files
            .remove(reference)
            .map(|_| ())
            .ok_or(FolderError::Missing)
    }
}

/// Every file the document refers to, as references, in order of first use: audio pieces'
/// files, devices' states, track renders, pattern renders.
pub fn referenced_files(doc: &Document) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |r: String| {
        if !out.contains(&r) {
            out.push(r);
        }
    };
    for d in &doc.master.devices {
        if let Some(s) = &d.state {
            push(s.path());
        }
    }
    for t in doc.all_tracks() {
        for d in &t.devices {
            if let Some(s) = &d.state {
                push(s.path());
            }
        }
        for p in &t.patterns {
            for e in &p.events {
                if let Event::Note(n) = e {
                    if let Some(a) = &n.audio {
                        push(a.file.clone());
                    }
                }
            }
        }
        for r in &t.renders {
            push(r.file.clone());
        }
        for p in &t.patterns {
            for r in &p.renders {
                push(r.file.clone());
            }
        }
    }
    out
}

/// Checks the folder against the document (spec 3.4, 9): every indexed file present with its
/// length and hash (`complete` and `valid`, as BagIt says), every referenced file present and
/// indexed, every audio piece's and every render's hash matching its index entry.
pub fn check_files(doc: &Document, folder: &dyn Folder) -> Vec<Issue> {
    let mut issues = Vec::new();
    let error = |issues: &mut Vec<Issue>, pointer: String, message: String| {
        issues.push(Issue {
            severity: Severity::Error,
            pointer,
            message,
        });
    };
    let warn = |issues: &mut Vec<Issue>, pointer: String, message: String| {
        issues.push(Issue {
            severity: Severity::Warning,
            pointer,
            message,
        });
    };
    for (i, f) in doc.files.iter().enumerate() {
        let p = format!("/files/{i}");
        match folder.size(&f.path) {
            Err(e) => error(&mut issues, format!("{p}/path"), format!("{}: {e}", f.path)),
            Ok(size) if size != f.bytes => error(
                &mut issues,
                format!("{p}/bytes"),
                format!("{} is {size} bytes, not {}", f.path, f.bytes),
            ),
            Ok(_) => match folder.sha256(&f.path) {
                Ok((hash, _)) if hash == f.sha256 => {}
                Ok((hash, _)) => error(
                    &mut issues,
                    format!("{p}/sha256"),
                    format!("{} hashes to {hash}, not {}", f.path, f.sha256),
                ),
                Err(e) => error(&mut issues, format!("{p}/path"), format!("{}: {e}", f.path)),
            },
        }
    }
    for reference in referenced_files(doc) {
        let indexed = doc.files.iter().find(|f| f.path == reference);
        if indexed.is_none() {
            warn(
                &mut issues,
                "/files".to_owned(),
                format!("{reference} is referred to but not in the index"),
            );
            if let Err(e) = folder.size(&reference) {
                error(
                    &mut issues,
                    "/files".to_owned(),
                    format!("{reference}: {e}"),
                );
            }
        }
    }
    let disagree = |issues: &mut Vec<Issue>, track: &str, what: &str, file: &str, sha256: &str| {
        if let Some(f) = doc.files.iter().find(|f| f.path == file) {
            if f.sha256 != sha256 {
                warn(
                    issues,
                    format!("/files (track {track})"),
                    format!(
                        "{file}: the {what} names hash {sha256} but the index {}",
                        f.sha256
                    ),
                );
            }
        }
    };
    for t in doc.all_tracks() {
        for p in &t.patterns {
            for e in &p.events {
                if let Event::Note(n) = e {
                    if let Some(a) = &n.audio {
                        disagree(&mut issues, t.id.as_str(), "piece", &a.file, &a.sha256);
                    }
                }
            }
            for r in &p.renders {
                disagree(&mut issues, t.id.as_str(), "render", &r.file, &r.sha256);
            }
        }
        for r in &t.renders {
            disagree(&mut issues, t.id.as_str(), "render", &r.file, &r.sha256);
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read;

    #[test]
    fn complete_and_valid() {
        let hash = sha256_hex(b"RIFF");
        let text = format!(
            r#"{{"format": "tutti", "version": "1.0", "generator": {{"name": "t"}}, "ppq": 960,
            "tempo": [{{"at": 0, "bpm": 120}}], "meter": [{{"at": 0, "numerator": 4, "denominator": 4}}],
            "files": [{{"path": "audio/x.wav", "sha256": "{hash}", "bytes": 4}}],
            "tracks": [{{"id": "t", "devices": [{{"id": "d", "role": "instrument", "plugin": "clap:x",
              "state": {{"sha256": "{hash}", "kind": "clap"}}}}]}}]}}"#
        );
        let doc = read(text.as_bytes()).unwrap();
        let mut folder = MemoryFolder::default();
        folder
            .files
            .insert("audio/x.wav".to_owned(), b"RIFF".to_vec());
        let issues = check_files(&doc, &folder);
        // The state is referred to but neither indexed nor present: a warning and an error.
        assert_eq!(issues.len(), 2, "{issues:?}");
        assert!(
            issues
                .iter()
                .any(|i| i.severity == Severity::Error && i.message.contains("plugins/states"))
        );
        folder
            .files
            .insert(format!("plugins/states/{hash}.clap"), b"RIFF".to_vec());
        let issues = check_files(&doc, &folder);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Warning);
        folder
            .files
            .insert("audio/x.wav".to_owned(), b"RIFX".to_vec());
        let issues = check_files(&doc, &folder);
        assert!(issues.iter().any(|i| i.pointer == "/files/0/sha256"));
    }
}
