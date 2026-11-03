use tutti::folder::Kind;
use tutti::{Document, Id};

use crate::{Error, Progress, Report, Source};

/// A project format tutti converts from, into, or both. One value per format, shared across
/// threads: a watcher holds a list of them and asks each about every folder and file it finds.
pub trait Format: Send + Sync {
    /// Stable for ever: written in records (`polyend-tracker`).
    fn id(&self) -> &'static str;

    /// Shown to the user (`Polyend Tracker project`).
    fn name(&self) -> &'static str;

    /// Raised whenever the format's output changes, so outputs written by an earlier version
    /// are out of date and converted again.
    fn version(&self) -> u32;

    /// Which ways it converts.
    fn directions(&self) -> Directions;

    /// Whether the entry at `at` (a folder or a file, as `kind` says; `""` the source's root) is
    /// a project of this format.
    fn detect(&self, source: &dyn Source, at: &str, kind: Kind) -> Result<Option<Found>, Error>;

    /// Every file a conversion of `found` reads, by reference: what a watcher hashes to tell a
    /// changed project.
    fn inputs(&self, source: &dyn Source, found: &Found) -> Result<Vec<String>, Error>;

    /// The project converted into tutti.
    fn to_tutti(
        &self,
        source: &dyn Source,
        found: &Found,
        progress: &mut dyn Progress,
    ) -> Result<Conversion, Error> {
        let _ = (source, found, progress);
        Err(Error::Unsupported {
            format: self.id(),
            direction: "into tutti",
        })
    }

    /// A tutti project converted into this format: `document` read from `project`, whose files
    /// the export may copy.
    // The pair of `to_tutti`, a conversion out of tutti that the format performs, not a
    // constructor, which is what the lint expects a `from_` method to be.
    #[allow(clippy::wrong_self_convention)]
    fn from_tutti(
        &self,
        document: &Document,
        project: &dyn Source,
        progress: &mut dyn Progress,
    ) -> Result<Export, Error> {
        let _ = (document, project, progress);
        Err(Error::Unsupported {
            format: self.id(),
            direction: "out of tutti",
        })
    }
}

/// Which ways a format converts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Directions {
    pub to_tutti: bool,
    pub from_tutti: bool,
}

/// A project a format recognised: where it is in the source (a folder or a file) and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub at: String,
    pub name: String,
}

/// A project converted into tutti, not yet written: the document, the files it refers to and
/// where each goes, and the report. [`write`](crate::write) puts it into a store.
#[derive(Clone, Debug)]
pub struct Conversion {
    /// The project's name; the document is `<name>.tutti`, cleaned as the core cleans a title.
    pub name: String,
    pub document: Document,
    /// Written in this order.
    pub files: Vec<Put>,
    pub report: Report,
}

/// One file of a conversion and where it goes.
#[derive(Clone, Debug)]
pub struct Put {
    pub place: Place,
    pub contents: Contents,
}

/// Where a file of a conversion goes, in the tutti project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// `audio/<file>`, exactly as the document's audio pieces refer to it.
    Audio { file: String },
    /// A device's own file (spec 7.8), `plugins/files/<device>/<path>`.
    DeviceFile { device: Id, path: String },
    /// A plugin state (spec 7.4), `plugins/states/<sha256>.<kind>`, named by its content.
    State { kind: String },
}

/// A file's contents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Contents {
    /// Bytes the format built (a WAV around PCM taken from inside another file), held whole.
    Bytes(Vec<u8>),
    /// A file of the source, streamed as it is.
    Copy { from: String },
}

/// A tutti project converted into another format: its files by path, new, and the report.
#[derive(Clone, Debug)]
pub struct Export {
    pub files: Vec<(String, Contents)>,
    pub report: Report,
}
