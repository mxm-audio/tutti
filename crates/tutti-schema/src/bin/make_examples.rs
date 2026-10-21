//! Writes `examples/`: the example projects (`minimal`, `typical`, `every-feature`) and the
//! Level 0 conformance fixtures, from the crate itself, so every document is the pretty form and
//! every hash and fingerprint is real. Deterministic: ids from fixed times, audio from closed
//! forms, so running it again changes nothing (`cargo run -p tutti-schema --bin examples`).

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
        write_text(
            &folder.join("expected.json"),
            &format!(
                "{}\n",
                serde_json::to_string_pretty(&json!({"what": case.what, "error": case.error}))
                    .unwrap()
            ),
        );
    }
    println!("examples written under {}", examples.display());
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
    Pattern {
        id: ids.next("pat"),
        name: name.to_owned(),
        length,
        key: None,
        events,
        extensions: Extensions::new(),
        unknown: Unknown::new(),
    }
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
    d.r#loop = Some(Loop {
        start: 2 * bar,
        end: 6 * bar,
        enabled: false,
        unknown: Unknown::new(),
    });

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
    d.r#loop.as_mut().unwrap().enabled = true;
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
    let (render, _) = sine(440.0, 2.0, 0.2, RATE);
    let render_bytes = wav(1, RATE, &render);
    let render_hash = sha256_hex(&render_bytes);
    let loop_track_clone = loop_track.clone();
    let doc_for_fingerprint = d.clone();
    let fingerprint = render_fingerprint(&doc_for_fingerprint, &loop_track_clone, 0, 2 * bar);
    let loop_track = d.tracks.iter_mut().find(|t| t.name == "Loop").unwrap();
    loop_track.renders.push(Render {
        file: "renders/Loop_000.wav".to_owned(),
        sha256: render_hash,
        from: 0,
        to: 2 * bar,
        fingerprint,
        unknown: Unknown::new(),
    });
    d.tracks.insert(0, folder);
    p.add_file("audio/Pad-220.wav", pad_bytes);
    p.add_file("renders/Loop_000.wav", render_bytes);
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
    });
    out.push(Hostile {
        name: "duplicate-key",
        what: "the same key twice in one object",
        error: "duplicate key at /tempo/0/bpm",
        files: vec![(doc_name, br#"{"format": "tutti", "version": "1.0", "generator": {"name": "x"}, "ppq": 960,
  "tempo": [{"at": 0, "bpm": 120, "bpm": 130}], "meter": [{"at": 0, "numerator": 4, "denominator": 4}]}
"#.to_vec())],
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
    });
    out
}
