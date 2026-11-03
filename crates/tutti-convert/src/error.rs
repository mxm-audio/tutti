use std::fmt;

use tutti::ProjectError;
use tutti::folder::FolderError;

/// Why a conversion, or writing one, did not happen.
#[derive(Debug)]
pub enum Error {
    /// A file of the source could not be read.
    Source {
        reference: String,
        error: FolderError,
    },
    /// A file the format cannot read: damaged, or of a version the format does not know.
    Unreadable { reference: String, why: String },
    /// The format does not convert this way.
    Unsupported {
        format: &'static str,
        direction: &'static str,
    },
    /// A file gave different bytes at two reads during one conversion: it changed while it was
    /// being converted, so nothing made from it may be recorded as made from either.
    Changed(String),
    /// The conversion was stopped through its [`Progress`](crate::Progress).
    Cancelled,
    /// The store a conversion was to be written into holds a file outside `private/`.
    OutputNotEmpty(String),
    /// A conversion's file that would not land where the document refers to it, or that nothing
    /// in the document refers to. Found before anything is written.
    Placement { what: String, why: String },
    /// A store refused a write.
    Store {
        reference: String,
        error: FolderError,
    },
    /// The project's own refusal (`tutti::Project`).
    Project(ProjectError),
}

impl Error {
    pub(crate) fn source(reference: &str) -> impl Fn(FolderError) -> Error + '_ {
        move |error| Error::Source {
            reference: reference.to_owned(),
            error,
        }
    }

    pub(crate) fn store(reference: &str) -> impl Fn(FolderError) -> Error + '_ {
        move |error| Error::Store {
            reference: reference.to_owned(),
            error,
        }
    }

    pub(crate) fn placement(what: impl Into<String>, why: impl Into<String>) -> Error {
        Error::Placement {
            what: what.into(),
            why: why.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Source { reference, error } => write!(f, "{reference}: {error}"),
            Error::Unreadable { reference, why } => write!(f, "{reference} can't be read: {why}"),
            Error::Unsupported { format, direction } => {
                write!(f, "{format} does not convert {direction}")
            }
            Error::Changed(reference) => {
                write!(f, "{reference} changed while it was being converted")
            }
            Error::Cancelled => write!(f, "stopped"),
            Error::OutputNotEmpty(reference) => {
                write!(f, "the output already holds {reference}")
            }
            Error::Placement { what, why } => write!(f, "{what}: {why}"),
            Error::Store { reference, error } => write!(f, "{reference}: {error}"),
            Error::Project(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<ProjectError> for Error {
    fn from(error: ProjectError) -> Self {
        Error::Project(error)
    }
}
