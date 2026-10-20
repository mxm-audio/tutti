//! The model: the types a document deserialises into, in the spec's key order, every object
//! keeping the keys it does not know (`unknown`) and its `extensions`. Defaults are the spec's
//! (section 4): an absent key reads as its default, and a key at its default is left out when
//! written. Nothing here checks a value against its bounds: that is `validate`'s, so that a
//! document with a fault still loads and reports it.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canon::utf16_cmp;
use crate::id::Id;

/// A key of an unknown field, ordered by UTF-16 code units, so unknown keys are written in JCS
/// order after the known ones.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Key(pub String);

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        utf16_cmp(&self.0, &other.0)
    }
}

/// The keys an object carried that this version does not know: kept and written back (spec 8.2).
pub type Unknown = BTreeMap<Key, Value>;

/// Vendor data by namespace (spec 8.1).
pub type Extensions = BTreeMap<String, Value>;

/// Ticks: integer musical time at `ppq` per quarter note (spec 5.2).
pub type Ticks = i64;

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_true(b: &bool) -> bool {
    *b
}

fn r#true() -> bool {
    true
}

fn is_zero(x: &f64) -> bool {
    *x == 0.0
}

fn is_one(x: &f64) -> bool {
    *x == 1.0
}

fn one() -> f64 {
    1.0
}

fn is_zero_ticks(t: &Ticks) -> bool {
    *t == 0
}

fn is_empty_string(s: &str) -> bool {
    s.is_empty()
}

// --- the document ----------------------------------------------------------------------------

/// A whole project: the manifest first, then the music (spec 3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub format: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub generator: Generator,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions_used: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions_required: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub private: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<FileEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<Dependency>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
    pub ppq: u32,
    pub tempo: Vec<TempoPoint>,
    pub meter: Vec<MeterPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub key: Vec<KeyPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<Marker>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#loop: Option<Loop>,
    #[serde(default, skip_serializing_if = "Master::is_default")]
    pub master: Master,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<Track>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<Scene>,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    #[serde(flatten)]
    pub unknown: Unknown,
}

impl Document {
    /// An empty project at 120 bpm in 4/4, `ppq` 960, written by `generator`.
    pub fn new(generator: Generator) -> Self {
        Self {
            format: crate::FORMAT.to_owned(),
            version: crate::VERSION.to_owned(),
            min_version: None,
            about: Some(crate::ABOUT.to_owned()),
            spec: Some(crate::SPEC_URL.to_owned()),
            schema: Some(crate::SCHEMA_URL.to_owned()),
            generator,
            extensions_used: Vec::new(),
            extensions_required: Vec::new(),
            private: Vec::new(),
            files: Vec::new(),
            dependencies: Vec::new(),
            metadata: Metadata::default(),
            ppq: 960,
            tempo: vec![TempoPoint {
                at: 0,
                bpm: 120.0,
                curve: Curve::Hold,
                unknown: Unknown::new(),
            }],
            meter: vec![MeterPoint {
                at: 0,
                numerator: 4,
                denominator: 4,
                unknown: Unknown::new(),
            }],
            key: Vec::new(),
            markers: Vec::new(),
            r#loop: None,
            master: Master::default(),
            tracks: Vec::new(),
            scenes: Vec::new(),
            extensions: Extensions::new(),
            unknown: Unknown::new(),
        }
    }

    /// The `major.minor` of `version`, when it has that form.
    pub fn version_parts(&self) -> Option<(u32, u32)> {
        version_parts(&self.version)
    }

    /// Puts every set of identified objects in its canonical order (spec 3.1): patterns, parts,
    /// events, automation lanes, markers and scenes by id; renders by file; `files` by path;
    /// `dependencies` by id; sends by destination; params by id; overrides by param. Ordered
    /// lists (tracks, children, devices, points) are left as they are.
    pub fn normalise(&mut self) {
        self.files.sort_by(|a, b| a.path.cmp(&b.path));
        self.dependencies.sort_by(|a, b| a.id.cmp(&b.id));
        self.markers.sort_by(|a, b| a.id.cmp(&b.id));
        self.scenes.sort_by(|a, b| a.id.cmp(&b.id));
        self.master.normalise();
        for track in &mut self.tracks {
            track.normalise();
        }
    }

    /// Every track in the tree, parents before children, in display order.
    pub fn all_tracks(&self) -> Vec<&Track> {
        fn walk<'a>(tracks: &'a [Track], out: &mut Vec<&'a Track>) {
            for t in tracks {
                out.push(t);
                walk(&t.children, out);
            }
        }
        let mut out = Vec::new();
        walk(&self.tracks, &mut out);
        out
    }
}

/// `major.minor` of a version string.
pub fn version_parts(version: &str) -> Option<(u32, u32)> {
    let (major, minor) = version.split_once('.')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// The last program that wrote the document (spec 3.2).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Generator {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(flatten)]
    pub unknown: Unknown,
}

impl Generator {
    pub fn new(name: &str, version: &str, url: &str) -> Self {
        Self {
            name: name.to_owned(),
            version: Some(version.to_owned()),
            url: Some(url.to_owned()),
            unknown: Unknown::new(),
        }
    }
}

/// An entry of the files index (spec 3.4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// What the project needs beyond its folder (spec 3.5).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Dependency {
    pub id: String,
    pub kind: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub formats: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<String>,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// Who and what the project is (spec 3.2).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(flatten)]
    pub unknown: Unknown,
}

impl Metadata {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

// --- time --------------------------------------------------------------------------------------

/// How a value runs from a point to the next (spec 5.2, 6.8). An unknown name reads as
/// `Other`, kept as read and played as the fallback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Curve {
    Hold,
    Linear,
    #[serde(untagged)]
    Other(String),
}

impl Curve {
    fn is_hold(&self) -> bool {
        *self == Curve::Hold
    }

    fn is_linear(&self) -> bool {
        *self == Curve::Linear
    }

    /// What a reader plays: `linear` for a name it does not know (spec 5.2, 6.8).
    pub fn effective(&self) -> Curve {
        match self {
            Curve::Hold => Curve::Hold,
            _ => Curve::Linear,
        }
    }
}

/// A point of the tempo map (spec 5.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TempoPoint {
    pub at: Ticks,
    pub bpm: f64,
    #[serde(default = "hold", skip_serializing_if = "Curve::is_hold")]
    pub curve: Curve,
    #[serde(flatten)]
    pub unknown: Unknown,
}

fn hold() -> Curve {
    Curve::Hold
}

fn linear() -> Curve {
    Curve::Linear
}

/// A point of the meter map (spec 5.3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeterPoint {
    pub at: Ticks,
    pub numerator: u32,
    pub denominator: u32,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A scale: a name from the spec's list, or a custom set of semitones above the tonic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Scale {
    Named(String),
    Custom { custom: Vec<u8> },
}

/// A point of the key map (spec 5.4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyPoint {
    pub at: Ticks,
    pub tonic: u8,
    pub scale: Scale,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A pattern's own key (spec 6.5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternKey {
    pub tonic: u8,
    pub scale: Scale,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A named point or section (spec 6.9).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub id: Id,
    pub at: Ticks,
    #[serde(default, skip_serializing_if = "is_empty_string")]
    pub name: String,
    #[serde(default = "marker_kind", skip_serializing_if = "is_marker")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<Ticks>,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    #[serde(flatten)]
    pub unknown: Unknown,
}

fn marker_kind() -> String {
    "marker".to_owned()
}

fn is_marker(kind: &str) -> bool {
    kind == "marker"
}

/// The loop range (spec 3.6).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loop {
    pub start: Ticks,
    pub end: Ticks,
    #[serde(default, skip_serializing_if = "is_false")]
    pub enabled: bool,
    #[serde(flatten)]
    pub unknown: Unknown,
}

// --- tracks and channels -----------------------------------------------------------------------

/// The master channel (spec 6.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Master {
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub gain: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pan: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub mute: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<Device>,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    #[serde(flatten)]
    pub unknown: Unknown,
}

impl Default for Master {
    fn default() -> Self {
        Self {
            gain: 1.0,
            pan: 0.0,
            mute: false,
            devices: Vec::new(),
            extensions: Extensions::new(),
            unknown: Unknown::new(),
        }
    }
}

impl Master {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    fn normalise(&mut self) {
        for device in &mut self.devices {
            device.normalise();
        }
    }
}

/// A track and its channel (spec 6.1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: Id,
    #[serde(default, skip_serializing_if = "is_empty_string")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Track>,
    #[serde(default, skip_serializing_if = "Channel::is_default")]
    pub channel: Channel,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<Device>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<Promoted>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drum_map: Option<Vec<Lane>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub patterns: Vec<Pattern>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Part>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub automation: Vec<AutomationLane>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub renders: Vec<Render>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slots: Vec<Slot>,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    #[serde(flatten)]
    pub unknown: Unknown,
}

impl Track {
    /// A track with nothing on it.
    pub fn new(id: Id, name: &str) -> Self {
        Self {
            id,
            name: name.to_owned(),
            color: None,
            children: Vec::new(),
            channel: Channel::default(),
            devices: Vec::new(),
            parameters: Vec::new(),
            drum_map: None,
            patterns: Vec::new(),
            parts: Vec::new(),
            automation: Vec::new(),
            renders: Vec::new(),
            slots: Vec::new(),
            extensions: Extensions::new(),
            unknown: Unknown::new(),
        }
    }

    fn normalise(&mut self) {
        self.channel.sends.sort_by(|a, b| a.to.cmp(&b.to));
        for device in &mut self.devices {
            device.normalise();
        }
        self.patterns.sort_by(|a, b| a.id.cmp(&b.id));
        for pattern in &mut self.patterns {
            pattern.events.sort_by(|a, b| a.id().cmp(b.id()));
        }
        self.parts.sort_by(|a, b| a.id.cmp(&b.id));
        self.automation.sort_by(|a, b| a.id.cmp(&b.id));
        self.renders.sort_by(|a, b| a.file.cmp(&b.file));
        for child in &mut self.children {
            child.normalise();
        }
    }
}

/// The mixer strip (spec 6.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub gain: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pan: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub mute: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub solo: bool,
    #[serde(default = "two", skip_serializing_if = "is_two")]
    pub channels: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<Id>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sends: Vec<Send>,
    #[serde(flatten)]
    pub unknown: Unknown,
}

fn two() -> u32 {
    2
}

fn is_two(n: &u32) -> bool {
    *n == 2
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            gain: 1.0,
            pan: 0.0,
            mute: false,
            solo: false,
            channels: 2,
            destination: None,
            sends: Vec::new(),
            unknown: Unknown::new(),
        }
    }
}

impl Channel {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// A send to another track (spec 6.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Send {
    pub to: Id,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub gain: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub pre_fader: bool,
    #[serde(default = "r#true", skip_serializing_if = "is_true")]
    pub enabled: bool,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A promoted parameter (spec 6.8).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Promoted {
    pub name: String,
    pub device: Id,
    pub param: String,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A drum lane (spec 6.11).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lane {
    pub pitch: u8,
    #[serde(default, skip_serializing_if = "is_empty_string")]
    pub name: String,
    #[serde(flatten)]
    pub unknown: Unknown,
}

// --- devices -----------------------------------------------------------------------------------

/// A device in a chain (spec 7.1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: Id,
    pub role: String,
    #[serde(default, skip_serializing_if = "is_empty_string")]
    pub name: String,
    pub plugin: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<Alias>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<StateRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Param>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<Override>,
    #[serde(default = "r#true", skip_serializing_if = "is_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    #[serde(flatten)]
    pub unknown: Unknown,
}

impl Device {
    fn normalise(&mut self) {
        self.params.sort_by(|a, b| a.id.cmp(&b.id));
        self.overrides.sort_by(|a, b| a.param.cmp(&b.param));
    }

    /// The role a reader plays: `audio_effect`, bypassed, for a role it does not know (spec 7.1).
    pub fn effective_role(&self) -> Role {
        match self.role.as_str() {
            "instrument" => Role::Instrument,
            "note_effect" => Role::NoteEffect,
            "audio_effect" => Role::AudioEffect,
            "analyzer" => Role::Analyzer,
            _ => Role::Unknown,
        }
    }
}

/// A device's place in the chain (spec 7.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Instrument,
    NoteEffect,
    AudioEffect,
    Analyzer,
    /// A role this version does not know: treated as an audio effect, bypassed.
    Unknown,
}

/// The same plugin in another format (spec 7.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alias {
    pub plugin: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub state_compatible: bool,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A state sidecar (spec 7.4): `plugins/states/<sha256>.<kind>`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateRef {
    pub sha256: String,
    pub kind: String,
    #[serde(flatten)]
    pub unknown: Unknown,
}

impl StateRef {
    /// The sidecar's reference, `plugins/states/<sha256>.<kind>`.
    pub fn path(&self) -> String {
        format!("plugins/states/{}.{}", self.sha256, self.kind)
    }
}

/// One parameter of the snapshot (spec 7.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A value applied after the state (spec 7.5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Override {
    pub param: String,
    pub value: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized: Option<f64>,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A bounce of a track's chain (spec 7.7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Render {
    pub file: String,
    pub sha256: String,
    pub from: Ticks,
    pub to: Ticks,
    pub fingerprint: String,
    #[serde(flatten)]
    pub unknown: Unknown,
}

// --- patterns, parts, events -------------------------------------------------------------------

/// Reusable content (spec 6.5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub id: Id,
    #[serde(default, skip_serializing_if = "is_empty_string")]
    pub name: String,
    pub length: Ticks,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<PatternKey>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<Event>,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A window on a pattern, placed on a track (spec 6.6).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub id: Id,
    pub pattern: Id,
    pub at: Ticks,
    pub length: Ticks,
    #[serde(default, skip_serializing_if = "is_zero_ticks")]
    pub offset: Ticks,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#loop: Option<LoopRange>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub mute: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A loop inside a pattern, in pattern ticks (spec 6.6, 6.10).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopRange {
    pub start: Ticks,
    pub end: Ticks,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// An event of a pattern (spec 6.7): a known kind, or one this version does not know, kept
/// whole and played as nothing. A note is the common case and the largest variant; boxing it
/// would cost an allocation per note for nothing.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Event {
    Note(Note),
    Chord(Chord),
    Param(ParamEvent),
    /// An event whose `type` this version does not know, as read.
    Unknown(serde_json::Map<String, Value>),
}

impl Event {
    /// The event's id, or an empty string for an unknown event without one.
    pub fn id(&self) -> &str {
        match self {
            Event::Note(e) => e.id.as_str(),
            Event::Chord(e) => e.id.as_str(),
            Event::Param(e) => e.id.as_str(),
            Event::Unknown(map) => map.get("id").and_then(Value::as_str).unwrap_or(""),
        }
    }

    /// Where the event starts, in pattern ticks, when it says.
    pub fn at(&self) -> Option<Ticks> {
        match self {
            Event::Note(e) => Some(e.at),
            Event::Chord(e) => Some(e.at),
            Event::Param(e) => Some(e.at),
            Event::Unknown(map) => map.get("at").and_then(Value::as_i64),
        }
    }

    /// The `type` as read.
    pub fn kind(&self) -> &str {
        match self {
            Event::Note(_) => "note",
            Event::Chord(_) => "chord",
            Event::Param(_) => "param",
            Event::Unknown(map) => map.get("type").and_then(Value::as_str).unwrap_or(""),
        }
    }
}

/// The known kinds, tagged by `type`, for serde.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
enum KnownEvent {
    Note(Note),
    Chord(Chord),
    Param(ParamEvent),
}

impl Serialize for Event {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Event::Note(e) => KnownEvent::Note(e.clone()).serialize(serializer),
            Event::Chord(e) => KnownEvent::Chord(e.clone()).serialize(serializer),
            Event::Param(e) => KnownEvent::Param(e.clone()).serialize(serializer),
            Event::Unknown(map) => map.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for Event {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let map = match value {
            Value::Object(map) => map,
            other => {
                return Err(serde::de::Error::custom(format!(
                    "an event is an object, not {other}"
                )));
            }
        };
        match map.get("type").and_then(Value::as_str) {
            Some("note" | "chord" | "param") => {
                let known: KnownEvent =
                    serde_json::from_value(Value::Object(map)).map_err(serde::de::Error::custom)?;
                Ok(match known {
                    KnownEvent::Note(e) => Event::Note(e),
                    KnownEvent::Chord(e) => Event::Chord(e),
                    KnownEvent::Param(e) => Event::Param(e),
                })
            }
            _ => Ok(Event::Unknown(map)),
        }
    }
}

/// A note (spec 6.7).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: Id,
    pub at: Ticks,
    pub length: Ticks,
    pub pitch: u8,
    pub velocity: f64,
    #[serde(default = "half", skip_serializing_if = "is_half")]
    pub release: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expressions: Option<Expressions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioPiece>,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    #[serde(flatten)]
    pub unknown: Unknown,
}

fn half() -> f64 {
    0.5
}

fn is_half(x: &f64) -> bool {
    *x == 0.5
}

/// Per-note curves, each relative to the note's start (spec 6.7).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Expressions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pitch: Vec<Point>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pressure: Vec<Point>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timbre: Vec<Point>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volume: Vec<Point>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pan: Vec<Point>,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A point of a value curve (spec 6.8).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub at: Ticks,
    pub value: f64,
    #[serde(default = "linear", skip_serializing_if = "Curve::is_linear")]
    pub curve: Curve,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// Frames of a file placed in a pattern (spec 5.5, 6.7).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioPiece {
    pub file: String,
    pub sha256: String,
    pub sample_rate: u32,
    pub channels: u32,
    pub frames: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub from: u64,
    pub to: u64,
    pub map: Vec<MapPoint>,
    pub bpm: Option<f64>,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub gain: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub polarity: bool,
    #[serde(default = "as_recorded", skip_serializing_if = "is_as_recorded")]
    pub channel_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_in: Option<Fade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade_out: Option<Fade>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub reverse: bool,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    #[serde(flatten)]
    pub unknown: Unknown,
}

fn as_recorded() -> String {
    "as_recorded".to_owned()
}

fn is_as_recorded(mode: &str) -> bool {
    mode == "as_recorded"
}

/// A point of an audio piece's map (spec 5.5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapPoint {
    pub frame: u64,
    pub tick: Ticks,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A fade at one edge of a piece (spec 6.7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fade {
    pub length: Ticks,
    #[serde(default, skip_serializing_if = "is_zero_ticks")]
    pub outside: Ticks,
    #[serde(default = "linear_shape", skip_serializing_if = "is_linear_shape")]
    pub shape: String,
    #[serde(flatten)]
    pub unknown: Unknown,
}

fn linear_shape() -> String {
    "linear".to_owned()
}

fn is_linear_shape(shape: &str) -> bool {
    shape == "linear"
}

/// A chord with its voicing (spec 6.7).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chord {
    pub id: Id,
    pub at: Ticks,
    pub length: Ticks,
    pub root: u8,
    pub quality: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bass: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub notes: Vec<u8>,
    #[serde(default = "point_eight", skip_serializing_if = "is_point_eight")]
    pub velocity: f64,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    #[serde(flatten)]
    pub unknown: Unknown,
}

fn point_eight() -> f64 {
    0.8
}

fn is_point_eight(x: &f64) -> bool {
    *x == 0.8
}

/// A parameter set at a point in a pattern (spec 6.7).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamEvent {
    pub id: Id,
    pub at: Ticks,
    pub param: String,
    pub value: f64,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    #[serde(flatten)]
    pub unknown: Unknown,
}

// --- automation, scenes --------------------------------------------------------------------------

/// A lane of automation on a track (spec 6.8).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AutomationLane {
    pub id: Id,
    pub target: Target,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<Point>,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// What a lane drives (spec 6.8). `channel` carries `gain`, `pan`, `mute` or `send`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter: Option<String>,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A scene of the clip launcher (spec 6.10).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub id: Id,
    #[serde(default, skip_serializing_if = "is_empty_string")]
    pub name: String,
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    #[serde(flatten)]
    pub unknown: Unknown,
}

/// A track's slot in a scene (spec 6.10).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    pub scene: Id,
    pub pattern: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#loop: Option<LoopRange>,
    #[serde(flatten)]
    pub unknown: Unknown,
}
