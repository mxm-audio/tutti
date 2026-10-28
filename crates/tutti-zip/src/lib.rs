//! A project as one file, `<Name>.tutti.zip` (spec 2.5): a project packed from any store and
//! unpacked into any store, refusing what a reader must refuse: an entry name that leaves the
//! folder, a link, a length that is not what the entry declares, a declared length beyond what a
//! project file may be. The archive is anything that seeks, a file (`tutti-fs`'s `zip`) or bytes
//! in memory (the JavaScript package's); the store is the core's `Store`, so nothing is written
//! outside it. The rules the core states without IO are `tutti::zip`'s.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io::{self, Read, Seek, Write};

use tutti::folder::{FolderError, Kind, Store};
use tutti::zip::{UNINDEXED_LIMIT, check_entry_name};
use tutti::{Project, ProjectError};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// The folders a reader reads, walked when a project is packed (spec 2.3).
const FOLDERS: [&str; 4] = ["audio", "plugins", "renders", "analysis"];
const README: &str = "README.md";

/// Why a project could not be packed or unpacked.
#[derive(Debug)]
pub enum ZipError {
    /// The store, or the file, is not a project: the reason.
    NotAProject(String),
    /// An entry a reader refuses: its name and the reason (spec 2.5).
    Entry {
        name: String,
        reason: String,
    },
    /// Unpacking would land in a store that holds a project already.
    HoldsProject,
    Io(io::Error),
    Zip(zip::result::ZipError),
}

impl fmt::Display for ZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZipError::NotAProject(reason) => write!(f, "not a project: {reason}"),
            ZipError::Entry { name, reason } => write!(f, "{name}: {reason}"),
            ZipError::HoldsProject => write!(f, "holds a project already"),
            ZipError::Io(e) => write!(f, "{e}"),
            ZipError::Zip(e) => write!(f, "not a zip file this reader opens: {e}"),
        }
    }
}

impl std::error::Error for ZipError {}

impl From<io::Error> for ZipError {
    fn from(e: io::Error) -> Self {
        ZipError::Io(e)
    }
}

impl From<zip::result::ZipError> for ZipError {
    fn from(e: zip::result::ZipError) -> Self {
        ZipError::Zip(e)
    }
}

fn entry_error(name: &str) -> impl FnOnce(FolderError) -> ZipError + '_ {
    move |error| ZipError::Entry {
        name: name.to_owned(),
        reason: error.to_string(),
    }
}

/// Packs the project in `store` into `out`: the document first and stored, then the README and
/// every file of the project, deflated; `private/` left out. The project is opened and
/// validated first. Gives `out` back, finished, and the number of entries.
pub fn pack<S: Store, W: Write + Seek>(store: &S, out: W) -> Result<(W, usize), ZipError> {
    let opened = Project::open(store.clone()).map_err(|e| ZipError::NotAProject(e.to_string()))?;
    let mut entries: Vec<String> = vec![opened.project.document_name().to_owned()];
    if store.exists(README).map_err(entry_error(README))? {
        entries.push(README.to_owned());
    }
    for entry in &opened.project.document().files {
        if !entries.contains(&entry.path) {
            entries.push(entry.path.clone());
        }
    }
    for top in FOLDERS {
        walk(store, top, &mut entries)?;
    }

    let mut writer = ZipWriter::new(out);
    for (i, reference) in entries.iter().enumerate() {
        let size = store.size(reference).map_err(entry_error(reference))?;
        let method = if i == 0 {
            CompressionMethod::Stored
        } else {
            CompressionMethod::Deflated
        };
        let options = SimpleFileOptions::default()
            .compression_method(method)
            .large_file(size >= u64::from(u32::MAX));
        writer.start_file(reference.as_str(), options)?;
        store
            .copy_to(reference, &mut writer)
            .map_err(entry_error(reference))?;
    }
    Ok((writer.finish()?, entries.len()))
}

/// Every file under `folder`, by reference, added to `out` once; a link is passed by.
fn walk<S: Store>(store: &S, folder: &str, out: &mut Vec<String>) -> Result<(), ZipError> {
    let entries = match store.list(folder) {
        Ok(entries) => entries,
        Err(FolderError::Missing) => return Ok(()),
        Err(e) => return Err(entry_error(folder)(e)),
    };
    let mut names: Vec<_> = entries.into_iter().collect();
    names.sort_by(|a, b| a.name.cmp(&b.name));
    for entry in names {
        let reference = format!("{folder}/{}", entry.name);
        match entry.kind {
            Kind::File => {
                if !out.contains(&reference) {
                    out.push(reference);
                }
            }
            Kind::Folder => walk(store, &reference, out)?,
            Kind::Other => {}
        }
    }
    Ok(())
}

/// Unpacks the archive `input` into `store`, refused when the store holds a project already.
/// Every entry's name is checked before anything is written, and one bad name refuses the whole
/// file; the document is read first, so each indexed entry is held to its indexed length and
/// every other to the limit; an entry that is not the length it declares refuses the file. On a
/// refusal the files written are removed again. Gives the number of files written.
pub fn unpack<R: Read + Seek, S: Store>(input: R, store: &S) -> Result<usize, ZipError> {
    let mut archive = ZipArchive::new(input)?;
    // Names first: an attack is refused before a byte is written.
    let mut entries: Vec<(usize, String, u64)> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let name = entry.name()?.into_owned();
        if entry.is_symlink() {
            return Err(ZipError::Entry {
                name,
                reason: "a link, which a reader refuses".to_owned(),
            });
        }
        if let Err(reason) = check_entry_name(&name) {
            return Err(ZipError::Entry { name, reason });
        }
        if entry.is_dir() || private(&name) {
            // A folder entry says nothing; `private/` is each program's own, and stays where
            // the project was (spec 2.3, 2.5).
            continue;
        }
        entries.push((i, name, entry.size()));
    }
    let documents: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, (_, name, _))| !name.contains('/') && name.ends_with(".tutti"))
        .map(|(n, _)| n)
        .collect();
    let document_at = match documents.len() {
        1 => documents[0],
        0 => return Err(ZipError::NotAProject("no .tutti document".to_owned())),
        n => return Err(ZipError::NotAProject(format!("{n} documents"))),
    };
    let (document_index, document_name, document_size) = entries[document_at].clone();
    if document_size > UNINDEXED_LIMIT {
        return Err(too_large(&document_name, document_size));
    }
    let document_bytes = read_whole(&mut archive, document_index, &document_name, document_size)?;
    let document =
        tutti::read(&document_bytes).map_err(|e| ZipError::NotAProject(e.to_string()))?;
    let index: HashMap<&str, u64> = document
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.bytes))
        .collect();

    match tutti::project::document_in(store) {
        Ok(_) | Err(ProjectError::SeveralDocuments(_)) => return Err(ZipError::HoldsProject),
        Err(_) => {}
    }
    let mut written: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let result = (|| -> Result<(), ZipError> {
        for (i, name, declared) in &entries {
            if !seen.insert(name.to_lowercase()) {
                return Err(ZipError::Entry {
                    name: name.clone(),
                    reason: "named twice".to_owned(),
                });
            }
            let limit = match index.get(name.as_str()) {
                Some(bytes) if bytes != declared => {
                    return Err(ZipError::Entry {
                        name: name.clone(),
                        reason: format!("declares {declared} bytes, and the index says {bytes}"),
                    });
                }
                Some(bytes) => *bytes,
                None if *declared > UNINDEXED_LIMIT => return Err(too_large(name, *declared)),
                None => *declared,
            };
            let mut out = store.open_new(name).map_err(entry_error(name))?;
            written.push(name.clone());
            let copied = {
                let mut entry = archive.by_index(*i)?;
                io::copy(&mut Read::by_ref(&mut entry).take(limit + 1), &mut out)?
            };
            out.flush()?;
            store.close(out).map_err(entry_error(name))?;
            if copied != limit {
                return Err(ZipError::Entry {
                    name: name.clone(),
                    reason: format!("isn't the length it declares ({limit} bytes)"),
                });
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        for name in &written {
            let _ = store.remove(name);
        }
        return Err(e);
    }
    Ok(written.len())
}

/// Whether an entry is under `private/`, the programs' own folders (spec 2.3).
fn private(name: &str) -> bool {
    name.split('/').next() == Some("private")
}

fn too_large(name: &str, declared: u64) -> ZipError {
    ZipError::Entry {
        name: name.to_owned(),
        reason: format!("declares {declared} bytes, more than a project file may be"),
    }
}

/// An entry whole, exactly as long as it declares.
fn read_whole<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    index: usize,
    name: &str,
    declared: u64,
) -> Result<Vec<u8>, ZipError> {
    let mut entry = archive.by_index(index)?;
    let mut bytes = Vec::with_capacity(declared.min(1 << 20) as usize);
    Read::by_ref(&mut entry)
        .take(declared + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 != declared {
        return Err(ZipError::Entry {
            name: name.to_owned(),
            reason: format!("isn't the length it declares ({declared} bytes)"),
        });
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use tutti::Generator;
    use tutti::folder::{Folder, MemoryStore};

    use super::*;

    fn generator() -> Generator {
        Generator::new(
            "tutti-zip tests",
            "0.1",
            "https://github.com/mxm-audio/tutti",
        )
    }

    /// A project in memory with a recording, packed into bytes and unpacked into another store,
    /// gives the same files.
    #[test]
    fn a_project_in_memory_packs_and_unpacks_the_same() {
        let store = MemoryStore::default();
        let mut project = Project::create(store.clone(), "Round Trip", &generator()).unwrap();
        let stored = project
            .put_audio("Take", "wav", b"RIFF made audio")
            .unwrap();
        project.save(&generator()).unwrap();

        let (bytes, entries) = pack(&store, Cursor::new(Vec::new())).unwrap();
        let bytes = bytes.into_inner();
        assert_eq!(entries, 3, "the document, the README and the recording");
        // The document is the first entry, stored (spec 2.5).
        let mut archive = ZipArchive::new(Cursor::new(&bytes)).unwrap();
        let first = archive.by_index(0).unwrap();
        assert_eq!(first.name().unwrap(), "Round-Trip.tutti");
        assert_eq!(first.compression(), CompressionMethod::Stored);
        drop(first);

        let into = MemoryStore::default();
        assert_eq!(unpack(Cursor::new(&bytes), &into).unwrap(), 3);
        for reference in ["Round-Trip.tutti", "README.md", stored.reference.as_str()] {
            assert_eq!(
                into.read(reference).unwrap(),
                store.read(reference).unwrap(),
                "{reference}"
            );
        }
        assert!(Project::open(into).is_ok());
    }

    /// A program's own folder stays where the project is: packing leaves it out, and an entry
    /// under `private/` in a zip is skipped (spec 2.3, 2.5).
    #[test]
    fn private_folders_are_left_out_both_ways() {
        let store = MemoryStore::default();
        let mut project = Project::create(store.clone(), "Own Files", &generator()).unwrap();
        project.save(&generator()).unwrap();
        store
            .write_new("private/com.example.app/journal.log", b"edit 1")
            .unwrap();
        let (bytes, entries) = pack(&store, Cursor::new(Vec::new())).unwrap();
        assert_eq!(entries, 2, "the document and the README");

        let mut archive = ZipArchive::new(Cursor::new(bytes.into_inner())).unwrap();
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for i in 0..archive.len() {
            writer.raw_copy_file(archive.by_index(i).unwrap()).unwrap();
        }
        writer
            .start_file("private/com.example.app/lock", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"pid 7").unwrap();
        let bytes = writer.finish().unwrap().into_inner();

        let into = MemoryStore::default();
        assert_eq!(unpack(Cursor::new(bytes), &into).unwrap(), 2);
        assert!(!into.exists("private/com.example.app/lock").unwrap());
    }

    #[test]
    fn a_store_holding_a_project_is_not_unpacked_into() {
        let store = MemoryStore::default();
        Project::create(store.clone(), "One", &generator()).unwrap();
        let (bytes, _) = pack(&store, Cursor::new(Vec::new())).unwrap();
        let error = unpack(Cursor::new(bytes.into_inner()), &store).unwrap_err();
        assert!(matches!(error, ZipError::HoldsProject), "{error}");
    }
}
