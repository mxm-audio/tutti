//! A project as one file on disk, `<Name>.tutti.zip` (spec 2.5): packing a project folder into a
//! file and unpacking a file into a folder. What is packed, and what a reader refuses, is
//! `tutti-zip`'s, the same for a file and for bytes in memory; this is the disk around it: the
//! file written under a temporary name and renamed into place, nothing overwritten, and the
//! folder being made removed again on a refusal.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::{Path, PathBuf};

use crate::DirStore;

/// Why a project could not be packed or unpacked.
#[derive(Debug)]
pub enum ZipError {
    /// What `tutti-zip` refused: not a project, or an entry a reader refuses.
    Archive(tutti_zip::ZipError),
    /// Unpacking would land in a folder that holds a project already.
    FolderExists(PathBuf),
    /// The file to write is there already: nothing is overwritten.
    FileExists(PathBuf),
    Io(io::Error),
}

impl fmt::Display for ZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZipError::Archive(e) => write!(f, "{e}"),
            ZipError::FolderExists(path) => {
                write!(f, "{} holds a project already", path.display())
            }
            ZipError::FileExists(path) => write!(f, "{} is there already", path.display()),
            ZipError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ZipError {}

impl From<io::Error> for ZipError {
    fn from(e: io::Error) -> Self {
        ZipError::Io(e)
    }
}

impl From<tutti_zip::ZipError> for ZipError {
    fn from(e: tutti_zip::ZipError) -> Self {
        ZipError::Archive(e)
    }
}

/// Packs a project folder into `file`, `<Name>.tutti.zip` (`tutti_zip::pack`), streamed from
/// the folder. Written to a temporary name and renamed into place; a file there already is
/// refused. Gives the number of entries.
pub fn pack(folder: &Path, file: &Path) -> Result<usize, ZipError> {
    if file.exists() {
        return Err(ZipError::FileExists(file.to_path_buf()));
    }
    let temporary = file.with_extension("zip.tmp");
    let out = BufWriter::new(File::create(&temporary)?);
    let packed = tutti_zip::pack(&DirStore::new(folder), out);
    let (out, entries) = match packed {
        Ok(packed) => packed,
        Err(e) => {
            let _ = fs::remove_file(&temporary);
            return Err(e.into());
        }
    };
    let file_out = out.into_inner().map_err(|e| ZipError::Io(e.into_error()))?;
    file_out.sync_all()?;
    fs::rename(&temporary, file)?;
    Ok(entries)
}

/// Unpacks `file`, `<Name>.tutti.zip`, into `into`, the project's folder to be
/// (`tutti_zip::unpack`): made if it is not there, refused when it holds a project already. On a
/// refusal nothing written stays, and a folder made for it goes. Gives the number of files
/// written.
pub fn unpack(file: &Path, into: &Path) -> Result<usize, ZipError> {
    let input = BufReader::new(File::open(file)?);
    let existed = into.exists();
    fs::create_dir_all(into)?;
    match tutti_zip::unpack(input, &DirStore::new(into)) {
        Ok(written) => Ok(written),
        Err(e) => {
            if !existed {
                let _ = fs::remove_dir_all(into);
            }
            Err(match e {
                tutti_zip::ZipError::HoldsProject => ZipError::FolderExists(into.to_path_buf()),
                e => e.into(),
            })
        }
    }
}
