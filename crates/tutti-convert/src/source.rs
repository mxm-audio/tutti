use std::collections::BTreeMap;
use std::io;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tutti::folder::{Entry, Folder, FolderError, Store};
use tutti::render::sha256_hex;

use crate::Error;

/// A project's files as a converter reads them: what a [`Folder`] gives (a file whole, its size,
/// its hash, its time, a streamed copy) and the entries of a folder. Read-only by its type.
/// Every core [`Store`] is one, so `tutti_fs::DirStore` over a folder on disk and `MemoryStore`
/// in a test or a browser serve as they are; unlike `Store` it can be a trait object.
///
/// Its listing is `entries`, not `list`, so code with `Store` in scope never meets two methods
/// of one name.
pub trait Source: Folder {
    /// The entries directly in `folder` (`""` the root), in any order; `Missing` when the
    /// folder is not there.
    fn entries(&self, folder: &str) -> Result<Vec<Entry>, FolderError>;
}

impl<S: Store> Source for S {
    fn entries(&self, folder: &str) -> Result<Vec<Entry>, FolderError> {
        Store::list(self, folder)
    }
}

/// A file as a conversion read it: the SHA-256 and size of the very bytes read or copied, and
/// the file's time when it was first read, as the source tells it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Served {
    pub sha256: String,
    pub size: u64,
    #[serde(default)]
    pub modified: Option<u64>,
}

/// A source that records what it serves: every file read, hashed or copied, by the bytes that
/// passed, so a watcher records the inputs a conversion was made from exactly, whatever happens
/// to the files after. A file that gives different bytes at a second read is refused there, and
/// [`Recording::served`] then gives [`Error::Changed`].
pub struct Recording<'a> {
    inner: &'a dyn Source,
    served: Mutex<BTreeMap<String, Served>>,
    changed: Mutex<Option<String>>,
}

impl<'a> Recording<'a> {
    pub fn new(inner: &'a dyn Source) -> Self {
        Self {
            inner,
            served: Mutex::new(BTreeMap::new()),
            changed: Mutex::new(None),
        }
    }

    /// Every file served, once each, by reference; [`Error::Changed`] naming the first file
    /// that gave different bytes at two reads.
    pub fn served(&self) -> Result<BTreeMap<String, Served>, Error> {
        if let Some(reference) = lock(&self.changed).clone() {
            return Err(Error::Changed(reference));
        }
        Ok(lock(&self.served).clone())
    }

    fn note(&self, reference: &str, sha256: String, size: u64) -> Result<(), FolderError> {
        let mut served = lock(&self.served);
        match served.get(reference) {
            Some(s) if s.sha256 == sha256 && s.size == size => Ok(()),
            Some(_) => {
                lock(&self.changed).get_or_insert_with(|| reference.to_owned());
                Err(FolderError::Other(format!(
                    "{reference} changed while it was being read"
                )))
            }
            None => {
                let modified = self.inner.modified(reference).unwrap_or(None);
                served.insert(
                    reference.to_owned(),
                    Served {
                        sha256,
                        size,
                        modified,
                    },
                );
                Ok(())
            }
        }
    }
}

impl Folder for Recording<'_> {
    fn read(&self, reference: &str) -> Result<Vec<u8>, FolderError> {
        let bytes = self.inner.read(reference)?;
        self.note(reference, sha256_hex(&bytes), bytes.len() as u64)?;
        Ok(bytes)
    }

    fn size(&self, reference: &str) -> Result<u64, FolderError> {
        self.inner.size(reference)
    }

    fn sha256(&self, reference: &str) -> Result<(String, u64), FolderError> {
        let (sha256, size) = self.inner.sha256(reference)?;
        self.note(reference, sha256.clone(), size)?;
        Ok((sha256, size))
    }

    fn modified(&self, reference: &str) -> Result<Option<u64>, FolderError> {
        self.inner.modified(reference)
    }

    fn copy_to(&self, reference: &str, out: &mut dyn io::Write) -> Result<u64, FolderError> {
        let mut hashing = Hashing {
            out,
            hasher: Sha256::new(),
        };
        let size = self.inner.copy_to(reference, &mut hashing)?;
        let sha256 = hex(&hashing.hasher.finalize());
        self.note(reference, sha256, size)?;
        Ok(size)
    }
}

impl Source for Recording<'_> {
    fn entries(&self, folder: &str) -> Result<Vec<Entry>, FolderError> {
        self.inner.entries(folder)
    }
}

/// A writer that hashes what passes through it.
struct Hashing<'w> {
    out: &'w mut dyn io::Write,
    hasher: Sha256,
}

impl io::Write for Hashing<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.out.write(buf)?;
        self.hasher.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}
