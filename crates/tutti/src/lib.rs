//! tutti, an open audio project format.
//!
//! A project is a plain folder holding one JSON document, `<Name>.tutti`, and every file the
//! project uses. This crate is the format's reference implementation: the model, reading a
//! document (any key order, any whitespace), writing one in the canonical pretty form,
//! validation, the time arithmetic the spec defines (ticks to seconds through the tempo map, an
//! audio piece's placement), and [`Project`]: a project in a store, opened, created, saved and
//! filled by the one procedure every writer follows. It has no file IO: documents come in and
//! go out as bytes, and a folder is a trait the host implements ([`folder::Folder`] to read,
//! [`folder::Store`] to write; `tutti-fs` is the disk, [`folder::MemoryStore`] is memory), so
//! the crate builds for `wasm32-unknown-unknown` unchanged.
//!
//! The normative text is `spec/<version>/tutti.md` in the repository; the schema is
//! `schema/<version>/tutti.schema.json`. A number the spec states is a constant here, and a test
//! in `tutti-schema` holds the two equal.

pub mod canon;
pub mod features;
pub mod folder;
pub mod id;
pub mod model;
pub mod names;
pub mod project;
pub mod read;
pub mod render;
pub mod time;
pub mod unknown;
pub mod validate;
pub mod zip;

use serde_json::Value;

pub use id::Id;
pub use model::*;
pub use project::{Incoming, Opened, Project, ProjectError, Saved, Stored, Verified};
pub use read::{ReadError, is_read_only, read};
pub use unknown::{UnknownKey, unknown_keys};
pub use validate::{Issue, Report, Severity, validate};

/// A document as a JSON value, in the spec's key order, defaults left out (spec 4).
pub fn to_value(document: &Document) -> Value {
    serde_json::to_value(document).expect("a document always serialises")
}

/// The pretty form a writer puts on disk (spec 3.1).
pub fn write(document: &Document) -> String {
    canon::pretty(&to_value(document))
}

/// The document's JCS bytes (spec 3.1); [`equal`] compares them with `generator` left out.
pub fn jcs(document: &Document) -> String {
    canon::jcs(&to_value(document))
}

/// Whether two documents are equal as the spec means it (3.1): the same JCS bytes once defaults
/// are applied, `generator` left out. It names the last program that wrote the document, so it
/// changes whenever another program writes the same project back.
pub fn equal(a: &Document, b: &Document) -> bool {
    let without_generator = |document: &Document| {
        let mut value = to_value(document);
        if let Value::Object(map) = &mut value {
            map.remove("generator");
        }
        canon::jcs(&value)
    };
    without_generator(a) == without_generator(b)
}

/// The value of the document's `format` key.
pub const FORMAT: &str = "tutti";

/// The spec version this crate writes, `major.minor`, equal to the schema's and the spec's.
pub const VERSION: &str = "1.0";

/// The major part of [`VERSION`]: a document of a higher major is refused (spec 8.3).
pub const MAJOR: u32 = 1;

/// The `about` sentence a writer puts in the manifest (spec 3.3).
pub const ABOUT: &str = "An open audio project (tutti 1.0): this folder holds every file it uses. \
                         Time is integer ticks, 'ppq' per quarter note, placed in seconds by 'tempo'. \
                         Audio is placed by (frame, tick) points. Keep what you don't understand.";

/// Where this version's normative text is (spec 3.2, `spec`).
pub const SPEC_URL: &str = "https://github.com/mxm-audio/tutti/blob/main/spec/1.0/tutti.md";

/// Where this version's JSON Schema is (spec 3.2, `schema`).
pub const SCHEMA_URL: &str =
    "https://github.com/mxm-audio/tutti/blob/main/schema/1.0/tutti.schema.json";

/// The repository, for a project folder's README.
pub const REPOSITORY_URL: &str = "https://github.com/mxm-audio/tutti";

/// The agent guide, for a project folder's README.
pub const AGENTS_URL: &str = "https://github.com/mxm-audio/tutti/blob/main/docs/agent-guide.md";

/// The README every writer puts in a project folder (spec 2.4), this version's template.
pub const README_TEMPLATE: &str = include_str!("../../../spec/1.0/README.template.md");

/// The README for a project folder (spec 2.4): the template with the title, the generator and
/// the document's file name filled in.
pub fn readme(title: &str, generator: &Generator, document_name: &str) -> String {
    let generator_text = match &generator.version {
        Some(v) => format!("{} {}", generator.name, v),
        None => generator.name.clone(),
    };
    README_TEMPLATE
        .replace("{title}", title)
        .replace("{version}", VERSION)
        .replace("{generator}", &generator_text)
        .replace("{document}", document_name)
        .replace("{spec}", SPEC_URL)
        .replace("{schema}", SCHEMA_URL)
        .replace("{agents}", AGENTS_URL)
        .replace("{repository}", REPOSITORY_URL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_major_dot_minor() {
        let (major, minor) = VERSION.split_once('.').expect("major.minor");
        assert_eq!(major.parse::<u32>().unwrap(), MAJOR);
        assert!(minor.parse::<u32>().is_ok());
        assert_eq!(FORMAT, "tutti");
    }

    #[test]
    fn a_document_round_trips_through_the_pretty_form_keeping_what_it_does_not_know() {
        let text = r#"{
  "format": "tutti", "version": "1.0", "generator": {"name": "t"}, "ppq": 960,
  "tempo": [{"at": 0, "bpm": 87.5, "curve": "hold", "swing": 0.1}],
  "meter": [{"at": 0, "numerator": 4, "denominator": 4}],
  "zeta": {"later": true}, "alpha": 1,
  "tracks": [{"id": "trk_b", "name": "B", "channel": {"gain": 1.0, "pan": 0.0, "mute": false},
    "patterns": [{"id": "pat_1", "length": 3840, "events": [
      {"type": "note", "id": "evt_2", "at": 960, "length": 480, "pitch": 62, "velocity": 0.5, "release": 0.5},
      {"type": "glide", "id": "evt_1", "at": 0, "to": 64},
      {"type": "note", "id": "evt_0", "at": 0, "length": 960, "pitch": 60, "velocity": 1, "extensions": {"dk.mxm.newdawn": {"x": 1}}}
    ]}],
    "parts": [{"id": "prt_1", "pattern": "pat_1", "at": 0, "length": 3840, "offset": 0, "loop": null}]
  }]
}"#;
        let doc = read(text.as_bytes()).unwrap();
        let out = write(&doc);
        let again = read(out.as_bytes()).unwrap();
        assert!(equal(&doc, &again));
        assert_eq!(write(&again), out, "the pretty form is a fixed point");
        // Defaults are left out, unknown keys kept after the known ones, events sorted by id,
        // the unknown event whole, and the extension on the note.
        assert!(!out.contains("\"release\""), "{out}");
        assert!(!out.contains("\"offset\""), "{out}");
        assert!(!out.contains("\"channel\""), "{out}");
        assert!(out.contains("\"swing\": 0.1"), "{out}");
        let alpha = out.find("\"alpha\": 1").unwrap();
        let zeta = out.find("\"zeta\"").unwrap();
        let tracks = out.find("\"tracks\"").unwrap();
        assert!(tracks < alpha && alpha < zeta, "{out}");
        let e0 = out.find("evt_0").unwrap();
        let e1 = out.find("\"type\": \"glide\"").unwrap();
        let e2 = out.find("evt_2").unwrap();
        assert!(e0 < e1 && e1 < e2, "{out}");
        assert!(out.contains("\"dk.mxm.newdawn\": {\"x\": 1}"), "{out}");
        assert!(out.contains("\"bpm\": 87.5"), "{out}");
    }

    #[test]
    fn equality_leaves_the_generator_out_and_nothing_else() {
        let text = r#"{"format": "tutti", "version": "1.0", "generator": {"name": "a", "version": "1"},
  "ppq": 960, "tempo": [{"at": 0, "bpm": 120}], "meter": [{"at": 0, "numerator": 4, "denominator": 4}]}"#;
        let a = read(text.as_bytes()).unwrap();
        let mut b = a.clone();
        b.generator = Generator::new("b", "2", "https://example.com/b");
        assert!(equal(&a, &b), "another writer's name");
        b.ppq = 480;
        assert!(!equal(&a, &b), "anything else");
    }
}
