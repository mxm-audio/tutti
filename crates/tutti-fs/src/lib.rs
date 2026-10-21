//! tutti on disk: [`DirStore`], the core's `Store` over `std::fs`, and the one-call entry points
//! an implementer uses, [`open`] and [`create`], each giving a `Project` that does the rest. The
//! one place in tutti's crates with file IO: the core has none, so it builds for the web, and
//! every program on a disk goes through here rather than writing the spec's procedure itself.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tutti::folder::{Entry, Folder, FolderError, Kind, Store};
use tutti::{Generator, Opened, Project, ProjectError};

/// How much of a file a hash reads at a time.
const CHUNK: usize = 1 << 20;

/// A project folder on disk: references resolved inside it, each part checked, links refused
/// (spec 9). A cheap handle: clone it for a recording thread.
#[derive(Clone, Debug)]
pub struct DirStore {
    root: PathBuf,
}

impl DirStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The path of a reference (`""` the root): every part a plain name, no part a link. The
    /// last part need not exist yet.
    pub fn path_of(&self, reference: &str) -> Result<PathBuf, FolderError> {
        let mut path = self.root.clone();
        if reference.is_empty() {
            return Ok(path);
        }
        for part in reference.split('/') {
            if part.is_empty()
                || part == "."
                || part == ".."
                || part.contains('\\')
                || part.contains('\0')
            {
                return Err(FolderError::Other(format!("{reference}: not a reference")));
            }
            path.push(part);
            if let Ok(meta) = fs::symlink_metadata(&path) {
                if meta.file_type().is_symlink() {
                    return Err(FolderError::Link);
                }
            }
        }
        Ok(path)
    }

    fn parents_of(&self, reference: &str) -> Result<PathBuf, FolderError> {
        let path = self.path_of(reference)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_error)?;
        }
        Ok(path)
    }
}

fn io_error(e: io::Error) -> FolderError {
    match e.kind() {
        io::ErrorKind::NotFound => FolderError::Missing,
        io::ErrorKind::AlreadyExists => FolderError::Exists,
        _ => FolderError::Other(e.to_string()),
    }
}

impl Folder for DirStore {
    fn read(&self, reference: &str) -> Result<Vec<u8>, FolderError> {
        fs::read(self.path_of(reference)?).map_err(io_error)
    }

    fn size(&self, reference: &str) -> Result<u64, FolderError> {
        fs::metadata(self.path_of(reference)?)
            .map(|m| m.len())
            .map_err(io_error)
    }

    /// Streamed: a file is never held whole.
    fn sha256(&self, reference: &str) -> Result<(String, u64), FolderError> {
        let file = File::open(self.path_of(reference)?).map_err(io_error)?;
        let mut reader = BufReader::with_capacity(CHUNK, file);
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; CHUNK];
        let mut length = 0u64;
        loop {
            let n = reader.read(&mut buffer).map_err(io_error)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            length += n as u64;
        }
        Ok((format!("{:x}", hasher.finalize()), length))
    }
}

impl Store for DirStore {
    type Writer = BufWriter<File>;

    fn list(&self, folder: &str) -> Result<Vec<Entry>, FolderError> {
        let entries = fs::read_dir(self.path_of(folder)?).map_err(io_error)?;
        let mut out = Vec::new();
        for entry in entries {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let kind = match fs::symlink_metadata(entry.path()) {
                Ok(meta) if meta.file_type().is_symlink() => Kind::Other,
                Ok(meta) if meta.is_dir() => Kind::Folder,
                Ok(meta) if meta.is_file() => Kind::File,
                _ => Kind::Other,
            };
            out.push(Entry { name, kind });
        }
        Ok(out)
    }

    fn exists(&self, reference: &str) -> Result<bool, FolderError> {
        match fs::symlink_metadata(self.path_of(reference)?) {
            Ok(meta) if meta.file_type().is_symlink() => Err(FolderError::Link),
            Ok(meta) => Ok(meta.is_file()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io_error(e)),
        }
    }

    fn make_folder(&self, folder: &str) -> Result<(), FolderError> {
        fs::create_dir_all(self.path_of(folder)?).map_err(io_error)
    }

    fn write_new(&self, reference: &str, bytes: &[u8]) -> Result<(), FolderError> {
        let path = self.parents_of(reference)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(io_error)?;
        file.write_all(bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)
    }

    fn open_new(&self, reference: &str) -> Result<Self::Writer, FolderError> {
        let path = self.parents_of(reference)?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(io_error)?;
        Ok(BufWriter::with_capacity(CHUNK, file))
    }

    fn close(&self, mut writer: Self::Writer) -> Result<(), FolderError> {
        writer.flush().map_err(io_error)?;
        let file = writer
            .into_inner()
            .map_err(|e| FolderError::Other(e.to_string()))?;
        file.sync_all().map_err(io_error)
    }

    fn rename_over(&self, from: &str, to: &str) -> Result<(), FolderError> {
        let from_path = self.path_of(from)?;
        let to_path = self.path_of(to)?;
        fs::rename(&from_path, &to_path).map_err(io_error)?;
        #[cfg(unix)]
        if let Some(parent) = to_path.parent() {
            if let Ok(dir) = File::open(parent) {
                let _ = dir.sync_all();
            }
        }
        Ok(())
    }

    fn remove(&self, reference: &str) -> Result<(), FolderError> {
        fs::remove_file(self.path_of(reference)?).map_err(io_error)
    }
}

/// Opens the project in a folder: its one document, read and validated, with the report and
/// whether it is read-only on this version.
pub fn open(folder: &Path) -> Result<Opened<DirStore>, ProjectError> {
    Project::open(DirStore::new(folder))
}

/// Creates a project in a folder that holds no document (the folder is made if it is not
/// there): the document named after the title, saved at once with its README.
pub fn create(
    folder: &Path,
    title: &str,
    generator: &Generator,
) -> Result<Project<DirStore>, ProjectError> {
    Project::create(DirStore::new(folder), title, generator)
}

/// The path of the one `.tutti` document in a folder (spec 3.1).
pub fn document_in(folder: &Path) -> Result<PathBuf, ProjectError> {
    tutti::project::document_in(&DirStore::new(folder)).map(|name| folder.join(name))
}
