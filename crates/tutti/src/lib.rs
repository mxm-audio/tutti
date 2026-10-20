//! tutti, an open audio project format.
//!
//! A project is a plain folder holding one JSON document, `<Name>.tutti`, and every file the
//! project uses. This crate is the format's reference implementation: the model, reading a
//! document (any key order, any whitespace), writing one in the canonical pretty form,
//! validation, and the time arithmetic the spec defines (ticks to seconds through the tempo map,
//! an audio piece's placement). It has no file IO: documents come in and go out as bytes, and
//! anything that lists or reads a folder goes through a trait the caller implements, so the crate
//! builds for `wasm32-unknown-unknown` unchanged.
//!
//! The normative text is `spec/<version>/tutti.md` in the repository; the schema is
//! `schema/<version>/tutti.schema.json`. A number the spec states is a constant here, and a test
//! in `tutti-schema` holds the two equal.

/// The value of the document's `format` key.
pub const FORMAT: &str = "tutti";

/// The spec version this crate writes, `major.minor`, equal to the schema's and the spec's.
pub const VERSION: &str = "1.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_major_dot_minor() {
        let (major, minor) = VERSION.split_once('.').expect("major.minor");
        assert!(major.parse::<u32>().is_ok() && minor.parse::<u32>().is_ok());
        assert_eq!(FORMAT, "tutti");
    }
}
