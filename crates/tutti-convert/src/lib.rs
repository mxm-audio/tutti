//! Converting projects of other formats into tutti and out of it.
//!
//! A format implements [`Format`]: it recognises its projects ([`Format::detect`]), names the
//! files a conversion reads ([`Format::inputs`]), and converts into tutti
//! ([`Format::to_tutti`], giving a [`Conversion`]) or out of it ([`Format::from_tutti`], giving
//! an [`Export`]), each with a [`Report`] of what did not map. It reads through a [`Source`],
//! which every core `Store` is, and a [`Recording`] notes exactly what was read, for a watcher's
//! record. [`write`] puts a conversion into a store through the core's `Project`, the procedure
//! every writer follows, after checking that every file lands where the document refers to it.
//!
//! No file IO and no platform code: a source and a store are traits the host implements
//! (`tutti-fs` on a disk, `MemoryStore` in memory), so the crate, and every format built on it,
//! builds for the web too.

mod error;
mod format;
mod ids;
mod report;
mod source;
mod write;

pub use error::Error;
pub use format::{Contents, Conversion, Directions, Export, Format, Found, Place, Put};
pub use ids::{derived_id, fingerprint};
pub use report::{Progress, Quiet, Remark, RemarkKind, Report};
pub use source::{Recording, Served, Source};
pub use write::{write, write_export};
