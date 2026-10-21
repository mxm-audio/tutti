//! The core for JavaScript: thin functions over `tutti`, strings in and strings out, so the
//! npm package (`js/`) wraps them with plain objects, and `Project` over a store the host
//! supplies (`project`). No file IO here: the host's store reads and writes files. Names mirror
//! the Rust API.

mod project;

use std::collections::HashMap;

pub use project::{Incoming, Project};

use serde_json::{Value, json};
use tutti::folder::{Folder, FolderError, check_files};
use tutti::time::{Placement, TempoMap};
use wasm_bindgen::prelude::*;

/// The format's version this build writes.
#[wasm_bindgen]
pub fn version() -> String {
    tutti::VERSION.to_owned()
}

/// The `about` sentence a writer puts in the manifest.
#[wasm_bindgen]
pub fn about() -> String {
    tutti::ABOUT.to_owned()
}

fn read(text: &str) -> Result<tutti::Document, JsError> {
    tutti::read(text.as_bytes()).map_err(|e| JsError::new(&e.to_string()))
}

/// Reads a document in any form and returns it in the pretty form (keys in order, defaults
/// left out, unknown data kept); throws with the reader's reason when it is refused.
#[wasm_bindgen]
pub fn fmt(text: &str) -> Result<String, JsError> {
    Ok(tutti::write(&read(text)?))
}

/// The document's JCS bytes; `equal` compares them with `generator` left out.
#[wasm_bindgen]
pub fn jcs(text: &str) -> Result<String, JsError> {
    Ok(tutti::jcs(&read(text)?))
}

/// Whether two documents are equal as the spec means it: `generator` left out.
#[wasm_bindgen]
pub fn equal(a: &str, b: &str) -> Result<bool, JsError> {
    Ok(tutti::equal(&read(a)?, &read(b)?))
}

/// Whether a document opens read-only on this version (a later `min_version`).
#[wasm_bindgen]
pub fn is_read_only(text: &str) -> Result<bool, JsError> {
    Ok(tutti::is_read_only(&read(text)?))
}

/// Validates a document: a JSON array of `{severity, pointer, message}`, errors and warnings.
#[wasm_bindgen]
pub fn validate(text: &str) -> Result<String, JsError> {
    let report = tutti::validate(&read(text)?);
    Ok(issues_json(&report.issues))
}

/// The keys the document carries that this version doesn't know, once each: a JSON array of
/// `{key, pointer, count}`, the pointer the first place it is in.
#[wasm_bindgen]
pub fn unknown_keys(text: &str) -> Result<String, JsError> {
    let items: Vec<Value> = tutti::unknown_keys(&read(text)?)
        .into_iter()
        .map(|k| json!({"key": k.key, "pointer": k.pointer, "count": k.count}))
        .collect();
    Ok(Value::Array(items).to_string())
}

pub(crate) fn issues_json(issues: &[tutti::Issue]) -> String {
    let items: Vec<Value> = issues
        .iter()
        .map(|i| {
            json!({
                "severity": match i.severity { tutti::Severity::Error => "error", tutti::Severity::Warning => "warning" },
                "pointer": i.pointer,
                "message": i.message,
            })
        })
        .collect();
    Value::Array(items).to_string()
}

/// A folder the host described: reference to `{sha256, bytes}`, hashes the host computed.
struct ProvidedFolder {
    entries: HashMap<String, (String, u64)>,
}

impl Folder for ProvidedFolder {
    fn read(&self, reference: &str) -> Result<Vec<u8>, FolderError> {
        Err(FolderError::Other(format!(
            "{reference}: the host did not provide the bytes"
        )))
    }

    fn size(&self, reference: &str) -> Result<u64, FolderError> {
        self.entries
            .get(reference)
            .map(|(_, b)| *b)
            .ok_or(FolderError::Missing)
    }

    fn sha256(&self, reference: &str) -> Result<(String, u64), FolderError> {
        self.entries
            .get(reference)
            .cloned()
            .ok_or(FolderError::Missing)
    }
}

/// Checks the files index against what the host found in the folder: `files` is a JSON object
/// of reference to `{"sha256": …, "bytes": …}` for every file in `audio/`, `plugins/states/` and
/// `renders/`. Returns the issues as `validate` does.
#[wasm_bindgen]
pub fn check_files_with(text: &str, files: &str) -> Result<String, JsError> {
    let doc = read(text)?;
    let provided: HashMap<String, Value> =
        serde_json::from_str(files).map_err(|e| JsError::new(&e.to_string()))?;
    let entries = provided
        .into_iter()
        .filter_map(|(k, v)| {
            let sha = v.get("sha256")?.as_str()?.to_owned();
            let bytes = v.get("bytes")?.as_u64()?;
            Some((k, (sha, bytes)))
        })
        .collect();
    Ok(issues_json(&check_files(&doc, &ProvidedFolder { entries })))
}

/// The features a document uses, as a JSON array of `{name, level}`.
#[wasm_bindgen]
pub fn features(text: &str) -> Result<String, JsError> {
    let doc = read(text)?;
    let items: Vec<Value> = tutti::features::features(&doc)
        .iter()
        .map(|f| json!({"name": f.name, "level": f.level}))
        .collect();
    Ok(Value::Array(items).to_string())
}

/// The lowest conformance level a reader needs to play the document in full.
#[wasm_bindgen]
pub fn level_needed(text: &str) -> Result<u8, JsError> {
    Ok(tutti::features::level_needed(&read(text)?))
}

/// Every file the document refers to, as a JSON array of references.
#[wasm_bindgen]
pub fn referenced_files(text: &str) -> Result<String, JsError> {
    Ok(Value::Array(
        tutti::folder::referenced_files(&read(text)?)
            .into_iter()
            .map(Value::String)
            .collect(),
    )
    .to_string())
}

fn tempo_map(ppq: u32, tempo: &str) -> Result<TempoMap, JsError> {
    let points: Vec<tutti::TempoPoint> =
        serde_json::from_str(tempo).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(TempoMap::new(ppq, &points))
}

/// Seconds from tick 0 to a tick, through a tempo map given as its JSON array (spec 5.2).
#[wasm_bindgen]
pub fn seconds_at(ppq: u32, tempo: &str, tick: f64) -> Result<f64, JsError> {
    Ok(tempo_map(ppq, tempo)?.seconds_at(tick))
}

/// The tick at a number of seconds from tick 0.
#[wasm_bindgen]
pub fn tick_at(ppq: u32, tempo: &str, seconds: f64) -> Result<f64, JsError> {
    Ok(tempo_map(ppq, tempo)?.tick_at(seconds))
}

/// The sample a tick falls on at a rate (`round_half_up(seconds · rate)`).
#[wasm_bindgen]
pub fn samples_at(ppq: u32, tempo: &str, tick: f64, rate: u32) -> Result<f64, JsError> {
    Ok(tempo_map(ppq, tempo)?.samples_at(tick, rate) as f64)
}

/// The tempo in force at a tick.
#[wasm_bindgen]
pub fn bpm_at(ppq: u32, tempo: &str, tick: f64) -> Result<f64, JsError> {
    Ok(tempo_map(ppq, tempo)?.bpm_at(tick))
}

fn placement(ppq: u32, rate: u32, map: &str, bpm: Option<f64>) -> Result<Placement, JsError> {
    let points: Vec<tutti::MapPoint> =
        serde_json::from_str(map).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(Placement::new(ppq, rate, &points, bpm))
}

/// The pattern tick a source frame sits at (spec 5.5), or `undefined` for an unwarped piece
/// (`bpm` null).
#[wasm_bindgen]
pub fn tick_of_frame(
    ppq: u32,
    rate: u32,
    map: &str,
    bpm: Option<f64>,
    frame: f64,
) -> Result<Option<f64>, JsError> {
    Ok(placement(ppq, rate, map, bpm)?.tick_of_frame(frame))
}

/// The source frame at a pattern tick, or `undefined` for an unwarped piece.
#[wasm_bindgen]
pub fn frame_at_tick(
    ppq: u32,
    rate: u32,
    map: &str,
    bpm: Option<f64>,
    tick: f64,
) -> Result<Option<f64>, JsError> {
    Ok(placement(ppq, rate, map, bpm)?.frame_at_tick(tick))
}

/// Source frames per output second at a tick, given the project's bpm there.
#[wasm_bindgen]
pub fn rate_at(
    ppq: u32,
    rate: u32,
    map: &str,
    bpm: Option<f64>,
    tick: f64,
    project_bpm: f64,
) -> Result<f64, JsError> {
    Ok(placement(ppq, rate, map, bpm)?.rate_at(tick, project_bpm))
}

/// A render's fingerprint for a track of the document (spec 7.7).
#[wasm_bindgen]
pub fn render_fingerprint(
    text: &str,
    track_id: &str,
    from: f64,
    to: f64,
) -> Result<String, JsError> {
    let doc = read(text)?;
    let track = doc
        .all_tracks()
        .into_iter()
        .find(|t| t.id.as_str() == track_id)
        .ok_or_else(|| JsError::new("no such track"))?;
    Ok(tutti::render::render_fingerprint(
        &doc,
        track,
        from as i64,
        to as i64,
    ))
}

/// SHA-256 of bytes, 64 lower-case hexadecimal digits.
#[wasm_bindgen]
pub fn sha256_hex(bytes: &[u8]) -> String {
    tutti::render::sha256_hex(bytes)
}

/// An id in the recommended form: a kind prefix and a ULID from a Unix time in milliseconds
/// and ten random bytes the host supplies.
#[wasm_bindgen]
pub fn ulid(prefix: &str, unix_millis: f64, random: &[u8]) -> String {
    let mut r: u128 = 0;
    for b in random.iter().take(10) {
        r = (r << 8) | *b as u128;
    }
    tutti::Id::ulid(prefix, unix_millis as u64, r).0
}

/// Whether a text is an id of the spec's form.
#[wasm_bindgen]
pub fn is_id(text: &str) -> bool {
    tutti::id::check(text).is_ok()
}

/// The README for a project folder: the template with the title, the generator (`{name,
/// version?, url?}` as JSON) and the document's file name filled in.
#[wasm_bindgen]
pub fn readme(title: &str, generator: &str, document_name: &str) -> Result<String, JsError> {
    let generator: tutti::Generator =
        serde_json::from_str(generator).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(tutti::readme(title, &generator, document_name))
}

/// The smallest valid document, in the pretty form, written by `generator` (JSON).
#[wasm_bindgen]
pub fn new_document(generator: &str) -> Result<String, JsError> {
    let generator: tutti::Generator =
        serde_json::from_str(generator).map_err(|e| JsError::new(&e.to_string()))?;
    Ok(tutti::write(&tutti::Document::new(generator)))
}
