//! Writes `examples/`: the example projects (`minimal`, `typical`, `every-feature`) and the
//! conformance fixtures of Levels 0, 1 and 2 and the hostile ones, from the crate itself, so
//! every document is the pretty form and every hash and fingerprint is real. Deterministic: ids
//! from fixed times, audio from closed forms, so running it again changes nothing (`cargo run -p
//! tutti-schema --bin make-examples`). A new case goes after the ones before it, so their ids
//! stay as they are.

use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tutti::folder::Store;
use tutti::render::{render_fingerprint, sha256_hex};
use tutti::*;
use tutti_fs::DirStore;

const RATE: u32 = 44_100;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let examples = root.join("examples");
    let mut ids = Ids::default();
    write_project(&examples.join("minimal"), "Minimal", minimal(&mut ids));
    write_project(&examples.join("typical"), "Typical", typical(&mut ids));
    write_project(
        &examples.join("every-feature"),
        "Every-feature",
        every_feature(&mut ids),
    );
    let l0 = examples.join("conformance/l0");
    for (name, project, expected) in fixtures(&mut ids) {
        write_project(&l0.join(name), &title_case(name), project);
        write_text(
            &l0.join(name).join("expected.json"),
            &format!("{}\n", serde_json::to_string_pretty(&expected).unwrap()),
        );
    }
    let hostile_dir = examples.join("conformance/hostile");
    let _ = fs::remove_dir_all(&hostile_dir);
    for case in hostile(&mut ids) {
        let folder = hostile_dir.join(case.name);
        fs::create_dir_all(&folder).unwrap();
        for (reference, bytes) in &case.files {
            let path = folder.join(reference);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        let mut expected = json!({"what": case.what, "error": case.error});
        if let Some((path, target)) = case.link {
            expected["link"] = json!({"path": path, "target": target});
        }
        write_text(
            &folder.join("expected.json"),
            &format!("{}\n", serde_json::to_string_pretty(&expected).unwrap()),
        );
    }
    // After the hostile cases, so the ids of everything above stay as they were.
    let l1 = examples.join("conformance/l1");
    for fixture in level_1(&mut ids) {
        let folder = l1.join(fixture.name);
        write_project(&folder, &title_case(fixture.name), fixture.project);
        write_json(&folder.join("expected.json"), &fixture.expected);
    }
    let l2 = examples.join("conformance/l2");
    everything(&l2.join("everything"), &mut ids);
    merge_different_tracks(&l2.join("merge-different-tracks"), &mut ids);
    // Cases added since go here, each after the one before, so the ids above stay as they were.
    let fixture = render_choice(&mut ids);
    let folder = l1.join(fixture.name);
    write_project(&folder, &title_case(fixture.name), fixture.project);
    write_json(&folder.join("expected.json"), &fixture.expected);
    println!("examples written under {}", examples.display());
}

fn write_json(path: &Path, value: &Value) {
    write_text(
        path,
        &format!("{}\n", serde_json::to_string_pretty(value).unwrap()),
    );
}

fn title_case(name: &str) -> String {
    let mut c = name.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

// --- a project in the making --------------------------------------------------------------------

/// A project with the files it carries, before it is written.
struct Project {
    document: Document,
    /// Reference (`audio/x.wav`) to bytes.
    files: BTreeMap<String, Vec<u8>>,
}

impl Project {
    fn new(generator: Generator) -> Self {
        Self {
            document: Document::new(generator),
            files: BTreeMap::new(),
        }
    }

    /// Puts a file in the folder and the index, and returns its hash.
    fn add_file(&mut self, reference: &str, bytes: Vec<u8>) -> String {
        let hash = sha256_hex(&bytes);
        self.document.files.push(FileEntry {
            path: reference.to_owned(),
            sha256: hash.clone(),
            bytes: bytes.len() as u64,
            unknown: Unknown::new(),
        });
        self.files.insert(reference.to_owned(), bytes);
        hash
    }

    /// A state sidecar from its bytes, named by content.
    fn add_state(&mut self, bytes: Vec<u8>, kind: &str) -> StateRef {
        let hash = sha256_hex(&bytes);
        self.add_file(&format!("plugins/states/{hash}.{kind}"), bytes);
        StateRef {
            sha256: hash,
            kind: kind.to_owned(),
            unknown: Unknown::new(),
        }
    }
}

/// Writes an example as any writer does: the files into the store, then the document through
/// `Project::save`, which indexes and verifies them and writes the README.
fn write_project(folder: &Path, title: &str, mut project: Project) {
    let _ = fs::remove_dir_all(folder);
    project.document.metadata.title = Some(title.replace('-', " "));
    let store = DirStore::new(folder);
    store.make_folder("").expect("the folder");
    for (reference, bytes) in &project.files {
        store
            .write_new(reference, bytes)
            .expect("a file of the example");
    }
    let generator = project.document.generator.clone();
    let mut example = tutti::Project::adopt(store, &format!("{title}.tutti"), project.document)
        .expect("a document name");
    match example.save(&generator) {
        Ok(saved) => {
            for issue in &saved.warnings {
                println!("  {} {title}: {issue}", folder.display());
            }
        }
        Err(e) => panic!("{title} must save: {e}"),
    }
}

fn write_text(path: &Path, text: &str) {
    fs::write(path, text.as_bytes()).expect("written");
}

fn generator() -> Generator {
    Generator::new("tutti-examples", VERSION, REPOSITORY_URL)
}

/// Deterministic ids: a fixed time, counting up.
#[derive(Default)]
struct Ids {
    n: u64,
}

impl Ids {
    fn next(&mut self, prefix: &str) -> Id {
        self.n += 1;
        Id::ulid(
            prefix,
            1_760_000_000_000 + self.n,
            0x5A5A_0000_0000_0000_0000 + self.n as u128,
        )
    }
}

// --- audio made, never taken ----------------------------------------------------------------------

/// 16-bit PCM WAV from interleaved samples in -1..1.
fn wav(channels: u16, rate: u32, samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

/// A click loop: `bars` bars of 4/4 at `bpm`, a 5 ms decaying 1 kHz burst on every beat, mono.
fn click_loop(bpm: f64, bars: u32, rate: u32) -> (Vec<f32>, u64) {
    let frames = ((bars * 4) as f64 * 60.0 / bpm * rate as f64).round() as u64;
    let mut out = vec![0.0f32; frames as usize];
    let beat = 60.0 / bpm * rate as f64;
    for b in 0..(bars * 4) {
        let start = (b as f64 * beat).round() as usize;
        for i in 0..(rate as usize * 5 / 1000) {
            if start + i < out.len() {
                let t = i as f64 / rate as f64;
                out[start + i] = ((2.0 * PI * 1000.0 * t).sin() * (-t * 800.0).exp() * 0.8) as f32;
            }
        }
    }
    (out, frames)
}

/// The rate the examples' renders are made at: low, so the files stay small.
const RENDER_RATE: u32 = 8_000;

/// A render's file: a mono sine at 110 Hz, exactly `frames` long.
fn render_wav(amplitude: f64, frames: i64) -> Vec<u8> {
    let samples: Vec<f32> = (0..frames)
        .map(|i| (amplitude * (2.0 * PI * 110.0 * i as f64 / RENDER_RATE as f64).sin()) as f32)
        .collect();
    wav(1, RENDER_RATE, &samples)
}

/// A track render of `devices` over `range`, exactly its length, and its file (spec 7.7).
fn track_render(
    doc: &Document,
    track: &Track,
    file: &str,
    name: &str,
    devices: Vec<Id>,
    (from, to): (i64, i64),
    amplitude: f64,
) -> (Render, Vec<u8>) {
    let frames = render::track_render_frames(doc, from, to, RENDER_RATE);
    let bytes = render_wav(amplitude, frames);
    let facts = AudioFacts {
        sample_rate: RENDER_RATE,
        channels: 1,
        frames: frames as u64,
    };
    let mut r = Render::new(file, &sha256_hex(&bytes), facts, devices, from, to);
    r.name = name.to_owned();
    r.fingerprint = render::render_fingerprint(doc, track, &r.devices, from, to);
    (r, bytes)
}

/// A render of a track's whole pattern through `devices` at `bpm`, exactly its length, and its
/// file (spec 7.7).
fn pattern_render(
    doc: &Document,
    track: &Track,
    pattern: &Pattern,
    file: &str,
    devices: Vec<Id>,
    bpm: f64,
) -> (PatternRender, Vec<u8>) {
    let frames = render::pattern_render_frames(doc.ppq, 0, pattern.length, bpm, RENDER_RATE);
    let bytes = render_wav(0.3, frames);
    let facts = AudioFacts {
        sample_rate: RENDER_RATE,
        channels: 1,
        frames: frames as u64,
    };
    let mut r = PatternRender::new(
        file,
        &sha256_hex(&bytes),
        facts,
        devices,
        0,
        pattern.length,
        bpm,
    );
    r.fingerprint =
        render::pattern_render_fingerprint(doc, track, pattern, &r.devices, 0, pattern.length, bpm);
    (r, bytes)
}

/// A sine, mono.
fn sine(hz: f64, seconds: f64, amplitude: f64, rate: u32) -> (Vec<f32>, u64) {
    let frames = (seconds * rate as f64).round() as u64;
    let out = (0..frames)
        .map(|i| (amplitude * (2.0 * PI * hz * i as f64 / rate as f64).sin()) as f32)
        .collect();
    (out, frames)
}

// --- the projects ----------------------------------------------------------------------------

fn minimal(_ids: &mut Ids) -> Project {
    Project::new(generator())
}

fn note(ids: &mut Ids, at: i64, length: i64, pitch: u8, velocity: f64) -> Event {
    Event::Note(Note {
        id: ids.next("evt"),
        at,
        length,
        pitch,
        velocity,
        release: 0.5,
        expressions: None,
        audio: None,
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    })
}

fn pattern(ids: &mut Ids, name: &str, length: i64, events: Vec<Event>) -> Pattern {
    let mut p = Pattern::new(ids.next("pat"), length);
    p.name = name.to_owned();
    p.events = events;
    p
}

fn part(ids: &mut Ids, pattern: &Id, at: i64, length: i64) -> Part {
    Part {
        id: ids.next("prt"),
        pattern: pattern.clone(),
        at,
        length,
        offset: 0,
        r#loop: None,
        mute: false,
        name: None,
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    }
}

fn device(ids: &mut Ids, role: &str, name: &str, plugin: &str) -> Device {
    Device {
        id: ids.next("dev"),
        role: role.to_owned(),
        name: name.to_owned(),
        plugin: plugin.to_owned(),
        aliases: Vec::new(),
        state: None,
        params: Vec::new(),
        overrides: Vec::new(),
        enabled: true,
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    }
}

fn mono_01_dependency() -> Dependency {
    Dependency {
        id: "clap:dk.mxm.mxm-mono-01".to_owned(),
        kind: "plugin".to_owned(),
        name: "mxm-mono-01".to_owned(),
        vendor: Some("MXM".to_owned()),
        version: Some("0.1.1".to_owned()),
        url: Some("https://github.com/mxm-audio/mxm-mono-01".to_owned()),
        repository: Some("https://github.com/mxm-audio/mxm-mono-01.git".to_owned()),
        commit: None,
        license: Some("GPL-3.0-or-later".to_owned()),
        formats: vec!["clap".to_owned(), "vst3".to_owned()],
        platforms: vec!["windows".to_owned(), "macos".to_owned(), "linux".to_owned()],
        unknown: Unknown::new(),
    }
}

fn audio_piece(reference: &str, hash: &str, frames: u64, bpm: Option<f64>) -> AudioPiece {
    AudioPiece {
        file: reference.to_owned(),
        sha256: hash.to_owned(),
        sample_rate: RATE,
        channels: 1,
        frames,
        name: None,
        from: 0,
        to: frames,
        map: vec![MapPoint {
            frame: 0,
            tick: 0,
            unknown: Unknown::new(),
        }],
        bpm,
        gain: 1.0,
        polarity: false,
        channel_mode: "as_recorded".to_owned(),
        fade_in: None,
        fade_out: None,
        reverse: false,
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    }
}

/// A song: drums with a drum map, a bass with a real (open-source) plugin and a state, a loop of
/// made clicks at its own tempo, chords, a reverb return with sends, automation through a
/// promoted parameter, sections.
fn typical(ids: &mut Ids) -> Project {
    let mut p = Project::new(generator());
    let state = p.add_state(
        br#"{"example": "a plugin's own state bytes"}"#.to_vec(),
        "clap",
    );
    let (samples, frames) = click_loop(95.0, 2, RATE);
    let hash = p.add_file("audio/Click-loop_95.wav", wav(1, RATE, &samples));
    let d = &mut p.document;
    d.tempo[0].bpm = 87.0;
    d.key.push(KeyPoint {
        at: 0,
        tonic: 9,
        scale: Scale::Named("minor".to_owned()),
        unknown: Unknown::new(),
    });
    d.metadata.artist = Some("tutti".to_owned());
    d.metadata.created = Some("2026-10-20".to_owned());
    d.dependencies.push(mono_01_dependency());
    let bar = 960 * 4;
    for (name, at, len) in [
        ("Intro", 0, 2 * bar),
        ("Verse", 2 * bar, 4 * bar),
        ("Chorus", 6 * bar, 4 * bar),
    ] {
        d.markers.push(Marker {
            id: ids.next("mrk"),
            at,
            name: name.to_owned(),
            kind: "section".to_owned(),
            length: Some(len),
            extensions: Extensions::new(),
            unknown: Unknown::new(),
        });
    }
    d.range = Some(Range::new(2 * bar, 6 * bar));

    // Drums: kick, snare, hats, with a drum map.
    let mut drums = Track::new(ids.next("trk"), "Drums");
    drums.color = Some("#d94a4a".to_owned());
    drums.devices.push(device(
        ids,
        "instrument",
        "Example drums",
        "builtin:example:drums",
    ));
    drums.drum_map = Some(
        [(36, "Kick"), (38, "Snare"), (42, "Hat")]
            .into_iter()
            .map(|(pitch, name)| Lane {
                pitch,
                name: name.to_owned(),
                unknown: Unknown::new(),
            })
            .collect(),
    );
    let mut events = Vec::new();
    for beat in 0..4 {
        if beat % 2 == 0 {
            events.push(note(ids, beat * 960, 240, 36, 0.9));
        } else {
            events.push(note(ids, beat * 960, 240, 38, 0.85));
        }
        events.push(note(ids, beat * 960, 120, 42, 0.5));
        events.push(note(ids, beat * 960 + 480, 120, 42, 0.35));
    }
    let beat_pattern = pattern(ids, "Beat", bar, events);
    let beat_id = beat_pattern.id.clone();
    drums.patterns.push(beat_pattern);
    for b in 2..10 {
        drums.parts.push(part(ids, &beat_id, b * bar, bar));
    }

    // Bass: a real open-source plugin with a state and a snapshot, a promoted cutoff, automation.
    let mut bass = Track::new(ids.next("trk"), "Bass");
    bass.color = Some("#4a90d9".to_owned());
    bass.channel.gain = 0.7943; // -2 dB
    let mut synth = device(ids, "instrument", "mxm-mono-01", "clap:dk.mxm.mxm-mono-01");
    synth.state = Some(state);
    synth.params.push(Param {
        id: "374".to_owned(),
        name: Some("Cutoff".to_owned()),
        value: 218.4,
        normalized: Some(0.37),
        min: Some(20.0),
        max: Some(20000.0),
        unit: Some("hertz".to_owned()),
        text: Some("218 Hz".to_owned()),
        unknown: Unknown::new(),
    });
    let synth_id = synth.id.clone();
    bass.devices.push(synth);
    bass.parameters.push(Promoted {
        name: "cutoff".to_owned(),
        device: synth_id,
        param: "374".to_owned(),
        unknown: Unknown::new(),
    });
    let line = [45u8, 45, 52, 48, 45, 45, 43, 48];
    let mut events: Vec<Event> = line
        .iter()
        .enumerate()
        .map(|(i, pitch)| note(ids, i as i64 * 480, 420, *pitch, 0.8))
        .collect();
    events.push(Event::Param(ParamEvent {
        id: ids.next("evt"),
        at: 1920,
        param: "cutoff".to_owned(),
        value: 440.0, // the plain value, hertz as the snapshot says
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    }));
    let bass_pattern = pattern(ids, "Verse bass", bar, events);
    let bass_id = bass_pattern.id.clone();
    bass.patterns.push(bass_pattern);
    for b in 2..10 {
        bass.parts.push(part(ids, &bass_id, b * bar, bar));
    }
    bass.automation.push(AutomationLane {
        id: ids.next("aut"),
        target: Target {
            device: None,
            param: None,
            channel: None,
            to: None,
            parameter: Some("cutoff".to_owned()),
            unknown: Unknown::new(),
        },
        points: vec![
            Point {
                at: 6 * bar,
                value: 120.0,
                curve: Curve::Linear,
                unknown: Unknown::new(),
            },
            Point {
                at: 10 * bar,
                value: 2400.0,
                curve: Curve::Hold,
                unknown: Unknown::new(),
            },
        ],
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    });

    // A click loop recorded at 95 bpm, played in the 87 bpm song: stretched, pitch kept.
    let mut loop_track = Track::new(ids.next("trk"), "Loop");
    let mut piece = audio_piece("audio/Click-loop_95.wav", &hash, frames, Some(95.0));
    piece.gain = 0.5;
    piece.fade_in = Some(Fade {
        length: 48,
        outside: 0,
        shape: "equal_power".to_owned(),
        unknown: Unknown::new(),
    });
    let mut n = note(ids, 0, 2 * bar, 60, 1.0);
    if let Event::Note(note) = &mut n {
        note.audio = Some(piece);
    }
    let loop_pattern = pattern(ids, "Click loop", 2 * bar, vec![n]);
    let loop_id = loop_pattern.id.clone();
    loop_track.patterns.push(loop_pattern);
    loop_track.parts.push(part(ids, &loop_id, 0, 2 * bar));

    // Chords, with their voicings.
    let mut chords = Track::new(ids.next("trk"), "Chords");
    chords.devices.push(device(
        ids,
        "instrument",
        "Example piano",
        "builtin:example:piano",
    ));
    let progression = [
        (9u8, "min", "Am", vec![57u8, 60, 64]),
        (5, "maj", "F", vec![53, 57, 60]),
        (0, "maj", "C", vec![48, 52, 55]),
        (7, "maj", "G", vec![55, 59, 62]),
    ];
    let events: Vec<Event> = progression
        .into_iter()
        .enumerate()
        .map(|(i, (root, quality, symbol, notes))| {
            Event::Chord(Chord {
                id: ids.next("evt"),
                at: i as i64 * bar,
                length: bar,
                root,
                quality: quality.to_owned(),
                bass: None,
                symbol: Some(symbol.to_owned()),
                notes,
                velocity: 0.7,
                extensions: Extensions::new(),
                unknown: Unknown::new(),
            })
        })
        .collect();
    let chord_pattern = pattern(ids, "Progression", 4 * bar, events);
    let chord_id = chord_pattern.id.clone();
    chords.patterns.push(chord_pattern);
    chords.parts.push(part(ids, &chord_id, 2 * bar, 4 * bar));
    chords.parts.push(part(ids, &chord_id, 6 * bar, 4 * bar));

    // A reverb return the chords and the loop send to.
    let mut reverb = Track::new(ids.next("trk"), "Reverb");
    reverb.devices.push(device(
        ids,
        "audio_effect",
        "Example reverb",
        "builtin:example:reverb",
    ));
    let reverb_id = reverb.id.clone();
    for (track, gain) in [(&mut chords, 0.3), (&mut loop_track, 0.15)] {
        track.channel.sends.push(Send {
            to: reverb_id.clone(),
            gain,
            pre_fader: false,
            enabled: true,
            unknown: Unknown::new(),
        });
    }

    d.tracks = vec![drums, bass, loop_track, chords, reverb];
    p
}

/// Every feature the format has, once each.
fn every_feature(ids: &mut Ids) -> Project {
    let mut p = typical(ids);
    let d = &mut p.document;
    let bar = 960 * 4;
    d.tempo = vec![
        TempoPoint {
            at: 0,
            bpm: 87.0,
            curve: Curve::Hold,
            unknown: Unknown::new(),
        },
        TempoPoint {
            at: 6 * bar,
            bpm: 87.0,
            curve: Curve::Linear,
            unknown: Unknown::new(),
        },
        TempoPoint {
            at: 8 * bar,
            bpm: 92.0,
            curve: Curve::Hold,
            unknown: Unknown::new(),
        },
    ];
    d.meter.push(MeterPoint {
        at: 8 * bar,
        numerator: 3,
        denominator: 4,
        unknown: Unknown::new(),
    });
    d.key.push(KeyPoint {
        at: 6 * bar,
        tonic: 2,
        scale: Scale::Custom {
            custom: vec![0, 2, 3, 5, 7, 9, 10],
        },
        unknown: Unknown::new(),
    });
    d.markers.push(Marker {
        id: ids.next("mrk"),
        at: 10 * bar,
        name: "Drop".to_owned(),
        kind: "marker".to_owned(),
        length: None,
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    });
    d.range.as_mut().unwrap().r#loop = true;
    d.private = vec!["notes/".to_owned()];
    d.metadata.album = Some("Examples".to_owned());
    d.metadata.year = Some("2026".to_owned());
    d.metadata.genre = Some("test".to_owned());
    d.metadata.comment = Some("Every feature of the format, once.".to_owned());
    d.metadata.copyright = Some("MIT".to_owned());
    d.dependencies[0].commit = Some("0000000000000000000000000000000000000000".to_owned());
    if let Some(chords) = d.tracks.iter_mut().find(|t| t.name == "Chords") {
        chords.channel.solo = true;
        if let Some(Event::Chord(c)) = chords.patterns[0].events.first_mut() {
            c.bass = Some(4);
            c.symbol = Some("Am/E".to_owned());
        }
    }
    d.extensions_used.push("dk.mxm.newdawn".to_owned());
    d.extensions_used.push("org.example.lyrics".to_owned());
    d.extensions_required.push("org.example.lyrics".to_owned());
    d.extensions.insert(
        "org.example.lyrics".to_owned(),
        json!({"language": "en", "lines": [{"at": 0, "text": "la"}]}),
    );
    d.master.gain = 0.891;
    d.master.devices.push(device(
        ids,
        "audio_effect",
        "Example limiter",
        "builtin:example:limiter",
    ));
    d.scenes.push(Scene {
        id: ids.next("scn"),
        name: "Verse".to_owned(),
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    });
    d.scenes.push(Scene {
        id: ids.next("scn"),
        name: "Chorus".to_owned(),
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    });
    let scene_ids: Vec<Id> = d.scenes.iter().map(|s| s.id.clone()).collect();

    // A folder holding the drums and the bass, with a group bus device and a child's own
    // destination; a VCA-less world, so a return for the folder.
    let mut drums = d.tracks.remove(0);
    let mut bass = d.tracks.remove(0);
    drums
        .extensions
        .insert("dk.mxm.newdawn".to_owned(), json!({"armed": true}));
    drums.slots.push(Slot {
        scene: scene_ids[0].clone(),
        pattern: drums.patterns[0].id.clone(),
        r#loop: None,
        unknown: Unknown::new(),
    });
    // Expressions on the first bass note, an alias and overrides on its synth.
    if let Some(Event::Note(n)) = bass.patterns[0].events.first_mut() {
        n.expressions = Some(Expressions {
            pitch: vec![
                Point {
                    at: 0,
                    value: -0.5,
                    curve: Curve::Linear,
                    unknown: Unknown::new(),
                },
                Point {
                    at: 240,
                    value: 0.0,
                    curve: Curve::Hold,
                    unknown: Unknown::new(),
                },
            ],
            pressure: vec![Point {
                at: 0,
                value: 0.6,
                curve: Curve::Hold,
                unknown: Unknown::new(),
            }],
            ..Expressions::default()
        });
    }
    bass.devices[0].aliases.push(Alias {
        plugin: "vst3:5653545A4D4F4E6F6D786D2D6D6F6E6F".to_owned(),
        state_compatible: true,
        unknown: Unknown::new(),
    });
    bass.devices[0].overrides.push(Override {
        param: "374".to_owned(),
        value: 440.0,
        normalized: Some(0.45),
        unknown: Unknown::new(),
    });
    bass.devices.push(device(
        ids,
        "audio_effect",
        "Example compressor",
        "builtin:example:compressor",
    ));
    let compressor_id = bass.devices[1].id.clone();
    bass.automation.push(AutomationLane {
        id: ids.next("aut"),
        target: Target {
            device: Some(compressor_id),
            param: Some("1".to_owned()),
            channel: None,
            to: None,
            parameter: None,
            unknown: Unknown::new(),
        },
        points: vec![Point {
            at: 0,
            value: 0.2,
            curve: Curve::Hold,
            unknown: Unknown::new(),
        }],
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    });
    bass.automation.push(AutomationLane {
        id: ids.next("aut"),
        target: Target {
            device: None,
            param: None,
            channel: Some("gain".to_owned()),
            to: None,
            parameter: None,
            unknown: Unknown::new(),
        },
        points: vec![
            Point {
                at: 0,
                value: 0.5,
                curve: Curve::Linear,
                unknown: Unknown::new(),
            },
            Point {
                at: 2 * bar,
                value: 0.7943,
                curve: Curve::Hold,
                unknown: Unknown::new(),
            },
        ],
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    });
    // The parts loop over half the pattern.
    for part in bass.parts.iter_mut().skip(4) {
        part.r#loop = Some(LoopRange {
            start: 0,
            end: bar / 2,
            unknown: Unknown::new(),
        });
        part.offset = 480;
        part.name = Some("Half bass".to_owned());
    }
    bass.parts[0].mute = true;
    bass.slots.push(Slot {
        scene: scene_ids[1].clone(),
        pattern: bass.patterns[0].id.clone(),
        r#loop: Some(LoopRange {
            start: 0,
            end: bar,
            unknown: Unknown::new(),
        }),
        unknown: Unknown::new(),
    });
    if let Some(Event::Note(n)) = bass.patterns[0].events.first_mut() {
        n.release = 0.6;
        if let Some(x) = &mut n.expressions {
            x.timbre.push(Point {
                at: 0,
                value: 0.3,
                curve: Curve::Hold,
                unknown: Unknown::new(),
            });
            x.volume.push(Point {
                at: 0,
                value: 0.9,
                curve: Curve::Linear,
                unknown: Unknown::new(),
            });
        }
    }
    let mut folder = Track::new(ids.next("trk"), "Rhythm");
    folder.color = Some("#777777".to_owned());
    folder.channel.gain = 0.9;
    folder.devices.push(device(
        ids,
        "audio_effect",
        "Example glue",
        "builtin:example:glue",
    ));
    let reverb_id = d
        .tracks
        .iter()
        .find(|t| t.name == "Reverb")
        .unwrap()
        .id
        .clone();
    folder.channel.sends.push(Send {
        to: reverb_id.clone(),
        gain: 0.1,
        pre_fader: true,
        enabled: false,
        unknown: Unknown::new(),
    });
    folder.automation.push(AutomationLane {
        id: ids.next("aut"),
        target: Target {
            device: None,
            param: None,
            channel: Some("send".to_owned()),
            to: Some(reverb_id),
            parameter: None,
            unknown: Unknown::new(),
        },
        points: vec![Point {
            at: 0,
            value: 0.1,
            curve: Curve::Hold,
            unknown: Unknown::new(),
        }],
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    });
    folder.automation.push(AutomationLane {
        id: ids.next("aut"),
        target: Target {
            device: None,
            param: None,
            channel: Some("mute".to_owned()),
            to: None,
            parameter: None,
            unknown: Unknown::new(),
        },
        points: vec![
            Point {
                at: 9 * bar,
                value: 1.0,
                curve: Curve::Hold,
                unknown: Unknown::new(),
            },
            Point {
                at: 10 * bar,
                value: 0.0,
                curve: Curve::Hold,
                unknown: Unknown::new(),
            },
        ],
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    });
    drums.channel.destination = Some(Id::from("master")); // the drums bypass the folder bus
    drums.channel.pan = -0.1;
    drums.channel.channels = 1;
    // The bass handed over as stems (spec 7.7): over the document's range, without its effects
    // and with them, each file exactly the range; and its verse pattern alone, at a tempo of
    // its own.
    let range = d.range.clone().expect("the range");
    let mut stems = Vec::new();
    for (name, file, devices, amplitude) in [
        (
            "Bass, without effects",
            "renders/Bass_dry.wav",
            render::without_effects(&bass),
            0.3,
        ),
        (
            "Bass, with effects",
            "renders/Bass_wet.wav",
            render::with_effects(&bass),
            0.2,
        ),
    ] {
        let (r, bytes) = track_render(
            d,
            &bass,
            file,
            name,
            devices,
            (range.start, range.end),
            amplitude,
        );
        bass.renders.push(r);
        stems.push((file, bytes));
    }
    let verse = bass.patterns[0].clone();
    let (mut r, bytes) = pattern_render(
        d,
        &bass,
        &verse,
        "renders/Verse-bass_120.wav",
        render::without_effects(&bass),
        120.0,
    );
    r.name = "Verse bass, without effects, at 120 bpm".to_owned();
    bass.patterns[0].renders.push(r);
    stems.push(("renders/Verse-bass_120.wav", bytes));
    folder.children = vec![drums, bass];

    // The loop track gets a warped second piece, an unwarped pad, a reversed piece, every channel
    // mode, a fade out with a handle, and a render.
    let loop_track = d.tracks.iter_mut().find(|t| t.name == "Loop").unwrap();
    let (pad, pad_frames) = sine(220.0, 1.5, 0.3, RATE);
    let pad_bytes = wav(1, RATE, &pad);
    let pad_hash = sha256_hex(&pad_bytes);
    let loop_hash = d
        .files
        .iter()
        .find(|f| f.path == "audio/Click-loop_95.wav")
        .unwrap()
        .sha256
        .clone();
    let loop_frames = (2 * 4) as f64 * 60.0 / 95.0 * RATE as f64;
    let loop_frames = loop_frames.round() as u64;
    let mut warped = audio_piece(
        "audio/Click-loop_95.wav",
        &loop_hash,
        loop_frames,
        Some(95.0),
    );
    warped.map = vec![
        MapPoint {
            frame: 0,
            tick: 0,
            unknown: Unknown::new(),
        },
        MapPoint {
            frame: loop_frames / 2,
            tick: 2 * 960 * 2 + 240,
            unknown: Unknown::new(),
        },
    ];
    warped.channel_mode = "mono".to_owned();
    warped.polarity = true;
    warped.fade_out = Some(Fade {
        length: 96,
        outside: 48,
        shape: "s_curve".to_owned(),
        unknown: Unknown::new(),
    });
    let mut unwarped = audio_piece("audio/Pad-220.wav", &pad_hash, pad_frames, None);
    unwarped.reverse = true;
    unwarped.channel_mode = "left".to_owned();
    unwarped.name = Some("Pad 220 Hz.wav".to_owned());
    let mut n1 = note(ids, 2 * bar, 2 * bar, 60, 0.9);
    if let Event::Note(x) = &mut n1 {
        x.audio = Some(warped);
    }
    let mut n2 = note(ids, 4 * bar, 960, 60, 1.0);
    if let Event::Note(x) = &mut n2 {
        x.audio = Some(unwarped);
    }
    let more = pattern(ids, "More audio", 6 * bar, vec![n1, n2]);
    let more_id = more.id.clone();
    loop_track.patterns.push(more);
    loop_track.parts.push(part(ids, &more_id, 2 * bar, 6 * bar));
    // A track without devices: its render holds its audio as placed, `devices` empty.
    let loop_track_clone = loop_track.clone();
    let (loop_render, loop_render_bytes) = track_render(
        d,
        &loop_track_clone,
        "renders/Loop_000.wav",
        "",
        Vec::new(),
        (0, 2 * bar),
        0.2,
    );
    let loop_track = d.tracks.iter_mut().find(|t| t.name == "Loop").unwrap();
    loop_track.renders.push(loop_render);
    d.tracks.insert(0, folder);
    p.add_file("audio/Pad-220.wav", pad_bytes);
    p.add_file("renders/Loop_000.wav", loop_render_bytes);
    for (file, bytes) in stems {
        p.add_file(file, bytes);
    }
    // A plugin's own files, in its device's folder (spec 7.8), nested as the plugin likes.
    let synth_id = p
        .document
        .all_tracks()
        .iter()
        .flat_map(|t| t.devices.iter())
        .find(|d| d.plugin == "clap:dk.mxm.mxm-mono-01")
        .map(|d| d.id.as_str().to_owned())
        .expect("the synth");
    p.add_file(
        &format!("plugins/files/{synth_id}/presets/Init.txt"),
        b"cutoff=218
"
        .to_vec(),
    );
    p
}

// --- the Level 0 fixtures ------------------------------------------------------------------------

/// Each fixture: its name, the project, and the expected table a reader is checked against.
fn fixtures(ids: &mut Ids) -> Vec<(&'static str, Project, Value)> {
    let mut out = Vec::new();

    // scale: C major up, one note a beat at 120 bpm, ppq 480 (not 960: the ppq is declared).
    let mut p = Project::new(generator());
    p.document.ppq = 480;
    let mut t = Track::new(ids.next("trk"), "Scale");
    let pitches = [60u8, 62, 64, 65, 67, 69, 71, 72];
    let events: Vec<Event> = pitches
        .iter()
        .enumerate()
        .map(|(i, pitch)| note(ids, i as i64 * 480, 240, *pitch, 0.8))
        .collect();
    let pat = pattern(ids, "Scale", 8 * 480, events);
    let pat_id = pat.id.clone();
    t.patterns.push(pat);
    t.parts.push(part(ids, &pat_id, 0, 8 * 480));
    p.document.tracks.push(t);
    let map = time::TempoMap::new(480, &p.document.tempo);
    let notes: Vec<Value> = pitches
        .iter()
        .enumerate()
        .map(|(i, pitch)| {
            let tick = i as f64 * 480.0;
            json!({"pitch": pitch, "tick": tick as i64, "seconds": map.seconds_at(tick), "sample_48000": map.samples_at(tick, 48_000)})
        })
        .collect();
    out.push((
        "scale",
        p,
        json!({"what": "each note starts at these seconds and samples; its length is half a beat", "rate": 48000, "notes": notes}),
    ));

    // ramp: the spec's vectors (5.2).
    let mut p = Project::new(generator());
    p.document.tempo = vec![
        TempoPoint {
            at: 0,
            bpm: 120.0,
            curve: Curve::Linear,
            unknown: Unknown::new(),
        },
        TempoPoint {
            at: 3840,
            bpm: 60.0,
            curve: Curve::Hold,
            unknown: Unknown::new(),
        },
    ];
    let mut t = Track::new(ids.next("trk"), "Ramp");
    let events: Vec<Event> = (1..=5)
        .map(|beat| note(ids, beat * 960, 240, 69, 0.8))
        .collect();
    let pat = pattern(ids, "Beats", 6 * 960, events);
    let pat_id = pat.id.clone();
    t.patterns.push(pat);
    t.parts.push(part(ids, &pat_id, 0, 6 * 960));
    p.document.tracks.push(t);
    let map = time::TempoMap::new(960, &p.document.tempo);
    let beats: Vec<Value> = (1..=5).map(|beat| {
        let tick = beat as f64 * 960.0;
        json!({"beat": beat, "tick": tick as i64, "seconds": map.seconds_at(tick), "sample_48000": map.samples_at(tick, 48_000)})
    }).collect();
    out.push(("ramp", p, json!({"what": "120 to 60 bpm linear over four beats, then 60: the notes fall on these samples (hold segments exact, linear within one sample)", "rate": 48000, "beats": beats})));

    // loop-audio: a click loop at 95 bpm in a project at 87 bpm.
    let mut p = Project::new(generator());
    p.document.tempo[0].bpm = 87.0;
    let (samples, frames) = click_loop(95.0, 2, RATE);
    let hash = p.add_file("audio/Click-loop_95.wav", wav(1, RATE, &samples));
    let mut t = Track::new(ids.next("trk"), "Loop");
    let piece = audio_piece("audio/Click-loop_95.wav", &hash, frames, Some(95.0));
    let mut n = note(ids, 0, 8 * 960, 60, 1.0);
    if let Event::Note(x) = &mut n {
        x.audio = Some(piece);
    }
    let pat = pattern(ids, "Loop", 8 * 960, vec![n]);
    let pat_id = pat.id.clone();
    t.patterns.push(pat);
    t.parts.push(part(ids, &pat_id, 0, 8 * 960));
    p.document.tracks.push(t);
    let placement = time::Placement::new(
        960,
        RATE,
        &[MapPoint {
            frame: 0,
            tick: 0,
            unknown: Unknown::new(),
        }],
        Some(95.0),
    );
    let map = time::TempoMap::new(960, &p.document.tempo);
    let clicks: Vec<Value> = (0..8).map(|k| {
        let source_frame = (k as f64 * 60.0 / 95.0 * RATE as f64).round();
        let tick = placement.tick_of_frame(source_frame).unwrap();
        json!({"click": k, "source_frame": source_frame as u64, "tick": tick, "seconds": map.seconds_at(tick)})
    }).collect();
    out.push(("loop-audio", p, json!({"what": "eight clicks recorded at 95 bpm play in an 87 bpm project at these seconds, stretched by 95/87, pitch kept", "stretch": 95.0 / 87.0, "clicks": clicks})));

    // mixer: four sine tracks at known gains and pans; expected RMS per channel.
    let mut p = Project::new(generator());
    let (tone, frames) = sine(440.0, 1.0, 0.5, RATE);
    let hash = p.add_file("audio/Tone-440.wav", wav(1, RATE, &tone));
    let rms_mono = 0.5 / 2f64.sqrt();
    let cases = [
        ("Centre", 1.0, 0.0, false, false),
        ("Half left", 0.5, -1.0, false, false),
        ("Muted", 1.0, 0.0, true, false),
        ("Right", 1.0, 1.0, false, false),
    ];
    let mut expected = Vec::new();
    for (name, gain, pan, mute, solo) in cases {
        let mut t = Track::new(ids.next("trk"), name);
        t.channel.gain = gain;
        t.channel.pan = pan;
        t.channel.mute = mute;
        t.channel.solo = solo;
        let piece = audio_piece("audio/Tone-440.wav", &hash, frames, None);
        let mut n = note(ids, 0, 960 * 2, 60, 1.0);
        if let Event::Note(x) = &mut n {
            x.audio = Some(piece);
        }
        let pat = pattern(ids, name, 960 * 2, vec![n]);
        let pat_id = pat.id.clone();
        t.patterns.push(pat);
        t.parts.push(part(ids, &pat_id, 0, 960 * 2));
        let id = t.id.clone();
        p.document.tracks.push(t);
        // Equal-power law for a mono source: left = cos(theta), right = sin(theta), theta = (pan + 1) * pi / 4.
        let theta = (pan + 1.0) * PI / 4.0;
        let (l, r) = if mute {
            (0.0, 0.0)
        } else {
            (gain * theta.cos() * rms_mono, gain * theta.sin() * rms_mono)
        };
        expected.push(json!({"track": id.as_str(), "name": name, "rms_left": l, "rms_right": r}));
    }
    out.push(("mixer", p, json!({"what": "each track soloed in turn: the RMS of its first second per output channel, within 0.5 dB", "tolerance_db": 0.5, "tracks": expected})));

    // folders: a folder at gain 0.5 with a child at gain 0.5: the child's tone arrives at 0.25.
    let mut p = Project::new(generator());
    let hash = p.add_file("audio/Tone-440.wav", wav(1, RATE, &tone));
    let mut child = Track::new(ids.next("trk"), "Child");
    child.channel.gain = 0.5;
    let piece = audio_piece("audio/Tone-440.wav", &hash, frames, None);
    let mut n = note(ids, 0, 960 * 2, 60, 1.0);
    if let Event::Note(x) = &mut n {
        x.audio = Some(piece);
    }
    let pat = pattern(ids, "Tone", 960 * 2, vec![n]);
    let pat_id = pat.id.clone();
    child.patterns.push(pat);
    child.parts.push(part(ids, &pat_id, 0, 960 * 2));
    let mut sibling = Track::new(ids.next("trk"), "Bypasses the folder");
    sibling.channel.destination = Some(Id::from("master"));
    sibling.channel.gain = 0.5;
    let piece = audio_piece("audio/Tone-440.wav", &hash, frames, None);
    let mut n = note(ids, 0, 960 * 2, 60, 1.0);
    if let Event::Note(x) = &mut n {
        x.audio = Some(piece);
    }
    let pat = pattern(ids, "Tone", 960 * 2, vec![n]);
    let pat_id = pat.id.clone();
    sibling.patterns.push(pat);
    sibling.parts.push(part(ids, &pat_id, 0, 960 * 2));
    let mut folder = Track::new(ids.next("trk"), "Folder");
    folder.channel.gain = 0.5;
    let (child_id, sibling_id) = (child.id.clone(), sibling.id.clone());
    folder.children = vec![child, sibling];
    p.document.tracks.push(folder);
    let centre = rms_mono * (PI / 4.0).cos();
    out.push(("folders", p, json!({"what": "each child soloed in turn: the child through the folder arrives at 0.25 of the tone, the sibling routed to the master at 0.5; RMS per channel within 0.5 dB", "tolerance_db": 0.5,
        "tracks": [{"track": child_id.as_str(), "rms_each_channel": 0.25 * centre}, {"track": sibling_id.as_str(), "rms_each_channel": 0.5 * centre}]})));

    // preserve: unknown keys at every level, an unknown event type, two extensions.
    let mut p = Project::new(generator());
    p.document.extensions_used = vec!["org.example.view".to_owned(), "dk.mxm.newdawn".to_owned()];
    p.document.unknown.insert(
        Key("future_top_level_key".to_owned()),
        json!({"keep": "me"}),
    );
    p.document
        .extensions
        .insert("org.example.view".to_owned(), json!({"zoom": 1.5}));
    let mut t = Track::new(ids.next("trk"), "Keep");
    t.unknown
        .insert(Key("future_track_key".to_owned()), json!([1, 2, 3]));
    t.extensions
        .insert("dk.mxm.newdawn".to_owned(), json!({"armed": true}));
    let mut events = vec![note(ids, 0, 960, 60, 0.8)];
    if let Event::Note(n) = &mut events[0] {
        n.unknown
            .insert(Key("future_note_key".to_owned()), json!("x"));
    }
    let mut unknown_event = serde_json::Map::new();
    unknown_event.insert("type".to_owned(), json!("glide"));
    unknown_event.insert("id".to_owned(), json!(ids.next("evt").as_str()));
    unknown_event.insert("at".to_owned(), json!(960));
    unknown_event.insert("to".to_owned(), json!(64));
    events.push(Event::Unknown(unknown_event));
    let pat = pattern(ids, "Keep", 1920, events);
    let pat_id = pat.id.clone();
    t.patterns.push(pat);
    t.parts.push(part(ids, &pat_id, 0, 1920));
    p.document.tracks.push(t);
    out.push(("preserve", p, json!({"what": "read, write, and compare by JCS after defaults: equal; and these survive", "must_survive": ["/future_top_level_key", "/extensions/org.example.view", "/tracks/0/future_track_key", "/tracks/0/extensions/dk.mxm.newdawn", "/tracks/0/patterns/0/events/0/future_note_key", "/tracks/0/patterns/0/events/1 (type glide)"]})));

    out
}

// --- the hostile fixtures ---------------------------------------------------------------------

/// A folder a reader must refuse, with the refusal it must give (`conformance.md` 4).
struct Hostile {
    name: &'static str,
    what: &'static str,
    /// What the first refusal must contain, in the words of `read`, `validate` or `check_files`.
    error: &'static str,
    /// Reference to bytes; the document among them.
    files: Vec<(String, Vec<u8>)>,
    /// A link the runner makes before the check, `(path, target)`, the target relative to the
    /// link's folder: git keeps no link on every system (Windows checks one out as a file).
    link: Option<(&'static str, &'static str)>,
}

/// A zip of stored entries, written by hand so a fixture can declare any length: each entry
/// `(name, bytes, declared)`, the declared length the bytes' unless given.
fn stored_zip(entries: &[(&str, &[u8], Option<u32>)]) -> Vec<u8> {
    fn le16(out: &mut Vec<u8>, v: u16) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    fn le32(out: &mut Vec<u8>, v: u32) {
        out.extend_from_slice(&v.to_le_bytes());
    }
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &b in bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, bytes, declared) in entries {
        let offset = out.len() as u32;
        let crc = crc32(bytes);
        let size = declared.unwrap_or(bytes.len() as u32);
        // The local header.
        out.extend_from_slice(b"PK\x03\x04");
        le16(&mut out, 10);
        le16(&mut out, 0);
        le16(&mut out, 0);
        le16(&mut out, 0);
        le16(&mut out, 0x21);
        le32(&mut out, crc);
        le32(&mut out, bytes.len() as u32);
        le32(&mut out, size);
        le16(&mut out, name.len() as u16);
        le16(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(bytes);
        // The central directory's record of it.
        central.extend_from_slice(b"PK\x01\x02");
        le16(&mut central, 20);
        le16(&mut central, 10);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0x21);
        le32(&mut central, crc);
        le32(&mut central, bytes.len() as u32);
        le32(&mut central, size);
        le16(&mut central, name.len() as u16);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le32(&mut central, 0);
        le32(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let central_offset = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(b"PK\x05\x06");
    le16(&mut out, 0);
    le16(&mut out, 0);
    le16(&mut out, entries.len() as u16);
    le16(&mut out, entries.len() as u16);
    le32(&mut out, central.len() as u32);
    le32(&mut out, central_offset);
    le16(&mut out, 0);
    out
}

fn hostile_document(ids: &mut Ids, mutate: impl FnOnce(&mut Document, &mut Ids)) -> Vec<u8> {
    let mut p = Project::new(generator());
    let mut t = Track::new(ids.next("trk"), "Track");
    let first_note = note(ids, 0, 960, 60, 0.8);
    let pat = pattern(ids, "P", 960, vec![first_note]);
    let pat_id = pat.id.clone();
    t.patterns.push(pat);
    t.parts.push(part(ids, &pat_id, 0, 960));
    p.document.tracks.push(t);
    mutate(&mut p.document, ids);
    write(&p.document).into_bytes()
}

fn hostile(ids: &mut Ids) -> Vec<Hostile> {
    let hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let piece = |file: &str| AudioPiece {
        file: file.to_owned(),
        sha256: hash.to_owned(),
        sample_rate: RATE,
        channels: 1,
        frames: 100,
        name: None,
        from: 0,
        to: 100,
        map: vec![MapPoint {
            frame: 0,
            tick: 0,
            unknown: Unknown::new(),
        }],
        bpm: Some(120.0),
        gain: 1.0,
        polarity: false,
        channel_mode: "as_recorded".to_owned(),
        fade_in: None,
        fade_out: None,
        reverse: false,
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    };
    let doc_name = "Hostile.tutti".to_owned();
    let mut out = Vec::new();

    out.push(Hostile {
        name: "dup-id",
        what: "two tracks with one id",
        error: "error /tracks/1/id",
        files: vec![(
            doc_name.clone(),
            hostile_document(ids, |d, _| {
                let mut twin = d.tracks[0].clone();
                twin.patterns.clear();
                twin.parts.clear();
                d.tracks.push(twin);
            }),
        )],
        link: None,
    });
    out.push(Hostile {
        name: "send-cycle",
        what: "a send to a track whose destination is the sender",
        error: "error /tracks: routing is not acyclic",
        files: vec![(
            doc_name.clone(),
            hostile_document(ids, |d, ids| {
                let mut b = Track::new(ids.next("trk"), "B");
                b.channel.destination = Some(d.tracks[0].id.clone());
                d.tracks[0].channel.sends.push(Send {
                    to: b.id.clone(),
                    gain: 1.0,
                    pre_fader: false,
                    enabled: true,
                    unknown: Unknown::new(),
                });
                d.tracks.push(b);
            }),
        )],
        link: None,
    });
    out.push(Hostile {
        name: "huge-tick",
        what: "a marker beyond 2^40 ticks",
        error: "error /markers/0/at",
        files: vec![(
            doc_name.clone(),
            hostile_document(ids, |d, ids| {
                d.markers.push(Marker {
                    id: ids.next("mrk"),
                    at: time::TICK_MAX + 1,
                    name: "Far".to_owned(),
                    kind: "marker".to_owned(),
                    length: None,
                    extensions: Extensions::new(),
                    unknown: Unknown::new(),
                });
            }),
        )],
        link: None,
    });
    out.push(Hostile {
        name: "map-unordered",
        what: "an audio map whose second point goes back in time",
        error: "error /tracks/0/patterns/0/events/0/audio/map",
        files: vec![(
            doc_name.clone(),
            hostile_document(ids, |d, _| {
                if let Event::Note(n) = &mut d.tracks[0].patterns[0].events[0] {
                    let mut a = piece("audio/x.wav");
                    a.map.push(MapPoint {
                        frame: 50,
                        tick: -10,
                        unknown: Unknown::new(),
                    });
                    n.audio = Some(a);
                }
            }),
        )],
        link: None,
    });
    out.push(Hostile {
        name: "path-escape",
        what: "an audio reference that leaves the folder",
        error: "error /tracks/0/patterns/0/events/0/audio/file",
        files: vec![(
            doc_name.clone(),
            hostile_document(ids, |d, _| {
                if let Event::Note(n) = &mut d.tracks[0].patterns[0].events[0] {
                    n.audio = Some(piece("audio/../../secret.wav"));
                }
            }),
        )],
        link: None,
    });
    let (tone, _) = sine(440.0, 0.1, 0.5, RATE);
    let tone_bytes = wav(1, RATE, &tone);
    let tone_hash = sha256_hex(&tone_bytes);
    out.push(Hostile {
        name: "size-lie",
        what: "the files index claims a length the file does not have",
        error: "error /files/0/bytes",
        files: vec![
            (
                doc_name.clone(),
                hostile_document(ids, |d, _| {
                    d.files.push(FileEntry {
                        path: "audio/Tone.wav".to_owned(),
                        sha256: tone_hash.clone(),
                        bytes: 4_000_000_000,
                        unknown: Unknown::new(),
                    });
                }),
            ),
            ("audio/Tone.wav".to_owned(), tone_bytes.clone()),
        ],
        link: None,
    });
    out.push(Hostile {
        name: "two-documents",
        what: "two .tutti files at the top of one folder",
        error: "2 documents",
        files: {
            let first = hostile_document(ids, |_, _| {});
            let second = hostile_document(ids, |_, _| {});
            vec![
                (doc_name.clone(), first),
                ("Other.tutti".to_owned(), second),
            ]
        },
        link: None,
    });
    out.push(Hostile {
        name: "higher-major",
        what: "a document of a major version this reader does not know",
        error: "tutti 2.0 is newer",
        files: vec![(
            doc_name.clone(),
            hostile_document(ids, |d, _| {
                d.version = "2.0".to_owned();
            }),
        )],
        link: None,
    });
    out.push(Hostile {
        name: "nan-token",
        what: "a number that is not a number",
        error: "not a tutti document",
        files: vec![(
            doc_name.clone(),
            br#"{"format": "tutti", "version": "1.0", "generator": {"name": "x"}, "ppq": 960,
  "tempo": [{"at": 0, "bpm": NaN}], "meter": [{"at": 0, "numerator": 4, "denominator": 4}]}
"#
            .to_vec(),
        )],
        link: None,
    });
    out.push(Hostile {
        name: "duplicate-key",
        what: "the same key twice in one object",
        error: "duplicate key at /tempo/0/bpm",
        files: vec![(doc_name, br#"{"format": "tutti", "version": "1.0", "generator": {"name": "x"}, "ppq": 960,
  "tempo": [{"at": 0, "bpm": 120, "bpm": 130}], "meter": [{"at": 0, "numerator": 4, "denominator": 4}]}
"#.to_vec())],
        link: None,
    });
    // A project as one file (spec 2.5): an entry that leaves the folder, and one that
    // declares more than a project file may be.
    let slip_document = hostile_document(ids, |_, _| {});
    out.push(Hostile {
        name: "zip-slip",
        what: "a project as one file with an entry whose name leaves the folder",
        error: "leaves the folder",
        files: vec![(
            "Slip.tutti.zip".to_owned(),
            stored_zip(&[
                ("Slip.tutti", &slip_document, None),
                ("../escape.wav", b"RIFF", None),
            ]),
        )],
        link: None,
    });
    let bomb_document = hostile_document(ids, |_, _| {});
    out.push(Hostile {
        name: "zip-bomb",
        what: "a project as one file with an entry that declares more than a project file may be",
        error: "more than a project file may be",
        files: vec![(
            "Bomb.tutti.zip".to_owned(),
            stored_zip(&[
                ("Bomb.tutti", &bomb_document, None),
                ("analysis/big.bin", b"ten bytes!", Some(0xF000_0000)),
            ]),
        )],
        link: None,
    });
    out.push(Hostile {
        name: "device-files-escape",
        what: "a device's own file indexed under a path that leaves its folder",
        error: "error /files/0/path",
        files: vec![(
            "Hostile.tutti".to_owned(),
            hostile_document(ids, |d, _| {
                d.files.push(FileEntry {
                    path: "plugins/files/dev_x/../escape.txt".to_owned(),
                    sha256: "0".repeat(64),
                    bytes: 1,
                    unknown: Unknown::new(),
                });
            }),
        )],
        link: None,
    });
    // A link in place of a folder the reader reads (spec 9): `audio` a link to a folder beside
    // it that holds the very file the index names, so only the link is wrong. A folder link,
    // not a file link: a junction makes it on Windows without a privilege, and a reader that
    // looks only at the last part of a path misses it.
    let tone_frames = (tone_bytes.len() as u64 - 44) / 2;
    out.push(Hostile {
        name: "symlink-audio",
        what: "the audio folder is a link (to link-target/ beside it, which holds the file the index names): refused, nothing read through it. Git keeps no link on every system, so the runner makes it before the check: a copy of this folder, its `audio` a link to `link-target`",
        error: "error /files/0/path: audio/Tone.wav: a link",
        files: vec![
            (
                "Hostile.tutti".to_owned(),
                hostile_document(ids, |d, _| {
                    if let Event::Note(n) = &mut d.tracks[0].patterns[0].events[0] {
                        let mut a = piece("audio/Tone.wav");
                        a.sha256 = tone_hash.clone();
                        a.frames = tone_frames;
                        a.to = tone_frames;
                        a.bpm = None;
                        n.audio = Some(a);
                    }
                    d.files.push(FileEntry {
                        path: "audio/Tone.wav".to_owned(),
                        sha256: tone_hash.clone(),
                        bytes: tone_bytes.len() as u64,
                        unknown: Unknown::new(),
                    });
                }),
            ),
            ("link-target/Tone.wav".to_owned(), tone_bytes.clone()),
        ],
        link: Some(("audio", "link-target")),
    });
    out
}

// --- the Level 1 fixtures ------------------------------------------------------------------------

/// A Level 1 fixture: its name, the project, and the expected table a reader is checked against.
struct Fixture {
    name: &'static str,
    project: Project,
    expected: Value,
}

/// Ticks in a second at the fixtures' 120 bpm and `ppq` 960.
const SECOND: i64 = 1920;

/// The fictitious plugins: no one has them, so what they are given is a table, not a sound.
const PROBE: &str = "clap:org.example.parameter-probe";
const MISSING: &str = "clap:org.example.missing-synth";
const MISSING_EFFECT: &str = "clap:org.example.missing-halver";

fn level_1(ids: &mut Ids) -> Vec<Fixture> {
    vec![
        automation(ids),
        sends(ids),
        warps(ids),
        expressions(ids),
        missing_plugin(ids),
    ]
}

/// A sine on two channels, each at its own amplitude, interleaved.
fn stereo_sine(hz: f64, seconds: f64, left: f64, right: f64, rate: u32) -> (Vec<f32>, u64) {
    let frames = (seconds * rate as f64).round() as u64;
    let mut out = Vec::with_capacity(frames as usize * 2);
    for i in 0..frames {
        let s = (2.0 * PI * hz * i as f64 / rate as f64).sin();
        out.push((left * s) as f32);
        out.push((right * s) as f32);
    }
    (out, frames)
}

/// The RMS of a sine of `amplitude` under a gain running linearly from `g0` to `g1` across a
/// window of many cycles: the sine's mean square times the mean of the squared line,
/// `amplitude / √2 · √((g0² + g0·g1 + g1²) / 3)`; a constant gain when the two are equal.
fn rms_under_ramp(amplitude: f64, g0: f64, g1: f64) -> f64 {
    amplitude / 2f64.sqrt() * ((g0 * g0 + g0 * g1 + g1 * g1) / 3.0).sqrt()
}

/// A value that is zero but for rounding (`cos(π/2)`), written as zero.
fn clean(x: f64) -> f64 {
    if x.abs() < 1e-12 { 0.0 } else { x }
}

/// A tick that is whole but for rounding, written whole.
fn snap(tick: f64) -> f64 {
    if (tick - tick.round()).abs() < 1e-6 {
        tick.round()
    } else {
        tick
    }
}

/// A curve's value at a tick (spec 6.8, and 6.7's expressions): `hold` keeps a point's value
/// until the next point, `linear` runs to it. The fixtures ask nothing before a curve's first
/// point or after its last, which the spec leaves unsaid.
fn curve_value(points: &[Point], tick: f64) -> f64 {
    let i = points
        .iter()
        .rposition(|p| p.at as f64 <= tick)
        .expect("a tick at or after the first point");
    let p = &points[i];
    match (p.curve.effective(), points.get(i + 1)) {
        (Curve::Linear, Some(next)) => {
            p.value + (next.value - p.value) * (tick - p.at as f64) / (next.at - p.at) as f64
        }
        _ => p.value,
    }
}

fn point(at: i64, value: f64, curve: Curve) -> Point {
    Point {
        at,
        value,
        curve,
        unknown: Unknown::new(),
    }
}

fn target() -> Target {
    Target {
        device: None,
        param: None,
        channel: None,
        to: None,
        parameter: None,
        unknown: Unknown::new(),
    }
}

fn channel_target(channel: &str, to: Option<Id>) -> Target {
    Target {
        channel: Some(channel.to_owned()),
        to,
        ..target()
    }
}

fn lane(ids: &mut Ids, target: Target, points: Vec<Point>) -> AutomationLane {
    AutomationLane {
        id: ids.next("aut"),
        target,
        points,
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    }
}

fn send_to(to: &Id, gain: f64, pre_fader: bool, enabled: bool) -> Send {
    Send {
        to: to.clone(),
        gain,
        pre_fader,
        enabled,
        unknown: Unknown::new(),
    }
}

/// `b`'s keys after `a`'s, in one object.
fn join(mut a: Value, b: Value) -> Value {
    if let (Value::Object(a), Value::Object(b)) = (&mut a, b) {
        a.extend(b);
    }
    a
}

/// A time on the tempo map, as the tables give it: the tick (an integer when it is whole), its
/// seconds, its sample at 48 kHz.
fn when(map: &time::TempoMap, tick: f64) -> Value {
    let at = if tick.fract() == 0.0 {
        json!(tick as i64)
    } else {
        json!(tick)
    };
    json!({"tick": at, "seconds": map.seconds_at(tick), "sample_48000": map.samples_at(tick, 48_000)})
}

/// A lane's value at each of `ticks`, one row each.
fn lane_rows(map: &time::TempoMap, lane: &AutomationLane, ticks: &[i64], out: &mut Vec<Value>) {
    for &tick in ticks {
        out.push(join(
            join(
                json!({"lane": lane.id.as_str(), "target": serde_json::to_value(&lane.target).unwrap()}),
                when(map, tick as f64),
            ),
            json!({"value": curve_value(&lane.points, tick as f64)}),
        ));
    }
}

/// What the master carries over a window of seconds while one track sounds: the RMS of each
/// output channel.
fn heard(track: &Track, from: f64, to: f64, left: f64, right: f64) -> Value {
    json!({"track": track.id.as_str(), "name": track.name, "from_seconds": from, "to_seconds": to,
        "rms_left": clean(left), "rms_right": clean(right)})
}

/// A track playing `piece` `times` times back to back, a second each (the tones are whole
/// cycles, so they join without a seam), from `at` on the track.
fn tone_track(ids: &mut Ids, name: &str, at: i64, times: i64, piece: &AudioPiece) -> Track {
    let mut t = Track::new(ids.next("trk"), name);
    let mut events = Vec::new();
    for k in 0..times {
        let mut n = note(ids, k * SECOND, SECOND, 60, 1.0);
        if let Event::Note(x) = &mut n {
            x.audio = Some(piece.clone());
        }
        events.push(n);
    }
    let pat = pattern(ids, name, times * SECOND, events);
    let pat_id = pat.id.clone();
    t.patterns.push(pat);
    t.parts.push(part(ids, &pat_id, at, times * SECOND));
    t
}

fn param(id: &str, name: &str, value: f64, min: f64, max: f64, unit: &str) -> Param {
    Param {
        id: id.to_owned(),
        name: Some(name.to_owned()),
        value,
        normalized: None,
        min: Some(min),
        max: Some(max),
        unit: Some(unit.to_owned()),
        text: None,
        unknown: Unknown::new(),
    }
}

fn example_dependency(id: &str, name: &str, page: &str) -> Dependency {
    Dependency {
        vendor: Some("Example".to_owned()),
        version: Some("1.0.0".to_owned()),
        url: Some(format!("https://example.org/{page}")),
        formats: vec!["clap".to_owned()],
        ..Dependency::plugin(id, name)
    }
}

/// automation: channel targets heard (gain held then linear, pan held, mute linear and muted
/// from 0.5, a send's gain), each track alone in its own seconds so no solo enters; and every
/// lane's value at stated ticks, a device target, a promoted name and a parameter event through
/// one included.
fn automation(ids: &mut Ids) -> Fixture {
    let mut p = Project::new(generator());
    let (mono, mono_frames) = sine(440.0, 1.0, 0.5, RATE);
    let mono_hash = p.add_file("audio/Tone-440.wav", wav(1, RATE, &mono));
    let (stereo, stereo_frames) = stereo_sine(440.0, 1.0, 0.5, 0.25, RATE);
    let stereo_hash = p.add_file("audio/Tone-440-stereo.wav", wav(2, RATE, &stereo));
    let mono_piece = audio_piece("audio/Tone-440.wav", &mono_hash, mono_frames, None);
    let mut stereo_piece = audio_piece(
        "audio/Tone-440-stereo.wav",
        &stereo_hash,
        stereo_frames,
        None,
    );
    stereo_piece.channels = 2;
    let map = time::TempoMap::new(960, &p.document.tempo);
    let rms = 0.5 / 2f64.sqrt();
    // A mono source at the centre, each side (equal power, spec 6.2).
    let centre = (PI / 4.0).cos();
    let mut heard_rows = Vec::new();
    let mut values = Vec::new();
    let windows = |base: f64| {
        [
            (base + 0.1, base + 0.9),
            (base + 1.1, base + 1.9),
            (base + 2.1, base + 2.9),
        ]
    };

    // Gain, from second 0: held at 1, then a line from 0.5 down to 0.25 over two seconds.
    let mut gain = tone_track(ids, "Gain", 0, 3, &mono_piece);
    let gain_lane = lane(
        ids,
        channel_target("gain", None),
        vec![
            point(0, 1.0, Curve::Hold),
            point(SECOND, 0.5, Curve::Linear),
            point(3 * SECOND, 0.25, Curve::Hold),
        ],
    );
    for (from, to) in windows(0.0) {
        let g0 = curve_value(&gain_lane.points, map.tick_at(from));
        let g1 = curve_value(&gain_lane.points, map.tick_at(to));
        let each = centre * rms_under_ramp(0.5, g0, g1);
        heard_rows.push(heard(&gain, from, to, each, each));
    }
    lane_rows(
        &map,
        &gain_lane,
        &[0, SECOND - 1, SECOND, 2 * SECOND, 3 * SECOND],
        &mut values,
    );
    gain.automation.push(gain_lane);

    // Pan, from second 4: hard left, hard right, centre, each held.
    let b = 4 * SECOND;
    let mut pan = tone_track(ids, "Pan", b, 3, &mono_piece);
    let pan_lane = lane(
        ids,
        channel_target("pan", None),
        vec![
            point(b, -1.0, Curve::Hold),
            point(b + SECOND, 1.0, Curve::Hold),
            point(b + 2 * SECOND, 0.0, Curve::Hold),
            point(b + 3 * SECOND, 0.0, Curve::Hold),
        ],
    );
    for (from, to) in windows(4.0) {
        let theta = (curve_value(&pan_lane.points, map.tick_at(from)) + 1.0) * PI / 4.0;
        heard_rows.push(heard(&pan, from, to, rms * theta.cos(), rms * theta.sin()));
    }
    lane_rows(
        &map,
        &pan_lane,
        &[b, b + SECOND - 1, b + SECOND, b + 2 * SECOND],
        &mut values,
    );
    pan.automation.push(pan_lane);

    // Mute, from second 8: a line from 0 to 1 over two seconds, muted from 0.5 (second 9) on,
    // then held at 1, and 0 again from second 10.5.
    let b = 8 * SECOND;
    let mut mute = tone_track(ids, "Mute", b, 3, &mono_piece);
    let mute_lane = lane(
        ids,
        channel_target("mute", None),
        vec![
            point(b, 0.0, Curve::Linear),
            point(b + 2 * SECOND, 1.0, Curve::Hold),
            point(b + 2 * SECOND + SECOND / 2, 0.0, Curve::Hold),
            point(b + 3 * SECOND, 0.0, Curve::Hold),
        ],
    );
    for (from, to) in [(8.1, 8.9), (9.1, 10.4), (10.6, 10.9)] {
        let muted = curve_value(&mute_lane.points, map.tick_at(from)) >= 0.5;
        assert_eq!(
            muted,
            curve_value(&mute_lane.points, map.tick_at(to)) >= 0.5,
            "a window the mute does not change in"
        );
        let each = if muted { 0.0 } else { centre * rms };
        heard_rows.push(heard(&mute, from, to, each, each));
    }
    let first = values.len();
    lane_rows(
        &map,
        &mute_lane,
        &[
            b,
            b + SECOND - 1,
            b + SECOND,
            b + 2 * SECOND,
            b + 2 * SECOND + SECOND / 2 - 1,
            b + 2 * SECOND + SECOND / 2,
        ],
        &mut values,
    );
    for row in &mut values[first..] {
        let muted = row["value"].as_f64().unwrap() >= 0.5;
        row["muted"] = json!(muted);
    }
    mute.automation.push(mute_lane);

    // A send's gain, from second 12: a stereo tone (so no pan law enters), the fader down and a
    // pre-fader send, so only the return is heard; the send held at 1, then a line from 0.5 to
    // 0 over a second, then held at 0.
    let b = 12 * SECOND;
    let ret = Track::new(ids.next("trk"), "Return");
    let mut send = tone_track(ids, "Send", b, 3, &stereo_piece);
    send.channel.gain = 0.0;
    send.channel.sends.push(send_to(&ret.id, 1.0, true, true));
    let send_lane = lane(
        ids,
        channel_target("send", Some(ret.id.clone())),
        vec![
            point(b, 1.0, Curve::Hold),
            point(b + SECOND, 0.5, Curve::Linear),
            point(b + 2 * SECOND, 0.0, Curve::Hold),
            point(b + 3 * SECOND, 0.0, Curve::Hold),
        ],
    );
    for (from, to) in windows(12.0) {
        let g0 = curve_value(&send_lane.points, map.tick_at(from));
        let g1 = curve_value(&send_lane.points, map.tick_at(to));
        heard_rows.push(heard(
            &send,
            from,
            to,
            rms_under_ramp(0.5, g0, g1),
            rms_under_ramp(0.25, g0, g1),
        ));
    }
    lane_rows(
        &map,
        &send_lane,
        &[
            b,
            b + SECOND - 1,
            b + SECOND,
            b + SECOND + SECOND / 2,
            b + 2 * SECOND,
        ],
        &mut values,
    );
    send.automation.push(send_lane);

    // A device of a fictitious plugin, on a track that sounds nothing: a device target (a line),
    // a promoted name (held steps) and a parameter event through another promoted name.
    let mut device_track = Track::new(ids.next("trk"), "Device");
    let mut probe = device(ids, "audio_effect", "Parameter probe", PROBE);
    probe.params = vec![
        param("1", "Amount", 0.0, 0.0, 1.0, "linear"),
        param("2", "Frequency", 220.0, 20.0, 20000.0, "hertz"),
        param("3", "Steps", 0.0, 0.0, 16.0, "other"),
    ];
    let probe_id = probe.id.clone();
    device_track.devices.push(probe);
    for (name, param) in [("frequency", "2"), ("steps", "3")] {
        device_track.parameters.push(Promoted {
            name: name.to_owned(),
            device: probe_id.clone(),
            param: param.to_owned(),
            unknown: Unknown::new(),
        });
    }
    let device_lane = lane(
        ids,
        Target {
            device: Some(probe_id.clone()),
            param: Some("1".to_owned()),
            ..target()
        },
        vec![
            point(0, 0.0, Curve::Linear),
            point(2 * SECOND, 1.0, Curve::Hold),
            point(3 * SECOND, 1.0, Curve::Hold),
        ],
    );
    lane_rows(
        &map,
        &device_lane,
        &[
            0,
            SECOND / 2,
            SECOND,
            3 * SECOND / 2,
            2 * SECOND,
            5 * SECOND / 2,
        ],
        &mut values,
    );
    let promoted_lane = lane(
        ids,
        Target {
            parameter: Some("frequency".to_owned()),
            ..target()
        },
        vec![
            point(0, 220.0, Curve::Hold),
            point(SECOND, 440.0, Curve::Hold),
            point(2 * SECOND, 880.0, Curve::Hold),
            point(3 * SECOND, 880.0, Curve::Hold),
        ],
    );
    let first = values.len();
    lane_rows(
        &map,
        &promoted_lane,
        &[
            0,
            SECOND - 1,
            SECOND,
            2 * SECOND - 1,
            2 * SECOND,
            5 * SECOND / 2,
        ],
        &mut values,
    );
    for row in &mut values[first..] {
        row["resolves_to"] = json!({"device": probe_id.as_str(), "param": "2"});
    }
    let step_at = 3 * SECOND / 2;
    let step = Event::Param(ParamEvent {
        id: ids.next("evt"),
        at: step_at,
        param: "steps".to_owned(),
        value: 4.0,
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    });
    let step_id = step.id().to_owned();
    let pat = pattern(ids, "Steps", 3 * SECOND, vec![step]);
    let pat_id = pat.id.clone();
    device_track.patterns.push(pat);
    device_track.parts.push(part(ids, &pat_id, 0, 3 * SECOND));
    for (tick, value, from) in [
        (step_at - 1, 0.0, "the snapshot"),
        (step_at, 4.0, "the event"),
    ] {
        values.push(join(
            join(
                json!({"event": step_id, "parameter": "steps", "resolves_to": {"device": probe_id.as_str(), "param": "3"}}),
                when(&map, tick as f64),
            ),
            json!({"value": value, "from": from}),
        ));
    }
    device_track.automation.push(device_lane);
    device_track.automation.push(promoted_lane);

    p.document.dependencies.push(example_dependency(
        PROBE,
        "Parameter probe",
        "parameter-probe",
    ));
    p.document.tracks = vec![gain, pan, mute, send, ret, device_track];
    Fixture {
        name: "automation",
        project: p,
        expected: json!({
            "what": "Channel targets, heard: each track sounds alone in its own seconds (no solo), and the master's RMS per output channel over each window is within 0.5 dB of these: gain held at 1, then a line from 0.5 to 0.25; pan held hard left, hard right, centre (a mono tone, equal power); mute a line from 0 to 1, muted from 0.5 on; a send's gain held at 1, then a line from 0.5 to 0, the source's fader down and its send pre-fader so only the return is heard (a stereo tone, left 0.5 and right 0.25). And every lane's value at these ticks, which is what a host gives its target, the plain value: a device target, and a promoted name resolving to a parameter of the device; a parameter event through a promoted name sets its parameter from the event's tick on, the snapshot's value before. The device's plugin is fictitious: its values are heard only through a plugin that shows them (the `plugin` case's reference CLAP, once there is one); until then a host checks its own evaluation against them.",
            "rate": 48000,
            "tolerance_db": 0.5,
            "heard": heard_rows,
            "values": values,
        }),
    }
}

/// sends: a send to a return pre-fader with the fader down, post-fader through two faders, and
/// one switched off; each source alone in its own seconds.
fn sends(ids: &mut Ids) -> Fixture {
    let mut p = Project::new(generator());
    let (stereo, frames) = stereo_sine(440.0, 1.0, 0.5, 0.25, RATE);
    let hash = p.add_file("audio/Tone-440-stereo.wav", wav(2, RATE, &stereo));
    let mut piece = audio_piece("audio/Tone-440-stereo.wav", &hash, frames, None);
    piece.channels = 2;
    let (left, right) = (0.5 / 2f64.sqrt(), 0.25 / 2f64.sqrt());
    let return_a = Track::new(ids.next("trk"), "Return A");
    let mut return_b = Track::new(ids.next("trk"), "Return B");
    return_b.channel.gain = 0.5;
    let mut sink = Track::new(ids.next("trk"), "Sink");
    sink.channel.mute = true;
    // From second 0: the fader down, a pre-fader send at 0.5 to Return A, still heard.
    let mut pre = tone_track(ids, "Pre-fader", 0, 1, &piece);
    pre.channel.gain = 0.0;
    pre.channel
        .sends
        .push(send_to(&return_a.id, 0.5, true, true));
    // From second 2: the fader at 0.5, a post-fader send at 0.5 to Return B (fader 0.5); the
    // source's own output goes to a muted track, so only the return is heard.
    let mut post = tone_track(ids, "Post-fader", 2 * SECOND, 1, &piece);
    post.channel.gain = 0.5;
    post.channel.destination = Some(sink.id.clone());
    post.channel
        .sends
        .push(send_to(&return_b.id, 0.5, false, true));
    // From second 4: the fader down, a pre-fader send to Return A switched off.
    let mut off = tone_track(ids, "Disabled", 4 * SECOND, 1, &piece);
    off.channel.gain = 0.0;
    off.channel
        .sends
        .push(send_to(&return_a.id, 1.0, true, false));
    let heard_rows = vec![
        join(
            heard(&pre, 0.1, 0.9, 0.5 * left, 0.5 * right),
            json!({"path": "the pre-fader send at 0.5 into Return A at 1"}),
        ),
        join(
            heard(&post, 2.1, 2.9, 0.125 * left, 0.125 * right),
            json!({"path": "the fader at 0.5, the post-fader send at 0.5, Return B at 0.5"}),
        ),
        join(
            heard(&off, 4.1, 4.9, 0.0, 0.0),
            json!({"path": "a disabled send: nothing"}),
        ),
    ];
    p.document.tracks = vec![pre, post, off, return_a, return_b, sink];
    Fixture {
        name: "sends",
        project: p,
        expected: json!({
            "what": "Each source sounds alone in its own seconds (no solo), and the master's RMS per output channel over each window is within 0.5 dB of these: a pre-fader send is heard with its source's fader down; a post-fader send follows the source's fader, and its return's fader after it; a disabled send sends nothing. The tone is stereo, left 0.5 and right 0.25, at the centre, so no pan law enters.",
            "tolerance_db": 0.5,
            "heard": heard_rows,
        }),
    }
}

/// warps: one click loop three ways under the spec's tempo ramp: warped by a three-point map,
/// anchored by one point at its bpm, and unwarped.
fn warps(ids: &mut Ids) -> Fixture {
    let mut p = Project::new(generator());
    p.document.tempo = vec![
        TempoPoint {
            at: 0,
            bpm: 120.0,
            curve: Curve::Linear,
            unknown: Unknown::new(),
        },
        TempoPoint {
            at: 3840,
            bpm: 60.0,
            curve: Curve::Hold,
            unknown: Unknown::new(),
        },
    ];
    let (samples, frames) = click_loop(100.0, 2, RATE);
    let hash = p.add_file("audio/Click-loop_100.wav", wav(1, RATE, &samples));
    // A click every beat at 100 bpm: 0.6 s, 26 460 frames.
    let beat = 26_460u64;
    assert_eq!(frames, 8 * beat);
    let map = time::TempoMap::new(960, &p.document.tempo);
    let map_point = |frame: u64, tick: i64| MapPoint {
        frame,
        tick,
        unknown: Unknown::new(),
    };
    let mut warped = audio_piece("audio/Click-loop_100.wav", &hash, frames, Some(100.0));
    // Two clicks squeezed into a beat and a half, the next two stretched over three beats, the
    // rest at the loop's own 100 bpm.
    warped.map = vec![
        map_point(0, 0),
        map_point(2 * beat, 1440),
        map_point(4 * beat, 4320),
    ];
    let anchored = audio_piece("audio/Click-loop_100.wav", &hash, frames, Some(100.0));
    let unwarped = audio_piece("audio/Click-loop_100.wav", &hash, frames, None);
    // The window every piece plays in: the warped loop's last frame falls at tick 8160.
    let window = 8160;
    let mut tracks = Vec::new();
    for (name, piece) in [
        ("Warped", warped),
        ("Anchored", anchored),
        ("Unwarped", unwarped),
    ] {
        let placement = time::Placement::new(960, RATE, &piece.map, piece.bpm);
        let clicks: Vec<Value> = (0..8)
            .map(|k| {
                let frame = k * beat;
                let head = json!({"click": k, "source_frame": frame});
                match placement.tick_of_frame(frame as f64) {
                    Some(tick) => join(head, when(&map, snap(tick))),
                    None => {
                        // Unwarped: its start follows the grid, its content plays at 1×.
                        let seconds = map.seconds_at(0.0) + frame as f64 / RATE as f64;
                        join(
                            head,
                            json!({"seconds": seconds, "sample_48000": time::round_half_up(seconds * 48_000.0)}),
                        )
                    }
                }
            })
            .collect();
        let mut t = Track::new(ids.next("trk"), name);
        tracks.push(json!({"track": t.id.as_str(), "name": name,
            "map": serde_json::to_value(&piece.map).unwrap(), "bpm": piece.bpm, "clicks": clicks}));
        let mut n = note(ids, 0, window, 60, 1.0);
        if let Event::Note(x) = &mut n {
            x.audio = Some(piece);
        }
        let pat = pattern(ids, name, window, vec![n]);
        let pat_id = pat.id.clone();
        t.patterns.push(pat);
        t.parts.push(part(ids, &pat_id, 0, window));
        p.document.tracks.push(t);
    }
    Fixture {
        name: "warps",
        project: p,
        expected: json!({
            "what": "A click loop recorded at 100 bpm (a 1 kHz click every 26460 frames at 44100 Hz), three ways, each track soloed in turn, under the tempo ramp of 5.2 (120 to 60 bpm over four beats, then 60): warped by a three-point map (two clicks squeezed into a beat and a half, the next two stretched over three beats, the rest at its bpm); anchored by one point at its bpm; unwarped (bpm null). Each click starts at these seconds and samples at 48000 Hz. The warped and anchored loops follow the map and the ramp, stretched with the pitch kept (the clicks stay 1 kHz); the unwarped one plays at its own speed whatever the tempo.",
            "rate": 48000,
            "click_hz": 1000,
            "tracks": tracks,
        }),
    }
}

/// expressions: two notes at once, one bent by its own pitch curve, the other not.
fn expressions(ids: &mut Ids) -> Fixture {
    let mut p = Project::new(generator());
    let map = time::TempoMap::new(960, &p.document.tempo);
    let length = 4 * 960;
    // Held at 0 for a beat, up an octave over the next two, held there.
    let curve = vec![
        point(0, 0.0, Curve::Hold),
        point(960, 0.0, Curve::Linear),
        point(2880, 12.0, Curve::Hold),
        point(length, 12.0, Curve::Hold),
    ];
    let mut bent = note(ids, 0, length, 69, 0.8);
    if let Event::Note(n) = &mut bent {
        n.expressions = Some(Expressions {
            pitch: curve.clone(),
            ..Expressions::default()
        });
    }
    let still = note(ids, 0, length, 57, 0.8);
    let ticks = [480i64, 1440, 1920, 2400, 3360];
    let rows = |points: Option<&[Point]>| -> Vec<Value> {
        ticks
            .iter()
            .map(|&tick| {
                let semitones = points.map_or(0.0, |p| curve_value(p, tick as f64));
                join(
                    when(&map, tick as f64),
                    json!({"semitones": semitones, "ratio": 2f64.powf(semitones / 12.0)}),
                )
            })
            .collect()
    };
    let notes = vec![
        json!({"note": bent.id(), "pitch": 69, "times": rows(Some(curve.as_slice()))}),
        json!({"note": still.id(), "pitch": 57, "times": rows(None)}),
    ];
    let mut t = Track::new(ids.next("trk"), "Bend");
    let pat = pattern(ids, "Bend", length, vec![bent, still]);
    let pat_id = pat.id.clone();
    t.patterns.push(pat);
    t.parts.push(part(ids, &pat_id, 0, length));
    p.document.tracks.push(t);
    Fixture {
        name: "expressions",
        project: p,
        expected: json!({
            "what": "Two notes at once on a track without a device, sounded by the reader's own instrument as Level 0's notes are: A4 (69) bends by its own pitch curve (6.7: held at 0 for a beat, up an octave over the next two, held there), A3 (57) carries none and stays: a note's curve, not the channel's bend. The pitch heard, in semitones above each note's own pitch and as a frequency ratio, at these times, within 10 cents.",
            "rate": 48000,
            "tolerance_cents": 10,
            "notes": notes,
        }),
    }
}

/// missing-plugin: a device of a fictitious plugin with a state, a snapshot, a dependency entry
/// and a current render.
fn missing_plugin(ids: &mut Ids) -> Fixture {
    let mut p = Project::new(generator());
    let state = p.add_state(
        b"tutti fixture: the state of a plugin no one has\n".to_vec(),
        "clap",
    );
    let dependency = example_dependency(MISSING, "Missing synth", "missing-synth");
    p.document.dependencies.push(dependency.clone());
    let mut t = Track::new(ids.next("trk"), "Synth");
    t.channel.gain = 0.5;
    let mut synth = device(ids, "instrument", "Missing synth", MISSING);
    synth.state = Some(state.clone());
    let mut level = param("0", "Level", 0.5, 0.0, 1.0, "linear");
    level.normalized = Some(0.5);
    level.text = Some("50 %".to_owned());
    synth.params.push(level);
    let synth_json = serde_json::to_value(&synth).unwrap();
    let synth_id = synth.id.clone();
    t.devices.push(synth);
    let events: Vec<Event> = (0..4).map(|i| note(ids, i * 480, 420, 69, 0.8)).collect();
    let pat = pattern(ids, "Notes", SECOND, events);
    let pat_id = pat.id.clone();
    t.patterns.push(pat);
    t.parts.push(part(ids, &pat_id, 0, SECOND));
    // The chain's output over the part, before the fader: a stereo sine, both sides 0.5, exactly
    // the second the range is.
    let (render, frames) = stereo_sine(440.0, 1.0, 0.5, 0.5, RATE);
    let render_bytes = wav(2, RATE, &render);
    let facts = AudioFacts {
        sample_rate: RATE,
        channels: 2,
        frames,
    };
    let devices = render::with_effects(&t);
    let mut r = Render::new(
        "renders/Synth_000.wav",
        &sha256_hex(&render_bytes),
        facts,
        devices,
        0,
        SECOND,
    );
    r.fingerprint = render_fingerprint(&p.document, &t, &r.devices, 0, SECOND);
    t.renders.push(r);
    p.add_file("renders/Synth_000.wav", render_bytes);
    let each = 0.5 * 0.5 / 2f64.sqrt();
    let heard_rows = vec![heard(&t, 0.1, 0.9, each, each)];
    p.document.tracks.push(t);
    Fixture {
        name: "missing-plugin",
        project: p,
        expected: json!({
            "what": "A device of a plugin no one has (fictitious), with a state, a snapshot, a dependency entry and a current render (7.6, 7.7). A reader opens the project; names the missing plugin from its dependencies entry; plays the render in the device's place, through the track's fader at 0.5, so the master's RMS per output channel over the window is within 0.5 dB of these; keeps the device, its state file and its snapshot as they are; and writes an edit it makes to that device into overrides only, after which the render is stale (the fingerprint covers the device).",
            "plugin": MISSING,
            "reports": {"name": dependency.name, "vendor": dependency.vendor, "version": dependency.version, "url": dependency.url},
            "tolerance_db": 0.5,
            "heard": heard_rows,
            "keeps": {"device": synth_json, "state_file": state.path()},
            "edit": {"device": synth_id.as_str(), "param": "0", "value": 0.25},
            "after_edit": {"overrides": [{"param": "0", "value": 0.25}], "render_stale": true},
        }),
    }
}

/// render-choice: an instrument and an effect no one has, a render without effects, one with
/// effects and a pattern render (spec 7.6, 7.7).
fn render_choice(ids: &mut Ids) -> Fixture {
    let mut p = Project::new(generator());
    let synth_dep = example_dependency(MISSING, "Missing synth", "missing-synth");
    let halver_dep = example_dependency(MISSING_EFFECT, "Missing halver", "missing-halver");
    p.document.dependencies.push(synth_dep);
    p.document.dependencies.push(halver_dep);
    let mut t = Track::new(ids.next("trk"), "Synth");
    t.channel.gain = 0.5;
    t.devices
        .push(device(ids, "instrument", "Missing synth", MISSING));
    let mut halver = device(ids, "audio_effect", "Missing halver", MISSING_EFFECT);
    halver
        .params
        .push(param("0", "Amount", 0.5, 0.0, 1.0, "linear"));
    let halver_id = halver.id.clone();
    t.devices.push(halver);
    let events: Vec<Event> = (0..4).map(|i| note(ids, i * 480, 420, 69, 0.8)).collect();
    let pat = pattern(ids, "Notes", SECOND, events);
    let pat_id = pat.id.clone();
    t.patterns.push(pat);
    t.parts.push(part(ids, &pat_id, 0, SECOND));
    // Without effects, the synth alone: a stereo sine at 0.5; with effects, halved: at 0.25.
    let mut files = Vec::new();
    for (name, file, devices, amplitude) in [
        (
            "Synth, without effects",
            "renders/Synth_dry.wav",
            render::without_effects(&t),
            0.5,
        ),
        (
            "Synth, with effects",
            "renders/Synth_wet.wav",
            render::with_effects(&t),
            0.25,
        ),
    ] {
        let (samples, frames) = stereo_sine(440.0, 1.0, amplitude, amplitude, RATE);
        let bytes = wav(2, RATE, &samples);
        let facts = AudioFacts {
            sample_rate: RATE,
            channels: 2,
            frames,
        };
        let mut r = Render::new(file, &sha256_hex(&bytes), facts, devices, 0, SECOND);
        r.name = name.to_owned();
        r.fingerprint = render_fingerprint(&p.document, &t, &r.devices, 0, SECOND);
        t.renders.push(r);
        files.push((file, bytes));
    }
    let notes_pattern = t.patterns[0].clone();
    let (r, bytes) = pattern_render(
        &p.document,
        &t,
        &notes_pattern,
        "renders/Notes_120.wav",
        render::without_effects(&t),
        120.0,
    );
    t.patterns[0].renders.push(r);
    files.push(("renders/Notes_120.wav", bytes));
    for (file, bytes) in files {
        p.add_file(file, bytes);
    }
    let each = 0.25 * 0.5 / 2f64.sqrt();
    let heard_rows = vec![heard(&t, 0.1, 0.9, each, each)];
    let fingerprints: serde_json::Map<String, Value> = t
        .renders
        .iter()
        .map(|r| (r.file.clone(), json!(r.fingerprint)))
        .chain(
            t.patterns[0]
                .renders
                .iter()
                .map(|r| (r.file.clone(), json!(r.fingerprint))),
        )
        .collect();
    p.document.tracks.push(t);
    Fixture {
        name: "render-choice",
        project: p,
        expected: json!({
            "what": "An instrument and an effect of plugins no one has (fictitious), with a render without effects (the synth alone), one with effects (the synth halved) and a render of the pattern alone at 120 bpm (7.6, 7.7). A reader that has neither plugin plays the render that stands in for both, the one with effects, through the track's fader at 0.5, so the master's RMS per output channel over the window is within 0.5 dB of these. An edit to the effect makes only the render that holds it stale; a note edit makes all three stale. The fingerprints are the test vectors of 7.7's two descriptors.",
            "plays": "renders/Synth_wet.wav",
            "tolerance_db": 0.5,
            "heard": heard_rows,
            "fingerprints": fingerprints,
            "effect_edit": {
                "device": halver_id.as_str(), "param": "0", "value": 0.75,
                "stale": {"renders/Notes_120.wav": false, "renders/Synth_dry.wav": false, "renders/Synth_wet.wav": true},
            },
            "note_edit": {
                "velocity": 0.5,
                "stale": {"renders/Notes_120.wav": true, "renders/Synth_dry.wav": true, "renders/Synth_wet.wav": true},
            },
        }),
    }
}

// --- the Level 2 fixtures ------------------------------------------------------------------------

/// The document a fixture's folder holds, as written.
fn written(folder: &Path, title: &str) -> Document {
    tutti::read(&fs::read(folder.join(format!("{title}.tutti"))).unwrap()).unwrap()
}

/// everything: every feature (the every-feature example's project, with ids of its own), and
/// what a round trip keeps of it.
fn everything(folder: &Path, ids: &mut Ids) {
    write_project(folder, "Everything", every_feature(ids));
    let doc = written(folder, "Everything");
    let shared: Vec<Value> = tutti_schema::shared_patterns(&doc)
        .into_iter()
        .map(|(pattern, parts)| json!({"pattern": pattern, "parts": parts}))
        .collect();
    write_json(
        &folder.join("expected.json"),
        &json!({
            "what": "Every feature of the format. Read, written back by another program, read again: equal (3.1) to what was read, generator left out; every id kept; each pattern several parts play still one pattern; the files index complete and valid; README.md written by the new writer; the required extension kept, a reader that lacks it writing the project all the same, preserving (8.1, 8.3).",
            "equal": true,
            "ids": tutti_schema::ids_of(&doc),
            "shared_patterns": shared,
            "files": doc.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
            "extensions_required": doc.extensions_required,
            "readme": "README.md",
        }),
    );
}

/// merge-different-tracks: a project, two branches each editing one track, and their merge.
fn merge_different_tracks(folder: &Path, ids: &mut Ids) {
    let title = "Merge-different-tracks";
    let mut p = Project::new(generator());
    let mut drums = Track::new(ids.next("trk"), "Drums");
    let beat: Vec<Event> = [
        (0, 36, 0.9),
        (480, 42, 0.5),
        (960, 38, 0.85),
        (1440, 42, 0.5),
        (1920, 36, 0.9),
        (2400, 42, 0.5),
        (2880, 38, 0.85),
        (3360, 42, 0.5),
    ]
    .iter()
    .map(|&(at, pitch, velocity)| note(ids, at, 120, pitch, velocity))
    .collect();
    let beat = pattern(ids, "Beat", 3840, beat);
    let beat_id = beat.id.clone();
    drums.patterns.push(beat);
    for at in [0, 3840] {
        drums.parts.push(part(ids, &beat_id, at, 3840));
    }
    let mut bass = Track::new(ids.next("trk"), "Bass");
    let line: Vec<Event> = [45u8, 45, 52, 48]
        .iter()
        .enumerate()
        .map(|(i, &pitch)| note(ids, i as i64 * 960, 840, pitch, 0.8))
        .collect();
    let line = pattern(ids, "Line", 3840, line);
    let line_id = line.id.clone();
    bass.patterns.push(line);
    for at in [0, 3840] {
        bass.parts.push(part(ids, &line_id, at, 3840));
    }
    let (drums_id, bass_id) = (drums.id.clone(), bass.id.clone());
    p.document.tracks = vec![drums, bass];
    write_project(folder, title, p);
    let base = written(folder, title);

    // The drums branch: the first kick softer, a ghost snare added, the drums' fader down a bit.
    let kick_id = base.track(&drums_id).unwrap().patterns[0].events[0]
        .id()
        .to_owned();
    let ghost = note(ids, 3600, 120, 38, 0.4);
    let drums_edit = |doc: &mut Document| {
        let t = doc.track_mut(&drums_id).unwrap();
        if let Event::Note(n) = &mut t.patterns[0].events[0] {
            n.velocity = 0.7;
        }
        t.patterns[0].events.push(ghost.clone());
        t.channel.gain = 0.8;
    };
    // The bass branch: the third note down a tone, the second part moved, a part added.
    let bass_note_id = base.track(&bass_id).unwrap().patterns[0].events[2]
        .id()
        .to_owned();
    let moved_id = base.track(&bass_id).unwrap().parts[1].id.clone();
    let added = part(ids, &line_id, 11520, 3840);
    let bass_edit = |doc: &mut Document| {
        let t = doc.track_mut(&bass_id).unwrap();
        if let Event::Note(n) = &mut t.patterns[0].events[2] {
            n.pitch = 50;
        }
        t.parts[1].at = 7680;
        t.parts.push(added.clone());
    };
    let branch = |edits: &[&dyn Fn(&mut Document)]| {
        let mut doc = base.clone();
        for edit in edits {
            edit(&mut doc);
        }
        doc.normalise();
        write(&doc)
    };
    let branches = folder.join("branches");
    fs::create_dir_all(&branches).unwrap();
    write_text(&branches.join("drums.tutti"), &branch(&[&drums_edit]));
    write_text(&branches.join("bass.tutti"), &branch(&[&bass_edit]));
    write_text(
        &branches.join("merged.tutti"),
        &branch(&[&drums_edit, &bass_edit]),
    );
    let set = |id: &str, key: &str, from: Value, value: Value| json!({"op": "set", "id": id, "key": key, "from": from, "value": value});
    let add = |id: &str, key: &str, value: Value| json!({"op": "add", "id": id, "key": key, "value": value});
    write_json(
        &folder.join("expected.json"),
        &json!({
            "what": "Two branches of this project, each one writer's edit of one track (the drums on one, the bass on the other; what either adds has a new id), both in the pretty form: git merges them without a conflict, this document their base, and the result is the document with both edits, valid. A Level 2 writer that makes these edits to this project, each from the base, writes documents that merge the same way. An edit: `set` puts `value` at `key` (keys with `/` between) in the object with the id, where `from` was (absent: the default); `add` appends `value` to the array at `key` in the object with the id.",
            "base": format!("{title}.tutti"),
            "branches": [
                {"name": "drums", "document": "branches/drums.tutti", "edits": [
                    set(&kick_id, "velocity", json!(0.9), json!(0.7)),
                    add(beat_id.as_str(), "events", serde_json::to_value(&ghost).unwrap()),
                    set(drums_id.as_str(), "channel/gain", json!(1.0), json!(0.8)),
                ]},
                {"name": "bass", "document": "branches/bass.tutti", "edits": [
                    set(&bass_note_id, "pitch", json!(52), json!(50)),
                    set(moved_id.as_str(), "at", json!(3840), json!(7680)),
                    add(bass_id.as_str(), "parts", serde_json::to_value(&added).unwrap()),
                ]},
            ],
            "merged": "branches/merged.tutti",
            "git": format!("git merge-file -p branches/drums.tutti {title}.tutti branches/bass.tutti"),
        }),
    );
}
