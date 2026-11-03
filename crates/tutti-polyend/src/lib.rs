//! Polyend Tracker projects into tutti.
//!
//! [`files`] reads the Tracker's files byte for byte and writes them back exactly; [`model`]
//! reads a project folder into a [`Model`]; [`song`] works out the song as the device plays it
//! (steps to ticks, the song's order, note lengths across patterns, tempo changes); [`convert`]
//! makes the tutti project, keeping everything tutti does not hold under the
//! `dk.mxm.polyend-tracker` extension, and reads a model back from it ([`to_model`]).
//! [`PolyendTracker`] is the [`Format`] a watcher lists.

pub mod convert;
pub mod files;
pub mod model;
pub mod song;

use tutti::folder::{FolderError, Kind};
use tutti_convert::{Conversion, Directions, Error, Format, Found, Progress, Source};

pub use convert::{NAMESPACE, to_model};
pub use model::{Locator, Locators, Model, read};

/// Polyend Tracker projects: a folder holding `project.mt`, as the Tracker keeps them under
/// `Projects/` on its card.
pub struct PolyendTracker;

impl Format for PolyendTracker {
    fn id(&self) -> &'static str {
        "polyend-tracker"
    }

    fn name(&self) -> &'static str {
        "Polyend Tracker project"
    }

    fn version(&self) -> u32 {
        convert::VERSION
    }

    fn directions(&self) -> Directions {
        Directions {
            to_tutti: true,
            from_tutti: false,
        }
    }

    fn detect(&self, source: &dyn Source, at: &str, kind: Kind) -> Result<Option<Found>, Error> {
        if kind != Kind::Folder {
            return Ok(None);
        }
        let project = if at.is_empty() {
            "project.mt".to_owned()
        } else {
            format!("{at}/project.mt")
        };
        match source.size(&project) {
            Ok(_) => {}
            Err(FolderError::Missing) => return Ok(None),
            Err(error) => {
                return Err(Error::Source {
                    reference: project,
                    error,
                });
            }
        }
        let (parent, name) = at.rsplit_once('/').unwrap_or(("", at));
        // The card's `Workspace/` holds the project open on the device, not a saved project.
        if name == "Workspace" {
            let beside = source.entries(parent).unwrap_or_default();
            if beside
                .iter()
                .any(|e| e.name == "Projects" && e.kind == Kind::Folder)
            {
                return Ok(None);
            }
        }
        Ok(Some(Found {
            at: at.to_owned(),
            name: if name.is_empty() { "Project" } else { name }.to_owned(),
        }))
    }

    fn inputs(&self, source: &dyn Source, found: &Found) -> Result<Vec<String>, Error> {
        model::inputs(source, &found.at)
    }

    fn to_tutti(
        &self,
        source: &dyn Source,
        found: &Found,
        progress: &mut dyn Progress,
    ) -> Result<Conversion, Error> {
        convert::to_tutti(source, found, progress)
    }
}
