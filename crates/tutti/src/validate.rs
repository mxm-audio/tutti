//! What the schema cannot say (spec 5.1, 6, 7, 9): ids unique and well-formed, references
//! resolving, routing acyclic, every number within its bounds, every point list strictly
//! increasing, every file reference in its form. An error makes the document invalid; a warning
//! is reported and the document still opens (a dangling reference, an unknown name).

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::id;
use crate::id::Id;
use crate::model::*;
use crate::time::TICK_MAX;

/// How bad an issue is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// The document is invalid (spec 9).
    Error,
    /// Reported; the document opens.
    Warning,
}

/// One finding, at a JSON pointer into the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    pub pointer: String,
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "{kind} {}: {}", self.pointer, self.message)
    }
}

/// Every finding of a validation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub issues: Vec<Issue>,
}

impl Report {
    pub fn is_valid(&self) -> bool {
        self.issues.iter().all(|i| i.severity != Severity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.severity == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Issue> {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
    }

    fn error(&mut self, pointer: &str, message: impl Into<String>) {
        self.issues.push(Issue {
            severity: Severity::Error,
            pointer: pointer.to_owned(),
            message: message.into(),
        });
    }

    fn warn(&mut self, pointer: &str, message: impl Into<String>) {
        self.issues.push(Issue {
            severity: Severity::Warning,
            pointer: pointer.to_owned(),
            message: message.into(),
        });
    }
}

/// The bounds the spec states (5.2, 5.3, 5.5, 6.2, 6.7).
pub const PPQ_MAX: u32 = 1 << 20;
pub const BPM_MAX: f64 = 1000.0;
pub const GAIN_MAX: f64 = 1000.0;
pub const NUMERATOR_MAX: u32 = 99;
pub const DENOMINATOR_MAX: u32 = 128;
pub const SAMPLE_RATE_MAX: u32 = 1 << 24;
pub const CHANNELS_MAX: u32 = 256;

/// The plugin id schemes (spec 7.2).
pub const PLUGIN_SCHEMES: &[&str] = &["clap:", "vst3:", "au:", "lv2:", "vst2:", "builtin:"];
/// The state kinds (spec 7.4).
pub const STATE_KINDS: &[&str] = &["clap", "vstpreset", "fxp", "aupreset", "lv2"];
/// The scale names (spec 5.4).
pub const SCALES: &[&str] = &[
    "major",
    "minor",
    "harmonic_minor",
    "melodic_minor",
    "dorian",
    "phrygian",
    "lydian",
    "mixolydian",
    "locrian",
    "major_pentatonic",
    "minor_pentatonic",
    "blues",
    "chromatic",
];
/// The chord qualities (spec 6.7).
pub const QUALITIES: &[&str] = &[
    "maj", "min", "dim", "aug", "maj7", "min7", "7", "min7b5", "dim7", "sus2", "sus4", "6", "min6",
    "9", "maj9", "min9", "add9", "11", "13", "5",
];
/// The parameter units (spec 7.3).
pub const UNITS: &[&str] = &[
    "linear",
    "normalized",
    "percent",
    "decibel",
    "hertz",
    "semitones",
    "seconds",
    "beats",
    "bpm",
    "bool",
    "enum",
    "other",
];

/// Whether `name` is a file name a writer may give (spec 2.1): `^[A-Za-z0-9_-]{1,120}\.[A-Za-z0-9]{1,8}$`.
pub fn is_file_name(name: &str) -> bool {
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return false;
    };
    (1..=120).contains(&stem.len())
        && stem
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        && (1..=8).contains(&ext.len())
        && ext.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// A device's own file in the index, `plugins/files/<device id>/<path>` (spec 7.8): the device's
/// id and the path inside its folder.
pub fn device_file(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix("plugins/files/")?;
    let (device, relative) = rest.split_once('/')?;
    (crate::id::check(device).is_ok() && !relative.is_empty()).then_some((device, relative))
}

/// Checks the path of a device's own file inside its folder (spec 7.8): safety is required
/// (a relative path of plain parts, as `zip::check_entry_name` says; the reason when not),
/// portability advised (every part in the file-name characters: `Ok(true)`, else `Ok(false)`).
pub fn check_device_file_path(relative: &str) -> Result<bool, String> {
    crate::zip::check_entry_name(relative)?;
    if relative.ends_with('/') {
        return Err("a folder, not a file".to_owned());
    }
    let portable = relative.split('/').all(|part| {
        let (stem, extension) = part.rsplit_once('.').unwrap_or((part, ""));
        !stem.is_empty()
            && stem
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            && extension.bytes().all(|b| b.is_ascii_alphanumeric())
    });
    Ok(portable)
}

/// Whether `reference` is `<folder>/<file name>` for one of the folders a reader reads (spec 2.2).
pub fn is_reference(reference: &str, folder: &str) -> bool {
    reference
        .strip_prefix(folder)
        .and_then(|r| r.strip_prefix('/'))
        .is_some_and(is_file_name)
}

/// 64 lower-case hexadecimal digits.
pub fn is_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A reverse-DNS namespace (spec 8.1): `^[a-z0-9]+(\.[a-z0-9-]+)+$`.
pub fn is_namespace(text: &str) -> bool {
    let mut parts = text.split('.');
    let first = parts.next().unwrap_or("");
    let rest: Vec<&str> = parts.collect();
    !first.is_empty()
        && first
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && !rest.is_empty()
        && rest.iter().all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

struct Ids<'a> {
    seen: HashMap<&'a str, String>,
    tracks: HashSet<&'a str>,
    scenes: HashSet<&'a str>,
}

/// Validates a document (spec 9). The document is taken as read: `read` has already refused
/// what a parser refuses.
pub fn validate(doc: &Document) -> Report {
    let mut r = Report::default();
    let mut ids = Ids {
        seen: HashMap::new(),
        tracks: HashSet::new(),
        scenes: HashSet::new(),
    };

    // Pass 1: every id, so references can be checked in pass 2.
    for (i, m) in doc.markers.iter().enumerate() {
        take_id(&mut r, &mut ids, &m.id, &format!("/markers/{i}/id"));
    }
    for (i, s) in doc.scenes.iter().enumerate() {
        take_id(&mut r, &mut ids, &s.id, &format!("/scenes/{i}/id"));
        ids.scenes.insert(s.id.as_str());
    }
    for (i, d) in doc.master.devices.iter().enumerate() {
        take_id(&mut r, &mut ids, &d.id, &format!("/master/devices/{i}/id"));
    }
    for (i, t) in doc.tracks.iter().enumerate() {
        collect_track_ids(&mut r, &mut ids, t, &format!("/tracks/{i}"));
    }

    // The manifest.
    if doc.format != crate::FORMAT {
        r.error(
            "/format",
            format!("the format is {:?}, not \"tutti\"", doc.format),
        );
    }
    match version_parts(&doc.version) {
        Some(_) => {}
        None => r.error("/version", "not major.minor"),
    }
    if let Some(min) = &doc.min_version {
        match (version_parts(min), version_parts(&doc.version)) {
            (Some((a, _)), Some((b, _))) if a == b => {}
            (Some(_), Some(_)) => r.error("/min_version", "a different major than the version"),
            _ => r.error("/min_version", "not major.minor"),
        }
    }
    if doc.generator.name.is_empty() {
        r.error("/generator/name", "empty");
    }
    for (i, ns) in doc.extensions_used.iter().enumerate() {
        if !is_namespace(ns) {
            r.error(
                &format!("/extensions_used/{i}"),
                "not a reverse-DNS namespace",
            );
        }
    }
    for (i, ns) in doc.extensions_required.iter().enumerate() {
        if !doc.extensions_used.contains(ns) {
            r.warn(
                &format!("/extensions_required/{i}"),
                "required but not listed as used",
            );
        }
    }
    for (i, p) in doc.private.iter().enumerate() {
        let name = p.strip_suffix('/').unwrap_or(p);
        if name.is_empty() || name.contains('/') || name.contains('\\') || name == ".." {
            r.error(
                &format!("/private/{i}"),
                "a private path is one folder or file name at the top",
            );
        }
    }
    let device_ids: HashSet<&str> = doc
        .master
        .devices
        .iter()
        .chain(doc.all_tracks().into_iter().flat_map(|t| t.devices.iter()))
        .map(|d| d.id.as_str())
        .collect();
    let mut paths = HashSet::new();
    for (i, f) in doc.files.iter().enumerate() {
        let p = format!("/files/{i}");
        let plain = ["audio", "plugins/states", "renders"]
            .iter()
            .any(|folder| is_reference(&f.path, folder));
        if !plain {
            match device_file(&f.path) {
                Some((device, relative)) => match check_device_file_path(relative) {
                    Err(reason) => r.error(&format!("{p}/path"), &reason),
                    Ok(portable) => {
                        if !portable {
                            r.warn(
                                &format!("{p}/path"),
                                "a part outside the file-name characters: may not travel to \
                                 every file system",
                            );
                        }
                        if !device_ids.contains(device) {
                            r.warn(&format!("{p}/path"), "no device with this id: stale");
                        }
                    }
                },
                None => r.error(
                    &format!("{p}/path"),
                    "not a reference into audio/, plugins/states/, renders/ or a device's \
                     plugins/files/<id>/",
                ),
            }
        }
        if !is_sha256(&f.sha256) {
            r.error(
                &format!("{p}/sha256"),
                "not 64 lower-case hexadecimal digits",
            );
        }
        if !paths.insert(f.path.as_str()) {
            r.error(&format!("{p}/path"), "listed twice");
        }
    }
    let mut dep_ids = HashSet::new();
    for (i, d) in doc.dependencies.iter().enumerate() {
        let p = format!("/dependencies/{i}");
        if !has_scheme(&d.id) {
            r.warn(&format!("{p}/id"), "no known plugin id scheme");
        }
        if d.kind != "plugin" {
            r.warn(
                &format!("{p}/kind"),
                format!("{:?} is not a kind this version knows", d.kind),
            );
        }
        if !dep_ids.insert(d.id.as_str()) {
            r.error(&format!("{p}/id"), "listed twice");
        }
    }

    // Time.
    if doc.ppq == 0 || doc.ppq > PPQ_MAX {
        r.error("/ppq", format!("not within 1 and {PPQ_MAX}"));
    }
    if doc.tempo.is_empty() {
        r.error("/tempo", "empty: a point at tick 0 is required");
    } else if doc.tempo[0].at != 0 {
        r.error("/tempo/0/at", "the first tempo point is not at tick 0");
    }
    check_increasing(&mut r, "/tempo", doc.tempo.iter().map(|p| p.at));
    for (i, p) in doc.tempo.iter().enumerate() {
        check_bpm(&mut r, &format!("/tempo/{i}/bpm"), p.bpm);
        check_tick(&mut r, &format!("/tempo/{i}/at"), p.at);
        if let Curve::Other(name) = &p.curve {
            r.warn(
                &format!("/tempo/{i}/curve"),
                format!("{name:?} is not a curve this version knows; played as linear"),
            );
        }
    }
    if doc.meter.is_empty() {
        r.error("/meter", "empty: a point at tick 0 is required");
    } else if doc.meter[0].at != 0 {
        r.error("/meter/0/at", "the first meter point is not at tick 0");
    }
    check_increasing(&mut r, "/meter", doc.meter.iter().map(|p| p.at));
    for (i, p) in doc.meter.iter().enumerate() {
        check_tick(&mut r, &format!("/meter/{i}/at"), p.at);
        if p.numerator == 0 || p.numerator > NUMERATOR_MAX {
            r.error(
                &format!("/meter/{i}/numerator"),
                format!("not within 1 and {NUMERATOR_MAX}"),
            );
        }
        if p.denominator == 0 || p.denominator > DENOMINATOR_MAX || !p.denominator.is_power_of_two()
        {
            r.error(
                &format!("/meter/{i}/denominator"),
                format!("not a power of two within 1 and {DENOMINATOR_MAX}"),
            );
        }
    }
    check_increasing(&mut r, "/key", doc.key.iter().map(|p| p.at));
    for (i, p) in doc.key.iter().enumerate() {
        check_tick(&mut r, &format!("/key/{i}/at"), p.at);
        check_key(&mut r, &format!("/key/{i}"), p.tonic, &p.scale);
    }
    for (i, m) in doc.markers.iter().enumerate() {
        let p = format!("/markers/{i}");
        check_tick(&mut r, &format!("{p}/at"), m.at);
        match m.kind.as_str() {
            "marker" => {}
            "section" => match m.length {
                Some(len) if len > 0 => check_tick(&mut r, &format!("{p}/length"), len),
                _ => r.error(&format!("{p}/length"), "a section needs a length above 0"),
            },
            other => r.warn(
                &format!("{p}/kind"),
                format!("{other:?} is not a kind this version knows; read as a marker"),
            ),
        }
        check_extensions(&mut r, &p, &m.extensions, doc);
    }
    if let Some(l) = &doc.r#loop {
        check_tick(&mut r, "/loop/start", l.start);
        check_tick(&mut r, "/loop/end", l.end);
        if l.start >= l.end {
            r.error("/loop", "start is not before end");
        }
    }

    // The master and the tracks.
    check_gain(&mut r, "/master/gain", doc.master.gain);
    check_pan(&mut r, "/master/pan", doc.master.pan);
    for (i, d) in doc.master.devices.iter().enumerate() {
        check_device(&mut r, &format!("/master/devices/{i}"), d, doc);
    }
    check_extensions(&mut r, "/master", &doc.master.extensions, doc);
    let mut edges: Vec<(String, String)> = Vec::new();
    for (i, t) in doc.tracks.iter().enumerate() {
        check_track(
            &mut r,
            &format!("/tracks/{i}"),
            t,
            None,
            doc,
            &ids,
            &mut edges,
        );
    }
    if let Some(cycle) = find_cycle(&edges) {
        r.error(
            "/tracks",
            format!("routing is not acyclic: {}", cycle.join(" -> ")),
        );
    }
    for (i, s) in doc.scenes.iter().enumerate() {
        check_extensions(&mut r, &format!("/scenes/{i}"), &s.extensions, doc);
    }
    check_extensions(&mut r, "", &doc.extensions, doc);
    r
}

fn take_id<'a>(r: &mut Report, ids: &mut Ids<'a>, id: &'a Id, pointer: &str) {
    if let Err(e) = id::check(id.as_str()) {
        r.error(pointer, e.to_string());
    }
    if id.as_str() == "master" {
        r.error(pointer, "\"master\" is reserved for the master channel");
    }
    if let Some(first) = ids.seen.insert(id.as_str(), pointer.to_owned()) {
        r.error(
            pointer,
            format!("the id {:?} is also at {first}", id.as_str()),
        );
    }
}

fn collect_track_ids<'a>(r: &mut Report, ids: &mut Ids<'a>, t: &'a Track, p: &str) {
    take_id(r, ids, &t.id, &format!("{p}/id"));
    ids.tracks.insert(t.id.as_str());
    for (i, d) in t.devices.iter().enumerate() {
        take_id(r, ids, &d.id, &format!("{p}/devices/{i}/id"));
    }
    for (i, pat) in t.patterns.iter().enumerate() {
        take_id(r, ids, &pat.id, &format!("{p}/patterns/{i}/id"));
        for (j, e) in pat.events.iter().enumerate() {
            let ep = format!("{p}/patterns/{i}/events/{j}/id");
            match e {
                Event::Note(n) => take_id(r, ids, &n.id, &ep),
                Event::Chord(c) => take_id(r, ids, &c.id, &ep),
                Event::Param(pe) => take_id(r, ids, &pe.id, &ep),
                Event::Unknown(_) => {
                    if e.id().is_empty() {
                        r.error(&ep, "an event needs an id");
                    } else {
                        // An unknown event's id is checked for form and uniqueness like any other.
                        if let Err(err) = id::check(e.id()) {
                            r.error(&ep, err.to_string());
                        }
                        if let Some(first) = ids.seen.insert(e.id(), ep.clone()) {
                            r.error(&ep, format!("the id {:?} is also at {first}", e.id()));
                        }
                    }
                }
            }
        }
    }
    for (i, part) in t.parts.iter().enumerate() {
        take_id(r, ids, &part.id, &format!("{p}/parts/{i}/id"));
    }
    for (i, lane) in t.automation.iter().enumerate() {
        take_id(r, ids, &lane.id, &format!("{p}/automation/{i}/id"));
    }
    for (i, child) in t.children.iter().enumerate() {
        collect_track_ids(r, ids, child, &format!("{p}/children/{i}"));
    }
}

#[allow(clippy::too_many_arguments)]
fn check_track(
    r: &mut Report,
    p: &str,
    t: &Track,
    parent: Option<&str>,
    doc: &Document,
    ids: &Ids<'_>,
    edges: &mut Vec<(String, String)>,
) {
    if let Some(c) = &t.color {
        let ok = c.len() == 7
            && c.starts_with('#')
            && c[1..]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if !ok {
            r.warn(&format!("{p}/color"), "not #rrggbb in lower case");
        }
    }
    // The channel.
    let ch = &t.channel;
    check_gain(r, &format!("{p}/channel/gain"), ch.gain);
    check_pan(r, &format!("{p}/channel/pan"), ch.pan);
    if ch.channels != 1 && ch.channels != 2 {
        r.error(&format!("{p}/channel/channels"), "not 1 or 2");
    }
    let me = t.id.as_str().to_owned();
    // A track whose id is the reserved "master" is already an error; it takes no part in routing.
    let routes = me != "master";
    match &ch.destination {
        None if routes => edges.push((me.clone(), parent.unwrap_or("master").to_owned())),
        Some(d) if d.as_str() == "master" => {
            if routes {
                edges.push((me.clone(), "master".to_owned()));
            }
        }
        Some(d) if ids.tracks.contains(d.as_str()) => {
            if routes {
                edges.push((me.clone(), d.as_str().to_owned()));
            }
        }
        Some(d) => r.warn(
            &format!("{p}/channel/destination"),
            format!("{:?} is not a track; dangling", d.as_str()),
        ),
        None => {}
    }
    let mut send_targets = HashSet::new();
    for (i, s) in ch.sends.iter().enumerate() {
        let sp = format!("{p}/channel/sends/{i}");
        check_gain(r, &format!("{sp}/gain"), s.gain);
        if !send_targets.insert(s.to.as_str()) {
            r.error(&format!("{sp}/to"), "two sends to one destination");
        }
        if ids.tracks.contains(s.to.as_str()) {
            if routes {
                edges.push((me.clone(), s.to.as_str().to_owned()));
            }
        } else {
            r.warn(
                &format!("{sp}/to"),
                format!("{:?} is not a track; dangling", s.to.as_str()),
            );
        }
    }
    // Devices and promoted parameters.
    let mut device_ids: HashSet<&str> = HashSet::new();
    for (i, d) in t.devices.iter().enumerate() {
        check_device(r, &format!("{p}/devices/{i}"), d, doc);
        device_ids.insert(d.id.as_str());
    }
    let mut names = HashSet::new();
    for (i, pr) in t.parameters.iter().enumerate() {
        let pp = format!("{p}/parameters/{i}");
        if pr.name.is_empty() {
            r.error(&format!("{pp}/name"), "empty");
        }
        if !names.insert(pr.name.as_str()) {
            r.error(
                &format!("{pp}/name"),
                "two promoted parameters with one name",
            );
        }
        if !device_ids.contains(pr.device.as_str()) {
            r.warn(
                &format!("{pp}/device"),
                "not a device of this track; dangling",
            );
        }
    }
    if let Some(lanes) = &t.drum_map {
        for (i, l) in lanes.iter().enumerate() {
            if l.pitch > 127 {
                r.error(&format!("{p}/drum_map/{i}/pitch"), "above 127");
            }
        }
    }
    // Patterns.
    let mut pattern_ids: HashSet<&str> = HashSet::new();
    let mut pattern_lengths: HashMap<&str, i64> = HashMap::new();
    for (i, pat) in t.patterns.iter().enumerate() {
        let pp = format!("{p}/patterns/{i}");
        pattern_ids.insert(pat.id.as_str());
        pattern_lengths.insert(pat.id.as_str(), pat.length);
        if pat.length <= 0 {
            r.error(&format!("{pp}/length"), "not above 0");
        }
        check_tick(r, &format!("{pp}/length"), pat.length);
        if let Some(k) = &pat.key {
            check_key(r, &format!("{pp}/key"), k.tonic, &k.scale);
        }
        for (j, e) in pat.events.iter().enumerate() {
            check_event(r, &format!("{pp}/events/{j}"), e, &names, doc);
        }
        check_extensions(r, &pp, &pat.extensions, doc);
    }
    // Parts.
    for (i, part) in t.parts.iter().enumerate() {
        let pp = format!("{p}/parts/{i}");
        check_tick(r, &format!("{pp}/at"), part.at);
        if part.length <= 0 {
            r.error(&format!("{pp}/length"), "not above 0");
        }
        check_tick(r, &format!("{pp}/length"), part.length);
        if part.offset < 0 {
            r.error(&format!("{pp}/offset"), "negative");
        }
        if !pattern_ids.contains(part.pattern.as_str()) {
            if ids.seen.contains_key(part.pattern.as_str()) {
                r.error(
                    &format!("{pp}/pattern"),
                    "a part only plays a pattern of its own track",
                );
            } else {
                r.warn(
                    &format!("{pp}/pattern"),
                    format!("{:?} is not a pattern; dangling", part.pattern.as_str()),
                );
            }
        }
        if let Some(l) = &part.r#loop {
            check_loop_range(
                r,
                &format!("{pp}/loop"),
                l,
                pattern_lengths.get(part.pattern.as_str()).copied(),
            );
        }
        check_extensions(r, &pp, &part.extensions, doc);
    }
    // Automation.
    for (i, lane) in t.automation.iter().enumerate() {
        check_lane(
            r,
            &format!("{p}/automation/{i}"),
            lane,
            &device_ids,
            &names,
            ids,
            doc,
        );
    }
    // Renders.
    for (i, rd) in t.renders.iter().enumerate() {
        let rp = format!("{p}/renders/{i}");
        if !is_reference(&rd.file, "renders") {
            r.error(&format!("{rp}/file"), "not a reference into renders/");
        }
        if !is_sha256(&rd.sha256) {
            r.error(
                &format!("{rp}/sha256"),
                "not 64 lower-case hexadecimal digits",
            );
        }
        if !is_sha256(&rd.fingerprint) {
            r.error(
                &format!("{rp}/fingerprint"),
                "not 64 lower-case hexadecimal digits",
            );
        }
        check_tick(r, &format!("{rp}/from"), rd.from);
        check_tick(r, &format!("{rp}/to"), rd.to);
        if rd.from >= rd.to {
            r.error(&rp, "from is not before to");
        }
    }
    // Slots.
    let mut slot_scenes = HashSet::new();
    for (i, s) in t.slots.iter().enumerate() {
        let sp = format!("{p}/slots/{i}");
        if !ids.scenes.contains(s.scene.as_str()) {
            r.warn(
                &format!("{sp}/scene"),
                format!("{:?} is not a scene; dangling", s.scene.as_str()),
            );
        }
        if !slot_scenes.insert(s.scene.as_str()) {
            r.error(&format!("{sp}/scene"), "two slots in one scene");
        }
        if !pattern_ids.contains(s.pattern.as_str()) {
            r.error(
                &format!("{sp}/pattern"),
                "a slot only plays a pattern of its own track",
            );
        }
        if let Some(l) = &s.r#loop {
            check_loop_range(
                r,
                &format!("{sp}/loop"),
                l,
                pattern_lengths.get(s.pattern.as_str()).copied(),
            );
        }
    }
    check_extensions(r, p, &t.extensions, doc);
    for (i, child) in t.children.iter().enumerate() {
        check_track(
            r,
            &format!("{p}/children/{i}"),
            child,
            Some(t.id.as_str()),
            doc,
            ids,
            edges,
        );
    }
}

fn check_device(r: &mut Report, p: &str, d: &Device, doc: &Document) {
    if d.effective_role() == Role::Unknown {
        r.warn(
            &format!("{p}/role"),
            format!("{:?} is not a role this version knows; bypassed", d.role),
        );
    }
    check_plugin_id(r, &format!("{p}/plugin"), &d.plugin);
    for (i, a) in d.aliases.iter().enumerate() {
        check_plugin_id(r, &format!("{p}/aliases/{i}/plugin"), &a.plugin);
    }
    if let Some(s) = &d.state {
        if !is_sha256(&s.sha256) {
            r.error(
                &format!("{p}/state/sha256"),
                "not 64 lower-case hexadecimal digits",
            );
        }
        if !STATE_KINDS.contains(&s.kind.as_str()) {
            r.warn(
                &format!("{p}/state/kind"),
                format!("{:?} is not a state kind this version knows", s.kind),
            );
        } else if !is_file_name(&format!("{}.{}", s.sha256, s.kind)) {
            r.error(&format!("{p}/state"), "does not name a file");
        }
    }
    let mut param_ids = HashSet::new();
    for (i, pa) in d.params.iter().enumerate() {
        let pp = format!("{p}/params/{i}");
        if pa.id.is_empty() {
            r.error(&format!("{pp}/id"), "empty");
        }
        if !param_ids.insert(pa.id.as_str()) {
            r.error(&format!("{pp}/id"), "listed twice");
        }
        if let Some(n) = pa.normalized {
            check_unit_interval(r, &format!("{pp}/normalized"), n);
        }
        if let (Some(min), Some(max)) = (pa.min, pa.max) {
            if min > max {
                r.error(&pp, "min is above max");
            }
        }
        if let Some(u) = &pa.unit {
            if !UNITS.contains(&u.as_str()) {
                r.warn(
                    &format!("{pp}/unit"),
                    format!("{u:?} is not a unit this version knows; read as other"),
                );
            }
        }
    }
    let mut overridden = HashSet::new();
    for (i, o) in d.overrides.iter().enumerate() {
        let op = format!("{p}/overrides/{i}");
        if !overridden.insert(o.param.as_str()) {
            r.error(&format!("{op}/param"), "listed twice");
        }
        if let Some(n) = o.normalized {
            check_unit_interval(r, &format!("{op}/normalized"), n);
        }
    }
    if !doc.dependencies.iter().any(|dep| dep.id == d.plugin) && !d.plugin.starts_with("builtin:") {
        r.warn(
            &format!("{p}/plugin"),
            "no dependencies entry for this plugin",
        );
    }
    check_extensions(r, p, &d.extensions, doc);
}

fn check_event(r: &mut Report, p: &str, e: &Event, promoted: &HashSet<&str>, doc: &Document) {
    match e {
        Event::Note(n) => {
            check_tick(r, &format!("{p}/at"), n.at);
            if n.length <= 0 {
                r.error(&format!("{p}/length"), "not above 0");
            }
            check_tick(r, &format!("{p}/length"), n.length);
            if n.pitch > 127 {
                r.error(&format!("{p}/pitch"), "above 127");
            }
            check_unit_interval(r, &format!("{p}/velocity"), n.velocity);
            check_unit_interval(r, &format!("{p}/release"), n.release);
            if let Some(x) = &n.expressions {
                for (name, points, lo, hi) in [
                    ("pitch", &x.pitch, f64::NEG_INFINITY, f64::INFINITY),
                    ("pressure", &x.pressure, 0.0, 1.0),
                    ("timbre", &x.timbre, 0.0, 1.0),
                    ("volume", &x.volume, 0.0, 1.0),
                    ("pan", &x.pan, -1.0, 1.0),
                ] {
                    let xp = format!("{p}/expressions/{name}");
                    check_increasing(r, &xp, points.iter().map(|pt| pt.at));
                    for (i, pt) in points.iter().enumerate() {
                        if pt.value < lo || pt.value > hi {
                            r.error(
                                &format!("{xp}/{i}/value"),
                                format!("not within {lo} and {hi}"),
                            );
                        }
                    }
                }
            }
            if let Some(a) = &n.audio {
                check_audio(r, &format!("{p}/audio"), a, doc);
            }
            check_extensions(r, p, &n.extensions, doc);
        }
        Event::Chord(c) => {
            check_tick(r, &format!("{p}/at"), c.at);
            if c.length <= 0 {
                r.error(&format!("{p}/length"), "not above 0");
            }
            if c.root > 11 {
                r.error(&format!("{p}/root"), "above 11");
            }
            if c.bass.is_some_and(|b| b > 11) {
                r.error(&format!("{p}/bass"), "above 11");
            }
            if !QUALITIES.contains(&c.quality.as_str()) {
                r.warn(
                    &format!("{p}/quality"),
                    format!(
                        "{:?} is not a quality this version knows; its notes play",
                        c.quality
                    ),
                );
            }
            if c.notes.is_empty() {
                r.error(
                    &format!("{p}/notes"),
                    "empty: a chord carries the notes to play",
                );
            }
            for (i, n) in c.notes.iter().enumerate() {
                if *n > 127 {
                    r.error(&format!("{p}/notes/{i}"), "above 127");
                }
            }
            check_unit_interval(r, &format!("{p}/velocity"), c.velocity);
            check_extensions(r, p, &c.extensions, doc);
        }
        Event::Param(pe) => {
            check_tick(r, &format!("{p}/at"), pe.at);
            // `value` is the plain value (spec 6.7, 7.3): any finite number.
            if !promoted.contains(pe.param.as_str()) {
                r.warn(
                    &format!("{p}/param"),
                    format!(
                        "{:?} is not a promoted parameter of this track; ignored",
                        pe.param
                    ),
                );
            }
            check_extensions(r, p, &pe.extensions, doc);
        }
        Event::Unknown(map) => {
            let kind = map
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            if kind.is_empty() {
                r.error(&format!("{p}/type"), "an event needs a type");
            } else {
                r.warn(
                    &format!("{p}/type"),
                    format!("{kind:?} is not an event type this version knows; kept, not played"),
                );
            }
        }
    }
}

fn check_audio(r: &mut Report, p: &str, a: &AudioPiece, doc: &Document) {
    if !is_reference(&a.file, "audio") {
        r.error(&format!("{p}/file"), "not a reference into audio/");
    }
    if !is_sha256(&a.sha256) {
        r.error(
            &format!("{p}/sha256"),
            "not 64 lower-case hexadecimal digits",
        );
    }
    if a.sample_rate == 0 || a.sample_rate > SAMPLE_RATE_MAX {
        r.error(
            &format!("{p}/sample_rate"),
            format!("not within 1 and {SAMPLE_RATE_MAX}"),
        );
    }
    if a.channels == 0 || a.channels > CHANNELS_MAX {
        r.error(
            &format!("{p}/channels"),
            format!("not within 1 and {CHANNELS_MAX}"),
        );
    }
    if !(a.from < a.to && a.to <= a.frames) {
        r.error(p, "from < to <= frames does not hold");
    }
    if a.map.is_empty() {
        r.error(&format!("{p}/map"), "empty: at least one point");
    }
    for w in a.map.windows(2) {
        if w[1].frame <= w[0].frame || w[1].tick <= w[0].tick {
            r.error(
                &format!("{p}/map"),
                "not strictly increasing in both frame and tick",
            );
            break;
        }
    }
    for (i, pt) in a.map.iter().enumerate() {
        if pt.tick.abs() > TICK_MAX {
            r.error(&format!("{p}/map/{i}/tick"), format!("beyond ±{TICK_MAX}"));
        }
    }
    match a.bpm {
        Some(bpm) => check_bpm(r, &format!("{p}/bpm"), bpm),
        None if a.map.len() > 1 => r.error(&format!("{p}/bpm"), "null needs exactly one map point"),
        None => {}
    }
    check_gain(r, &format!("{p}/gain"), a.gain);
    if !["as_recorded", "swap", "mono", "left", "right"].contains(&a.channel_mode.as_str()) {
        r.warn(
            &format!("{p}/channel_mode"),
            format!(
                "{:?} is not a mode this version knows; as recorded",
                a.channel_mode
            ),
        );
    }
    for (name, fade) in [("fade_in", &a.fade_in), ("fade_out", &a.fade_out)] {
        if let Some(f) = fade {
            let fp = format!("{p}/{name}");
            if f.length < 0 || f.outside < 0 || f.outside > f.length {
                r.error(&fp, "needs 0 <= outside <= length");
            }
            if !["linear", "equal_power", "s_curve"].contains(&f.shape.as_str()) {
                r.warn(
                    &format!("{fp}/shape"),
                    format!("{:?} is not a shape this version knows; linear", f.shape),
                );
            }
        }
    }
    if !doc.files.iter().any(|f| f.path == a.file) {
        r.warn(&format!("{p}/file"), "not in the files index");
    }
    check_extensions(r, p, &a.extensions, doc);
}

fn check_lane(
    r: &mut Report,
    p: &str,
    lane: &AutomationLane,
    device_ids: &HashSet<&str>,
    promoted: &HashSet<&str>,
    ids: &Ids<'_>,
    doc: &Document,
) {
    let t = &lane.target;
    let forms = [
        t.device.is_some() || t.param.is_some(),
        t.channel.is_some(),
        t.parameter.is_some(),
    ];
    let (lo, hi): (f64, f64) = match forms.iter().filter(|f| **f).count() {
        1 if t.device.is_some() => {
            if t.param.is_none() {
                r.error(&format!("{p}/target"), "a device target needs a param");
            }
            if !device_ids.contains(t.device.as_ref().map(Id::as_str).unwrap_or("")) {
                r.warn(
                    &format!("{p}/target/device"),
                    "not a device of this track; dangling",
                );
            }
            // The plain value (spec 6.8, 7.3): the plugin's range, which the snapshot states
            // and this check does not know.
            (f64::NEG_INFINITY, f64::INFINITY)
        }
        1 if t.channel.is_some() => match t.channel.as_deref().unwrap_or("") {
            "gain" => (0.0, GAIN_MAX),
            "pan" => (-1.0, 1.0),
            "mute" => (0.0, 1.0),
            "send" => {
                match &t.to {
                    Some(to) if ids.tracks.contains(to.as_str()) => {}
                    Some(_) => r.warn(&format!("{p}/target/to"), "not a track; dangling"),
                    None => r.error(&format!("{p}/target"), "a send target needs a to"),
                }
                (0.0, GAIN_MAX)
            }
            other => {
                r.error(
                    &format!("{p}/target/channel"),
                    format!("{other:?} is not gain, pan, mute or send"),
                );
                (f64::NEG_INFINITY, f64::INFINITY)
            }
        },
        1 => {
            if !promoted.contains(t.parameter.as_deref().unwrap_or("")) {
                r.warn(
                    &format!("{p}/target/parameter"),
                    "not a promoted parameter of this track; dangling",
                );
            }
            (f64::NEG_INFINITY, f64::INFINITY)
        }
        _ => {
            r.error(
                &format!("{p}/target"),
                "exactly one of device+param, channel, or parameter",
            );
            (f64::NEG_INFINITY, f64::INFINITY)
        }
    };
    check_increasing(
        r,
        &format!("{p}/points"),
        lane.points.iter().map(|pt| pt.at),
    );
    for (i, pt) in lane.points.iter().enumerate() {
        check_tick(r, &format!("{p}/points/{i}/at"), pt.at);
        if pt.value < lo || pt.value > hi {
            r.error(
                &format!("{p}/points/{i}/value"),
                format!("not within {lo} and {hi}"),
            );
        }
        if let Curve::Other(name) = &pt.curve {
            r.warn(
                &format!("{p}/points/{i}/curve"),
                format!("{name:?} is not a curve this version knows; linear"),
            );
        }
    }
    check_extensions(r, p, &lane.extensions, doc);
}

fn check_loop_range(r: &mut Report, p: &str, l: &LoopRange, pattern_length: Option<i64>) {
    if l.start < 0 || l.start >= l.end {
        r.error(p, "needs 0 <= start < end");
    }
    if let Some(len) = pattern_length {
        if l.end > len {
            r.error(&format!("{p}/end"), "beyond the pattern's length");
        }
    }
}

fn check_key(r: &mut Report, p: &str, tonic: u8, scale: &Scale) {
    if tonic > 11 {
        r.error(&format!("{p}/tonic"), "above 11");
    }
    match scale {
        Scale::Named(name) => {
            if !SCALES.contains(&name.as_str()) {
                r.warn(
                    &format!("{p}/scale"),
                    format!("{name:?} is not a scale this version knows; chromatic"),
                );
            }
        }
        Scale::Custom { custom } => {
            let ok = custom.first() == Some(&0)
                && custom.windows(2).all(|w| w[0] < w[1])
                && custom.iter().all(|s| *s < 12);
            if !ok {
                r.error(
                    &format!("{p}/scale/custom"),
                    "semitones above the tonic, ascending, 0 first, all below 12",
                );
            }
        }
    }
}

fn check_plugin_id(r: &mut Report, p: &str, plugin: &str) {
    if !has_scheme(plugin) {
        r.warn(
            p,
            "no plugin id scheme this version knows (clap:, vst3:, au:, lv2:, vst2:, builtin:)",
        );
        return;
    }
    if let Some(class) = plugin.strip_prefix("vst3:") {
        if class.len() != 32
            || !class
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
        {
            r.warn(p, "a vst3 class id is 32 upper-case hexadecimal digits");
        }
    }
}

fn has_scheme(plugin: &str) -> bool {
    PLUGIN_SCHEMES
        .iter()
        .any(|s| plugin.starts_with(s) && plugin.len() > s.len())
}

fn check_extensions(r: &mut Report, p: &str, extensions: &Extensions, doc: &Document) {
    for ns in extensions.keys() {
        let pointer = format!(
            "{p}/extensions/{}",
            ns.replace('~', "~0").replace('/', "~1")
        );
        if !is_namespace(ns) {
            r.error(&pointer, "not a reverse-DNS namespace");
        } else if !doc.extensions_used.contains(ns) {
            r.warn(&pointer, "not listed in extensions_used");
        }
    }
}

fn check_increasing(r: &mut Report, p: &str, ticks: impl Iterator<Item = i64>) {
    let mut last: Option<i64> = None;
    for (i, t) in ticks.enumerate() {
        if last.is_some_and(|l| t <= l) {
            r.error(&format!("{p}/{i}/at"), "not strictly increasing");
            return;
        }
        last = Some(t);
    }
}

fn check_tick(r: &mut Report, p: &str, tick: i64) {
    if !(0..=TICK_MAX).contains(&tick) {
        r.error(p, format!("not within 0 and {TICK_MAX}"));
    }
}

fn check_bpm(r: &mut Report, p: &str, bpm: f64) {
    if !(bpm > 0.0 && bpm <= BPM_MAX) {
        r.error(p, format!("not above 0 and at most {BPM_MAX}"));
    }
}

fn check_gain(r: &mut Report, p: &str, gain: f64) {
    if !(0.0..=GAIN_MAX).contains(&gain) {
        r.error(p, format!("not within 0 and {GAIN_MAX}"));
    }
}

fn check_pan(r: &mut Report, p: &str, pan: f64) {
    if !(-1.0..=1.0).contains(&pan) {
        r.error(p, "not within -1 and 1");
    }
}

fn check_unit_interval(r: &mut Report, p: &str, x: f64) {
    if !(0.0..=1.0).contains(&x) {
        r.error(p, "not within 0 and 1");
    }
}

/// A cycle in the routing graph, if any, as the ids along it.
fn find_cycle(edges: &[(String, String)]) -> Option<Vec<String>> {
    let mut out: HashMap<&str, Vec<&str>> = HashMap::new();
    for (a, b) in edges {
        out.entry(a.as_str()).or_default().push(b.as_str());
    }
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Open,
        Done,
    }
    let mut marks: HashMap<&str, Mark> = HashMap::new();
    fn visit<'a>(
        node: &'a str,
        out: &HashMap<&'a str, Vec<&'a str>>,
        marks: &mut HashMap<&'a str, Mark>,
        path: &mut Vec<&'a str>,
    ) -> Option<Vec<String>> {
        match marks.get(node) {
            Some(Mark::Done) => return None,
            Some(Mark::Open) => {
                let start = path.iter().position(|n| *n == node).unwrap_or(0);
                let mut cycle: Vec<String> =
                    path[start..].iter().map(|s| (*s).to_owned()).collect();
                cycle.push(node.to_owned());
                return Some(cycle);
            }
            None => {}
        }
        marks.insert(node, Mark::Open);
        path.push(node);
        if let Some(next) = out.get(node) {
            for n in next {
                if let Some(cycle) = visit(n, out, marks, path) {
                    return Some(cycle);
                }
            }
        }
        path.pop();
        marks.insert(node, Mark::Done);
        None
    }
    let mut nodes: Vec<&str> = out.keys().copied().collect();
    nodes.sort_unstable();
    for node in nodes {
        let mut path = Vec::new();
        if let Some(cycle) = visit(node, &out, &mut marks, &mut path) {
            return Some(cycle);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read;

    fn doc(extra: &str) -> Document {
        let text = format!(
            r#"{{"format": "tutti", "version": "1.0", "generator": {{"name": "t"}}, "ppq": 960,
            "tempo": [{{"at": 0, "bpm": 120}}], "meter": [{{"at": 0, "numerator": 4, "denominator": 4}}] {extra}}}"#
        );
        read(text.as_bytes()).unwrap()
    }

    fn errors(d: &Document) -> Vec<String> {
        validate(d).errors().map(|i| i.pointer.clone()).collect()
    }

    #[test]
    fn the_smallest_document_is_valid() {
        let r = validate(&doc(""));
        assert!(r.is_valid(), "{:?}", r.issues);
        assert!(r.issues.is_empty());
    }

    #[test]
    fn forms() {
        assert!(is_file_name("Drum-loop_2.wav"));
        assert!(!is_file_name("a/b.wav"));
        assert!(!is_file_name(".wav"));
        assert!(!is_file_name("x.toolongext"));
        assert!(is_reference("audio/x.wav", "audio"));
        assert!(!is_reference("audio/../x.wav", "audio"));
        assert!(is_reference("plugins/states/8de1.clap", "plugins/states"));
        assert!(is_namespace("dk.mxm.newdawn"));
        assert!(!is_namespace("newdawn"));
        assert!(!is_namespace("Dk.mxm"));
        assert!(is_sha256(&"a".repeat(64)));
        assert!(!is_sha256(&"A".repeat(64)));
    }

    #[test]
    fn ids_must_be_unique_and_well_formed_and_never_master() {
        let d = doc(r#", "tracks": [{"id": "x"}, {"id": "x"}, {"id": "master"}, {"id": "a b"}]"#);
        let e = errors(&d);
        assert_eq!(e, ["/tracks/1/id", "/tracks/2/id", "/tracks/3/id"]);
    }

    #[test]
    fn a_part_plays_only_its_own_tracks_pattern_and_a_dangling_reference_only_warns() {
        let d = doc(
            r#", "tracks": [{"id": "a", "patterns": [{"id": "pa", "length": 960}]},
                           {"id": "b", "parts": [{"id": "p1", "pattern": "pa", "at": 0, "length": 960},
                                                 {"id": "p2", "pattern": "nope", "at": 0, "length": 960}]}]"#,
        );
        let r = validate(&d);
        assert_eq!(errors(&d), ["/tracks/1/parts/0/pattern"]);
        assert!(
            r.warnings()
                .any(|w| w.pointer == "/tracks/1/parts/1/pattern")
        );
    }

    #[test]
    fn routing_must_be_acyclic() {
        let d = doc(
            r#", "tracks": [{"id": "a", "channel": {"sends": [{"to": "b"}]}},
                           {"id": "b", "channel": {"destination": "a"}}]"#,
        );
        assert_eq!(errors(&d), ["/tracks"]);
        let fine =
            doc(r#", "tracks": [{"id": "a", "channel": {"sends": [{"to": "b"}]}}, {"id": "b"}]"#);
        assert!(validate(&fine).is_valid());
    }

    #[test]
    fn point_lists_are_strictly_increasing_and_bounds_hold() {
        let d = doc(
            r#", "key": [{"at": 0, "tonic": 0, "scale": "major"}, {"at": 0, "tonic": 12, "scale": "x"}]"#,
        );
        let e = errors(&d);
        assert!(
            e.contains(&"/key/1/at".to_owned()) && e.contains(&"/key/1/tonic".to_owned()),
            "{e:?}"
        );
        let d = doc(
            r#", "tracks": [{"id": "t", "patterns": [{"id": "p", "length": 960, "events": [
            {"type": "note", "id": "n", "at": 0, "length": 0, "pitch": 128, "velocity": 2}]}]}]"#,
        );
        let e = errors(&d);
        assert_eq!(
            e,
            [
                "/tracks/0/patterns/0/events/0/length",
                "/tracks/0/patterns/0/events/0/pitch",
                "/tracks/0/patterns/0/events/0/velocity"
            ]
        );
        let huge = doc(&format!(
            r#", "markers": [{{"id": "m", "at": {}}}]"#,
            TICK_MAX + 1
        ));
        assert_eq!(errors(&huge), ["/markers/0/at"]);
    }

    #[test]
    fn an_audio_piece_is_checked_whole() {
        let d = doc(
            r#", "files": [{"path": "audio/x.wav", "sha256": "0000000000000000000000000000000000000000000000000000000000000000", "bytes": 1}],
            "tracks": [{"id": "t", "patterns": [{"id": "p", "length": 960, "events": [
            {"type": "note", "id": "n", "at": 0, "length": 960, "pitch": 60, "velocity": 1, "audio": {
              "file": "audio/x.wav", "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
              "sample_rate": 44100, "channels": 2, "frames": 100, "from": 0, "to": 100,
              "map": [{"frame": 0, "tick": 0}, {"frame": 50, "tick": 480}], "bpm": null}}]}]}]"#,
        );
        assert_eq!(errors(&d), ["/tracks/0/patterns/0/events/0/audio/bpm"]);
    }
}
