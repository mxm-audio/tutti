//! A project folder as the core sees it: a trait the caller implements (the command line with
//! `std::fs`, a JavaScript host with its own files), and the checks that need the folder: the
//! files index complete and valid (spec 3.4, 9), and what the document refers to being there.

use std::fmt;

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
    /// The folder's own reason.
    Other(String),
}

impl fmt::Display for FolderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FolderError::Missing => write!(f, "missing"),
            FolderError::Link => write!(f, "a link, which a reader refuses"),
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
}

/// A folder held in memory: references to bytes. For tests, and for a host that unpacked an
/// archive.
#[derive(Clone, Debug, Default)]
pub struct MemoryFolder {
    pub files: std::collections::BTreeMap<String, Vec<u8>>,
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

/// Every file the document refers to, as references, in order of first use: audio pieces'
/// files, devices' states, renders.
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
    }
    out
}

/// Checks the folder against the document (spec 3.4, 9): every indexed file present with its
/// length and hash (`complete` and `valid`, as BagIt says), every referenced file present and
/// indexed, every audio piece's facts matching its index entry.
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
    for t in doc.all_tracks() {
        for p in &t.patterns {
            for e in &p.events {
                if let Event::Note(n) = e {
                    if let Some(a) = &n.audio {
                        if let Some(f) = doc.files.iter().find(|f| f.path == a.file) {
                            if f.sha256 != a.sha256 {
                                warn(
                                    &mut issues,
                                    format!("/files (track {})", t.id),
                                    format!(
                                        "{}: the piece names hash {} but the index {}",
                                        a.file, a.sha256, f.sha256
                                    ),
                                );
                            }
                        }
                    }
                }
            }
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
