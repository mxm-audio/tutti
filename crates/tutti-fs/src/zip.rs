//! A project as one file, `<Name>.tutti.zip` (spec 2.5): packing a project folder and unpacking
//! a file, refusing what a reader must refuse: an entry name that leaves the folder, a link, a
//! length that is not what the entry declares, a declared length beyond what a project file may
//! be. Nothing is written outside the folder being made.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use tutti::folder::{Kind, Store};
use tutti::zip::{UNINDEXED_LIMIT, check_entry_name};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::DirStore;

/// The folders a reader reads, walked when a project is packed (spec 2.3).
const FOLDERS: [&str; 4] = ["audio", "plugins", "renders", "analysis"];
const README: &str = "README.md";

/// Why a project could not be packed or unpacked.
#[derive(Debug)]
pub enum ZipError {
    /// The folder, or the file, is not a project: the reason.
    NotAProject(String),
    /// An entry a reader refuses: its name and the reason (spec 2.5).
    Entry {
        name: String,
        reason: String,
    },
    /// Unpacking would land in a folder that holds a project already.
    FolderExists(PathBuf),
    /// The file to write is there already: nothing is overwritten.
    FileExists(PathBuf),
    Io(io::Error),
    Zip(zip::result::ZipError),
}

impl fmt::Display for ZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZipError::NotAProject(reason) => write!(f, "not a project: {reason}"),
            ZipError::Entry { name, reason } => write!(f, "{name}: {reason}"),
            ZipError::FolderExists(path) => {
                write!(f, "{} holds a project already", path.display())
            }
            ZipError::FileExists(path) => write!(f, "{} is there already", path.display()),
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

fn entry_error(name: &str) -> impl FnOnce(tutti::folder::FolderError) -> ZipError + '_ {
    move |error| ZipError::Entry {
        name: name.to_owned(),
        reason: error.to_string(),
    }
}

/// Packs a project folder into `file`, `<Name>.tutti.zip`: the document first and stored, then
/// the README and every file of the project, deflated; the reserved and private paths left out.
/// The folder is opened and validated first. Written to a temporary name and renamed into
/// place; a file there already is refused. Gives the number of entries.
pub fn pack(folder: &Path, file: &Path) -> Result<usize, ZipError> {
    if file.exists() {
        return Err(ZipError::FileExists(file.to_path_buf()));
    }
    let opened = crate::open(folder).map_err(|e| ZipError::NotAProject(e.to_string()))?;
    let store = DirStore::new(folder);
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
        walk(&store, top, &mut entries)?;
    }

    let temporary = file.with_extension("zip.tmp");
    let out = File::create(&temporary)?;
    let mut writer = ZipWriter::new(BufWriter::new(out));
    let result = (|| -> Result<(), ZipError> {
        for (i, reference) in entries.iter().enumerate() {
            let path = store.path_of(reference).map_err(entry_error(reference))?;
            let size = fs::metadata(&path)?.len();
            let method = if i == 0 {
                CompressionMethod::Stored
            } else {
                CompressionMethod::Deflated
            };
            let options = SimpleFileOptions::default()
                .compression_method(method)
                .large_file(size >= u64::from(u32::MAX));
            writer.start_file(reference.as_str(), options)?;
            let mut input = File::open(&path)?;
            io::copy(&mut input, &mut writer)?;
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&temporary);
        return Err(e);
    }
    let out = writer.finish()?;
    let file_out = out.into_inner().map_err(|e| ZipError::Io(e.into_error()))?;
    file_out.sync_all()?;
    fs::rename(&temporary, file)?;
    Ok(entries.len())
}

/// Every file under `folder`, by reference, added to `out` once; a link is passed by.
fn walk(store: &DirStore, folder: &str, out: &mut Vec<String>) -> Result<(), ZipError> {
    let entries = match store.list(folder) {
        Ok(entries) => entries,
        Err(tutti::folder::FolderError::Missing) => return Ok(()),
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

/// Unpacks `file`, `<Name>.tutti.zip`, into `into`, the project's folder to be: made if it is not
/// there, refused when it holds a project already. Every entry's name is checked before anything
/// is written, and one bad name refuses the whole file; the document is read first, so each
/// indexed entry is held to its indexed length and every other to the limit; an entry that is
/// not the length it declares refuses the file. On a refusal nothing written stays. Gives the
/// number of files written.
pub fn unpack(file: &Path, into: &Path) -> Result<usize, ZipError> {
    let mut archive = ZipArchive::new(BufReader::new(File::open(file)?))?;
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
        if entry.is_dir() {
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

    let store = DirStore::new(into);
    let existed = into.exists();
    if existed && tutti::project::document_in(&store).is_ok() {
        return Err(ZipError::FolderExists(into.to_path_buf()));
    }
    fs::create_dir_all(into)?;
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
        if !existed {
            let _ = fs::remove_dir_all(into);
        }
        return Err(e);
    }
    Ok(written.len())
}

fn too_large(name: &str, declared: u64) -> ZipError {
    ZipError::Entry {
        name: name.to_owned(),
        reason: format!("declares {declared} bytes, more than a project file may be"),
    }
}

/// An entry whole, exactly as long as it declares.
fn read_whole<R: Read + io::Seek>(
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
