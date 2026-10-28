//! Renders (spec 7.7): what a render stands in for, the two device lists a program usually
//! makes, the length a render's file must have, and its fingerprint, the SHA-256 of the JCS
//! bytes of one descriptor object over what the file depends on and nothing else.

use std::collections::HashSet;

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::canon;
use crate::id::Id;
use crate::model::{
    Channel, Device, Document, Event, Pattern, PatternRender, Render, Role, TempoPoint, Track,
};
use crate::time::TempoMap;

/// SHA-256 of bytes as 64 lower-case hexadecimal digits.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// --- what a render holds ---------------------------------------------------------------------

/// The track's devices in signal order (spec 7.7): its enabled note effects in list order, then
/// its enabled instruments, then its enabled audio effects; analyzers and devices of a role this
/// version does not know are left out.
pub fn signal_order(track: &Track) -> Vec<&Device> {
    let of = |role: Role| {
        track
            .devices
            .iter()
            .filter(move |d| d.enabled && d.effective_role() == role)
    };
    of(Role::NoteEffect)
        .chain(of(Role::Instrument))
        .chain(of(Role::AudioEffect))
        .collect()
}

/// The devices of a render "without effects" (spec 7.7): the enabled note effects and
/// instruments, in the track's order.
pub fn without_effects(track: &Track) -> Vec<Id> {
    listed(track, |role| {
        matches!(role, Role::NoteEffect | Role::Instrument)
    })
}

/// The devices of a render "with effects" (spec 7.7): the enabled note effects, instruments and
/// audio effects, in the track's order.
pub fn with_effects(track: &Track) -> Vec<Id> {
    listed(track, |role| {
        matches!(
            role,
            Role::NoteEffect | Role::Instrument | Role::AudioEffect
        )
    })
}

fn listed(track: &Track, keep: impl Fn(Role) -> bool) -> Vec<Id> {
    track
        .devices
        .iter()
        .filter(|d| d.enabled && keep(d.effective_role()))
        .map(|d| d.id.clone())
        .collect()
}

/// How many devices of the chain, in signal order, a render of `devices` stands in for (spec
/// 7.7), or `None` when it stands in for none: its devices, those still in the signal order,
/// must be the first ones of it, and every enabled instrument among them.
pub fn stands_in(track: &Track, devices: &[Id]) -> Option<usize> {
    let order = signal_order(track);
    let listed: HashSet<&str> = devices.iter().map(Id::as_str).collect();
    let n = order
        .iter()
        .filter(|d| listed.contains(d.id.as_str()))
        .count();
    if !order[..n].iter().all(|d| listed.contains(d.id.as_str())) {
        return None;
    }
    let past_instruments = order
        .iter()
        .rposition(|d| d.effective_role() == Role::Instrument)
        .map_or(0, |i| i + 1);
    if n < past_instruments {
        return None;
    }
    Some(n)
}

/// The frames a track render from `from` to `to` holds at `rate` (spec 7.7): the sample at `to`
/// minus the sample at `from`, through the document's tempo map.
pub fn track_render_frames(doc: &Document, from: i64, to: i64, rate: u32) -> i64 {
    let map = TempoMap::new(doc.ppq, &doc.tempo);
    map.samples_at(to as f64, rate) - map.samples_at(from as f64, rate)
}

/// The frames a pattern render from `from` to `to` at `bpm` holds at `rate` (spec 7.7): the
/// sample of `to`, counted from `from`, at that one tempo.
pub fn pattern_render_frames(ppq: u32, from: i64, to: i64, bpm: f64, rate: u32) -> i64 {
    TempoMap::new(ppq, &[TempoPoint::new(0, bpm)]).samples_at((to - from) as f64, rate)
}

// --- fingerprints ------------------------------------------------------------------------------

/// The track reduced (spec 7.7) to what a render of `devices` can depend on: no renders,
/// children, name, colour, drum map or slots; the channel without what acts after the tap; only
/// the listed devices, unnamed; only the promoted parameters and the automation lanes that reach
/// one of them; each pattern reduced; each part unnamed. Everything else stays, unknown keys
/// included.
fn reduced_track(track: &Track, devices: &[Id]) -> Track {
    let listed: HashSet<&str> = devices.iter().map(Id::as_str).collect();
    let mut t = track.clone();
    t.renders.clear();
    t.children.clear();
    t.name.clear();
    t.color = None;
    t.drum_map = None;
    t.slots.clear();
    t.channel = Channel {
        channels: track.channel.channels,
        unknown: track.channel.unknown.clone(),
        ..Channel::default()
    };
    t.devices.retain(|d| listed.contains(d.id.as_str()));
    for d in &mut t.devices {
        d.name.clear();
    }
    t.parameters.retain(|p| listed.contains(p.device.as_str()));
    let kept: HashSet<String> = t.parameters.iter().map(|p| p.name.clone()).collect();
    t.automation.retain(|lane| {
        lane.target
            .device
            .as_ref()
            .is_some_and(|d| listed.contains(d.as_str()))
            || lane
                .target
                .parameter
                .as_ref()
                .is_some_and(|name| kept.contains(name))
    });
    for p in &mut t.patterns {
        reduce_pattern(p, &kept);
    }
    for part in &mut t.parts {
        part.name = None;
    }
    t
}

/// A pattern reduced (spec 7.7): no renders, no name, only the parameter events whose
/// parameter is kept.
fn reduce_pattern(pattern: &mut Pattern, kept: &HashSet<String>) {
    pattern.renders.clear();
    pattern.name.clear();
    pattern.events.retain(|e| match e {
        Event::Param(p) => kept.contains(&p.param),
        _ => true,
    });
}

fn kept_parameters(track: &Track, devices: &[Id]) -> HashSet<String> {
    let listed: HashSet<&str> = devices.iter().map(Id::as_str).collect();
    track
        .parameters
        .iter()
        .filter(|p| listed.contains(p.device.as_str()))
        .map(|p| p.name.clone())
        .collect()
}

/// The folders above the track with `id`, the outermost first: none for a track at the top, or
/// for one the document doesn't hold.
pub fn folders_above<'a>(doc: &'a Document, id: &Id) -> Vec<&'a Track> {
    fn walk<'a>(tracks: &'a [Track], id: &Id, path: &mut Vec<&'a Track>) -> bool {
        for t in tracks {
            if &t.id == id {
                return true;
            }
            path.push(t);
            if walk(&t.children, id, path) {
                return true;
            }
            path.pop();
        }
        false
    }
    let mut path = Vec::new();
    if walk(&doc.tracks, id, &mut path) {
        path
    } else {
        Vec::new()
    }
}

/// The folders above a track whose notes can reach it (spec 7.1), as a track render's descriptor
/// holds them (spec 7.7): those with patterns or enabled note effects, the outermost first, each
/// reduced as a track is, its enabled note effects standing for the listed devices.
fn reduced_folders(doc: &Document, track: &Track) -> Vec<Track> {
    folders_above(doc, &track.id)
        .into_iter()
        .filter_map(|folder| {
            let note_effects = listed(folder, |role| role == Role::NoteEffect);
            (!folder.patterns.is_empty() || !note_effects.is_empty())
                .then(|| reduced_track(folder, &note_effects))
        })
        .collect()
}

fn maps(doc: &Document) -> (Value, Value, Value) {
    let doc_value = serde_json::to_value(doc).expect("a document always serialises");
    let field = |name: &str| {
        doc_value
            .get(name)
            .cloned()
            .unwrap_or(Value::Array(Vec::new()))
    };
    (field("tempo"), field("meter"), field("key"))
}

/// The descriptor a track render's fingerprint is over (spec 7.7): the track reduced to what a
/// render of `devices` depends on; the folders above it whose notes reach it, when any; `from`
/// and `to`; the document's `ppq`, `tempo`, `meter` and `key`.
pub fn render_descriptor(
    doc: &Document,
    track: &Track,
    devices: &[Id],
    from: i64,
    to: i64,
) -> Value {
    let track_value =
        serde_json::to_value(reduced_track(track, devices)).expect("a track always serialises");
    let (tempo, meter, key) = maps(doc);
    let mut descriptor = Map::new();
    descriptor.insert("track".to_owned(), track_value);
    let folders = reduced_folders(doc, track);
    if !folders.is_empty() {
        descriptor.insert(
            "folders".to_owned(),
            serde_json::to_value(folders).expect("a track always serialises"),
        );
    }
    descriptor.insert("from".to_owned(), json!(from));
    descriptor.insert("to".to_owned(), json!(to));
    descriptor.insert("ppq".to_owned(), json!(doc.ppq));
    descriptor.insert("tempo".to_owned(), tempo);
    descriptor.insert("meter".to_owned(), meter);
    descriptor.insert("key".to_owned(), key);
    Value::Object(descriptor)
}

/// The fingerprint of a track render of `devices` from `from` to `to` (spec 7.7).
pub fn render_fingerprint(
    doc: &Document,
    track: &Track,
    devices: &[Id],
    from: i64,
    to: i64,
) -> String {
    sha256_hex(canon::jcs(&render_descriptor(doc, track, devices, from, to)).as_bytes())
}

/// The descriptor a pattern render's fingerprint is over (spec 7.7): the track reduced, without
/// its patterns, parts and automation; the pattern reduced; `from`, `to` and `bpm`; the
/// document's `ppq`, `meter` and `key`.
pub fn pattern_render_descriptor(
    doc: &Document,
    track: &Track,
    pattern: &Pattern,
    devices: &[Id],
    from: i64,
    to: i64,
    bpm: f64,
) -> Value {
    let mut t = reduced_track(track, devices);
    t.patterns.clear();
    t.parts.clear();
    t.automation.clear();
    let mut p = pattern.clone();
    reduce_pattern(&mut p, &kept_parameters(track, devices));
    let (_, meter, key) = maps(doc);
    let mut descriptor = Map::new();
    descriptor.insert(
        "track".to_owned(),
        serde_json::to_value(t).expect("a track always serialises"),
    );
    descriptor.insert(
        "pattern".to_owned(),
        serde_json::to_value(p).expect("a pattern always serialises"),
    );
    descriptor.insert("from".to_owned(), json!(from));
    descriptor.insert("to".to_owned(), json!(to));
    descriptor.insert("bpm".to_owned(), json!(bpm));
    descriptor.insert("ppq".to_owned(), json!(doc.ppq));
    descriptor.insert("meter".to_owned(), meter);
    descriptor.insert("key".to_owned(), key);
    Value::Object(descriptor)
}

/// The fingerprint of a render of `pattern` through `devices` from `from` to `to` at `bpm`
/// (spec 7.7).
pub fn pattern_render_fingerprint(
    doc: &Document,
    track: &Track,
    pattern: &Pattern,
    devices: &[Id],
    from: i64,
    to: i64,
    bpm: f64,
) -> String {
    sha256_hex(
        canon::jcs(&pattern_render_descriptor(
            doc, track, pattern, devices, from, to, bpm,
        ))
        .as_bytes(),
    )
}

/// Whether a track render still matches its track (spec 7.7: not stale).
pub fn is_current(doc: &Document, track: &Track, render: &Render) -> bool {
    render_fingerprint(doc, track, &render.devices, render.from, render.to) == render.fingerprint
}

/// Whether a pattern render still matches its pattern and its track (spec 7.7: not stale).
pub fn is_pattern_render_current(
    doc: &Document,
    track: &Track,
    pattern: &Pattern,
    render: &PatternRender,
) -> bool {
    pattern_render_fingerprint(
        doc,
        track,
        pattern,
        &render.devices,
        render.from,
        render.to,
        render.bpm,
    ) == render.fingerprint
}

// --- every render of a document ----------------------------------------------------------------

/// One render of a document, as [`renders`] finds it.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderState {
    /// Its JSON pointer: `/tracks/0/renders/1`, `/tracks/2/patterns/0/renders/0`.
    pub pointer: String,
    pub file: String,
    pub name: String,
    pub track: Id,
    /// The pattern, for a pattern render.
    pub pattern: Option<Id>,
    pub devices: Vec<Id>,
    /// Whether its fingerprint still matches (not stale).
    pub current: bool,
    /// How many devices of the chain it stands in for (see [`stands_in`]).
    pub stands_in: Option<usize>,
    /// The frames the spec says its file holds, and the frames it states.
    pub expected_frames: i64,
    pub frames: u64,
}

/// Every render of the document, track renders before each track's pattern renders, tracks in
/// the tree's order.
pub fn renders(doc: &Document) -> Vec<RenderState> {
    fn walk(doc: &Document, tracks: &[Track], prefix: &str, out: &mut Vec<RenderState>) {
        for (i, t) in tracks.iter().enumerate() {
            let tp = format!("{prefix}/{i}");
            for (j, r) in t.renders.iter().enumerate() {
                out.push(RenderState {
                    pointer: format!("{tp}/renders/{j}"),
                    file: r.file.clone(),
                    name: r.name.clone(),
                    track: t.id.clone(),
                    pattern: None,
                    devices: r.devices.clone(),
                    current: is_current(doc, t, r),
                    stands_in: stands_in(t, &r.devices),
                    expected_frames: track_render_frames(doc, r.from, r.to, r.sample_rate),
                    frames: r.frames,
                });
            }
            for (k, p) in t.patterns.iter().enumerate() {
                for (j, r) in p.renders.iter().enumerate() {
                    out.push(RenderState {
                        pointer: format!("{tp}/patterns/{k}/renders/{j}"),
                        file: r.file.clone(),
                        name: r.name.clone(),
                        track: t.id.clone(),
                        pattern: Some(p.id.clone()),
                        devices: r.devices.clone(),
                        current: is_pattern_render_current(doc, t, p, r),
                        stands_in: stands_in(t, &r.devices),
                        expected_frames: pattern_render_frames(
                            doc.ppq,
                            r.from,
                            r.to,
                            r.bpm,
                            r.sample_rate,
                        ),
                        frames: r.frames,
                    });
                }
            }
            walk(doc, &t.children, &format!("{tp}/children"), out);
        }
    }
    let mut out = Vec::new();
    walk(doc, &doc.tracks, "/tracks", &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AutomationLane, ParamEvent, Point, Promoted, Target, Unknown};
    use crate::{Extensions, read};

    const DOC: &str = r#"{"format": "tutti", "version": "1.0", "generator": {"name": "t"}, "ppq": 960,
        "tempo": [{"at": 0, "bpm": 120}], "meter": [{"at": 0, "numerator": 4, "denominator": 4}],
        "tracks": [{"id": "t", "name": "Bass", "channel": {"gain": 0.5},
          "devices": [
            {"id": "inst", "role": "instrument", "plugin": "clap:a"},
            {"id": "eq", "role": "audio_effect", "plugin": "clap:b"},
            {"id": "verb", "role": "audio_effect", "plugin": "clap:c"}],
          "parameters": [{"name": "cut", "device": "inst", "param": "1"},
                         {"name": "mix", "device": "verb", "param": "2"}],
          "patterns": [{"id": "p", "length": 3840}, {"id": "q", "length": 960}]}]}"#;

    fn doc() -> Document {
        read(DOC.as_bytes()).unwrap()
    }

    fn ids(names: &[&str]) -> Vec<Id> {
        names.iter().map(|n| Id::from(*n)).collect()
    }

    fn lane(id: &str, target: Target) -> AutomationLane {
        AutomationLane {
            id: Id::from(id),
            target,
            points: vec![Point {
                at: 0,
                value: 0.5,
                curve: crate::model::Curve::Linear,
                unknown: Unknown::new(),
            }],
            extensions: Extensions::new(),
            unknown: Unknown::new(),
        }
    }

    fn device_target(device: &str) -> Target {
        Target {
            device: Some(Id::from(device)),
            param: Some("1".to_owned()),
            channel: None,
            to: None,
            parameter: None,
            unknown: Unknown::new(),
        }
    }

    fn param_event(id: &str, param: &str) -> Event {
        Event::Param(ParamEvent {
            id: Id::from(id),
            at: 0,
            param: param.to_owned(),
            value: 1.0,
            extensions: Extensions::new(),
            unknown: Unknown::new(),
        })
    }

    #[test]
    fn the_two_usual_lists_leave_out_what_is_off_an_analyzer_and_an_unknown_role() {
        let mut d = doc();
        let t = &mut d.tracks[0];
        t.devices[1].enabled = false;
        t.devices
            .push(Device::new(Id::from("meter"), "analyzer", "clap:m"));
        t.devices
            .push(Device::new(Id::from("odd"), "spatialiser", "clap:o"));
        t.devices
            .insert(0, Device::new(Id::from("arp"), "note_effect", "clap:n"));
        assert_eq!(without_effects(t), ids(&["arp", "inst"]));
        assert_eq!(with_effects(t), ids(&["arp", "inst", "verb"]));
        assert_eq!(
            signal_order(t)
                .iter()
                .map(|d| d.id.as_str())
                .collect::<Vec<_>>(),
            ["arp", "inst", "verb"]
        );
    }

    #[test]
    fn a_render_stands_in_for_the_first_devices_and_every_instrument() {
        let mut d = doc();
        let t = &mut d.tracks[0];
        assert_eq!(stands_in(t, &ids(&["inst"])), Some(1), "without effects");
        assert_eq!(
            stands_in(t, &ids(&["inst", "eq", "verb"])),
            Some(3),
            "with effects"
        );
        assert_eq!(
            stands_in(t, &ids(&["verb", "inst"])),
            None,
            "the eq between left out"
        );
        assert_eq!(
            stands_in(t, &ids(&[])),
            None,
            "a track with an instrument: notes, not audio"
        );
        // A note effect before the instrument: a list that stops before the instrument is notes.
        t.devices
            .insert(0, Device::new(Id::from("arp"), "note_effect", "clap:n"));
        assert_eq!(stands_in(t, &ids(&["arp"])), None);
        assert_eq!(
            stands_in(t, &ids(&["inst"])),
            None,
            "the note effect in front left out"
        );
        assert_eq!(stands_in(t, &ids(&["arp", "inst"])), Some(2));
        // Two instruments: both, or none.
        t.devices
            .push(Device::new(Id::from("pad"), "instrument", "clap:p"));
        assert_eq!(stands_in(t, &ids(&["arp", "inst"])), None);
        assert_eq!(stands_in(t, &ids(&["arp", "inst", "pad"])), Some(3));
        // A disabled instrument is skipped, an analyzer is no gap.
        t.devices.last_mut().unwrap().enabled = false;
        t.devices
            .insert(2, Device::new(Id::from("scope"), "analyzer", "clap:s"));
        assert_eq!(stands_in(t, &ids(&["arp", "inst", "eq"])), Some(3));
        // A device inserted after a render was made leaves it standing in for less.
        t.devices
            .insert(3, Device::new(Id::from("comp"), "audio_effect", "clap:k"));
        assert_eq!(stands_in(t, &ids(&["arp", "inst", "eq"])), None);
        assert_eq!(stands_in(t, &ids(&["arp", "inst"])), Some(2));
    }

    #[test]
    fn a_track_render_follows_what_it_holds_and_nothing_else() {
        let d = doc();
        let dry = ids(&["inst"]);
        let wet = ids(&["inst", "eq", "verb"]);
        let fp =
            |d: &Document, devices: &[Id]| render_fingerprint(d, &d.tracks[0], devices, 0, 3840);
        let (dry0, wet0) = (fp(&d, &dry), fp(&d, &wet));
        assert_eq!(dry0.len(), 64);
        assert_ne!(dry0, wet0);

        let mut e = d.clone();
        e.tracks[0].devices[2].enabled = false;
        assert_eq!(fp(&e, &dry), dry0, "an effect the render doesn't hold");
        assert_ne!(fp(&e, &wet), wet0, "an effect it holds, switched off");

        let mut e = d.clone();
        e.tracks[0].channel.gain = 0.25;
        e.tracks[0].channel.pan = -0.5;
        e.tracks[0].name = "Bass 2".to_owned();
        e.tracks[0].color = Some("#ff0000".to_owned());
        assert_eq!(
            fp(&e, &wet),
            wet0,
            "the fader, the pan, the name, the colour"
        );
        let mut e = d.clone();
        e.tracks[0].channel = Channel::default();
        assert_eq!(
            fp(&e, &wet),
            wet0,
            "a channel at its defaults is the same as one at gain 0.5"
        );

        let mut e = d.clone();
        e.tracks[0]
            .automation
            .push(lane("aut_v", device_target("verb")));
        assert_eq!(fp(&e, &dry), dry0, "automation of a device it doesn't hold");
        assert_ne!(fp(&e, &wet), wet0, "automation of one it holds");

        let mut e = d.clone();
        e.tracks[0].patterns[0]
            .events
            .push(param_event("evt_m", "mix"));
        assert_eq!(
            fp(&e, &dry),
            dry0,
            "a parameter event of a device it doesn't hold"
        );
        assert_ne!(fp(&e, &wet), wet0, "one of a device it holds");

        let mut e = d.clone();
        e.tracks[0].devices.swap(1, 2);
        assert_ne!(fp(&e, &wet), wet0, "the order of the devices it holds");
        assert_eq!(fp(&e, &dry), dry0);

        let mut e = d.clone();
        e.tracks[0].patterns[1].length = 1920;
        assert_ne!(fp(&e, &dry), dry0, "a pattern of the track");
        let mut e = d.clone();
        e.tempo[0].bpm = 121.0;
        assert_ne!(fp(&e, &dry), dry0, "the tempo map");
        let mut e = d.clone();
        e.tracks[0]
            .extensions
            .insert("org.example.x".to_owned(), json!(1));
        assert_ne!(fp(&e, &dry), dry0, "the track's extensions");
        let mut e = d.clone();
        e.tracks[0]
            .unknown
            .insert(crate::model::Key("later".to_owned()), json!(true));
        assert_ne!(fp(&e, &dry), dry0, "a key a later version adds");
        assert_ne!(
            render_fingerprint(&d, &d.tracks[0], &dry, 0, 1920),
            dry0,
            "the range"
        );

        // Adding a pattern render, or another track render, leaves it current.
        let mut e = d.clone();
        let facts = crate::model::AudioFacts {
            sample_rate: 48000,
            channels: 2,
            frames: 96000,
        };
        e.tracks[0].patterns[0].renders.push(PatternRender::new(
            "renders/p.wav",
            &sha256_hex(b"p"),
            facts,
            dry.clone(),
            0,
            3840,
            120.0,
        ));
        e.tracks[0].renders.push(Render::new(
            "renders/w.wav",
            &sha256_hex(b"w"),
            facts,
            wet.clone(),
            0,
            3840,
        ));
        assert_eq!(fp(&e, &dry), dry0);
    }

    #[test]
    fn a_pattern_render_follows_its_pattern_its_devices_and_the_track_but_no_other_pattern() {
        let d = doc();
        let dry = ids(&["inst"]);
        let fp = |d: &Document| {
            pattern_render_fingerprint(
                d,
                &d.tracks[0],
                &d.tracks[0].patterns[0],
                &dry,
                0,
                3840,
                120.0,
            )
        };
        let base = fp(&d);
        let mut e = d.clone();
        e.tracks[0].patterns[1].length = 1920;
        assert_eq!(fp(&e), base, "another pattern");
        let mut e = d.clone();
        e.tracks[0]
            .automation
            .push(lane("aut_i", device_target("inst")));
        assert_eq!(
            fp(&e),
            base,
            "track automation doesn't play in a pattern render"
        );
        let mut e = d.clone();
        e.tracks[0].patterns[0]
            .events
            .push(param_event("evt_c", "cut"));
        assert_ne!(fp(&e), base, "a parameter event of a device it holds");
        let mut e = d.clone();
        e.tracks[0].patterns[0]
            .events
            .push(param_event("evt_m", "mix"));
        assert_eq!(fp(&e), base, "one of a device it doesn't hold");
        let mut e = d.clone();
        e.tracks[0].patterns[0].name = "Verse".to_owned();
        e.tracks[0].channel.gain = 1.0;
        assert_eq!(fp(&e), base, "its name, the fader");
        let mut e = d.clone();
        e.tracks[0].devices[0].enabled = false;
        assert_ne!(fp(&e), base, "its instrument off");
        let mut e = d.clone();
        e.tracks[0]
            .extensions
            .insert("org.example.x".to_owned(), json!(1));
        assert_ne!(fp(&e), base, "the track's extensions");
        let mut e = d.clone();
        e.tempo[0].bpm = 90.0;
        assert_eq!(fp(&e), base, "the tempo map: the render has its own bpm");
        assert_ne!(
            pattern_render_fingerprint(
                &d,
                &d.tracks[0],
                &d.tracks[0].patterns[0],
                &dry,
                0,
                3840,
                121.0
            ),
            base,
            "its bpm"
        );
        let mut e = d.clone();
        e.tracks[0].parameters.push(Promoted {
            name: "new".to_owned(),
            device: Id::from("verb"),
            param: "9".to_owned(),
            unknown: Unknown::new(),
        });
        assert_eq!(
            fp(&e),
            base,
            "a parameter promoted from a device it doesn't hold"
        );
    }

    #[test]
    fn a_render_is_exactly_its_range() {
        let d = doc();
        // One bar at 120 bpm is 2 s: 96 000 frames at 48 kHz, as the spec's worked example.
        assert_eq!(pattern_render_frames(960, 0, 3840, 120.0, 48000), 96000);
        assert_eq!(track_render_frames(&d, 0, 3840, 48000), 96000);
        assert_eq!(track_render_frames(&d, 960, 3840, 44100), 66150);
    }

    #[test]
    fn every_render_is_found_with_its_pointer_and_state() {
        let mut d = doc();
        let facts = crate::model::AudioFacts {
            sample_rate: 48000,
            channels: 2,
            frames: 96000,
        };
        let devices = ids(&["inst"]);
        let mut r = Render::new(
            "renders/a.wav",
            &sha256_hex(b"a"),
            facts,
            devices.clone(),
            0,
            3840,
        );
        r.fingerprint = render_fingerprint(&d, &d.tracks[0], &devices, 0, 3840);
        d.tracks[0].renders.push(r);
        let mut p = PatternRender::new(
            "renders/b.wav",
            &sha256_hex(b"b"),
            facts,
            devices.clone(),
            0,
            3840,
            120.0,
        );
        p.fingerprint = "0".repeat(64);
        d.tracks[0].patterns[0].renders.push(p);
        let found = renders(&d);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].pointer, "/tracks/0/renders/0");
        assert!(found[0].current);
        assert_eq!(found[0].stands_in, Some(1));
        assert_eq!(found[0].expected_frames, 96000);
        assert_eq!(found[1].pointer, "/tracks/0/patterns/0/renders/0");
        assert_eq!(found[1].pattern, Some(Id::from("p")));
        assert!(!found[1].current, "a fingerprint that never matched");
    }

    /// The document's track inside a folder inside another, both `outer` and `inner` as given.
    fn nested(outer: Track, inner: Track) -> Document {
        let mut d = doc();
        let track = d.tracks.remove(0);
        let mut inner = inner;
        inner.children = vec![track];
        let mut outer = outer;
        outer.children = vec![inner];
        d.tracks = vec![outer];
        d
    }

    fn fingerprint_of(d: &Document) -> String {
        let t = d.track(&Id::from("t")).unwrap();
        render_fingerprint(d, t, &ids(&["inst"]), 0, 3840)
    }

    #[test]
    fn a_track_render_holds_the_notes_its_folders_give_it() {
        let alone = doc();
        let at_top = render_fingerprint(&alone, &alone.tracks[0], &ids(&["inst"]), 0, 3840);
        let plain = |id: &str| Track::new(Id::from(id), "Folder");
        // Folders holding no notes leave the descriptor as it was: regrouping never stales.
        let d = nested(plain("o"), plain("i"));
        assert_eq!(fingerprint_of(&d), at_top);
        let t = d.track(&Id::from("t")).unwrap();
        assert!(
            render_descriptor(&d, t, &ids(&["inst"]), 0, 3840)
                .get("folders")
                .is_none()
        );
        // Notes on the outer folder reach the track through the inner one.
        let mut outer = plain("o");
        outer.patterns.push(Pattern::new(Id::from("op"), 1920));
        outer
            .devices
            .push(Device::new(Id::from("gate"), "audio_effect", "clap:g"));
        let d = nested(outer, plain("i"));
        let held = fingerprint_of(&d);
        assert_ne!(held, at_top);
        let t = d.track(&Id::from("t")).unwrap();
        let folders = render_descriptor(&d, t, &ids(&["inst"]), 0, 3840)["folders"].clone();
        assert_eq!(
            folders.as_array().unwrap().len(),
            1,
            "only the one with notes"
        );
        assert_eq!(folders[0]["id"], "o");
        assert!(
            folders[0].get("devices").is_none(),
            "its audio effect isn't listed"
        );
        // The folder's pattern changes it; its audio effect, its fader and its name don't.
        let mut e = d.clone();
        e.tracks[0].patterns[0].length = 3840;
        assert_ne!(fingerprint_of(&e), held);
        let mut e = d.clone();
        e.tracks[0].devices[0].enabled = false;
        e.tracks[0].channel.gain = 0.25;
        e.tracks[0].name = "Renamed".to_owned();
        assert_eq!(fingerprint_of(&e), held);
        // A note effect on the inner folder passes the outer folder's notes on, so it counts.
        let mut e = d.clone();
        e.tracks[0].children[0]
            .devices
            .push(Device::new(Id::from("arp"), "note_effect", "clap:n"));
        let t = e.track(&Id::from("t")).unwrap();
        let folders = render_descriptor(&e, t, &ids(&["inst"]), 0, 3840)["folders"].clone();
        assert_eq!(folders.as_array().unwrap().len(), 2);
        assert_eq!(folders[1]["devices"][0]["id"], "arp");
    }

    #[test]
    fn the_folders_above_a_track_are_found_outermost_first() {
        let plain = |id: &str| Track::new(Id::from(id), "Folder");
        let d = nested(plain("o"), plain("i"));
        let above: Vec<&str> = folders_above(&d, &Id::from("t"))
            .iter()
            .map(|t| t.id.as_str())
            .collect();
        assert_eq!(above, ["o", "i"]);
        assert!(folders_above(&d, &Id::from("o")).is_empty());
        assert!(folders_above(&d, &Id::from("nowhere")).is_empty());
    }

    #[test]
    fn sha256_is_the_standard_one() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
