//! The converter interface through a test format, `Lines`: a folder holding `song.lines` (one
//! note per line: pitch, tick, length) and `hit.wav`, a made sample the conversion puts three
//! ways (an audio piece's file, the instrument's own file, and a state). Converted into a store
//! in memory and on disk, opened again, the same twice; what a recording source notes; each of
//! `write`'s refusals with nothing written; a format's defaults; the ids; the export.

use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::json;
use tutti::folder::{Entry, Folder, FolderError, Kind, MemoryStore, Store};
use tutti::render::sha256_hex;
use tutti::{Document, Event, Generator, Id, Project, Track};
use tutti_convert::{
    Contents, Conversion, Directions, Error, Export, Format, Found, Place, Progress, Put, Quiet,
    Recording, RemarkKind, Report, Source, derived_id, fingerprint, write, write_export,
};
use tutti_fs::DirStore;

const SONG: &str = "60 0 960\n64 960 480\n67 1440 2400\n";

fn generator() -> Generator {
    Generator::new(
        "tutti-convert-tests",
        "0.1",
        "https://github.com/mxm-audio/tutti",
    )
}

/// A made WAV: 16-bit mono at 48 kHz, `frames` frames of a ramp.
fn wav(frames: u32) -> Vec<u8> {
    let data = frames * 2;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&48_000u32.to_le_bytes());
    out.extend_from_slice(&96_000u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for i in 0..frames {
        out.extend_from_slice(&((i as i16).wrapping_mul(7)).to_le_bytes());
    }
    out
}

const FRAMES: u32 = 4800;
const STATE: &[u8] = b"the instrument's state";

fn join(at: &str, name: &str) -> String {
    if at.is_empty() {
        name.to_owned()
    } else {
        format!("{at}/{name}")
    }
}

fn read(source: &dyn Source, reference: &str) -> Result<Vec<u8>, Error> {
    source.read(reference).map_err(|error| Error::Source {
        reference: reference.to_owned(),
        error,
    })
}

struct Lines;

impl Format for Lines {
    fn id(&self) -> &'static str {
        "lines"
    }

    fn name(&self) -> &'static str {
        "Lines test project"
    }

    fn version(&self) -> u32 {
        1
    }

    fn directions(&self) -> Directions {
        Directions {
            to_tutti: true,
            from_tutti: true,
        }
    }

    fn detect(&self, source: &dyn Source, at: &str, kind: Kind) -> Result<Option<Found>, Error> {
        if kind != Kind::Folder {
            return Ok(None);
        }
        let song = join(at, "song.lines");
        match source.size(&song) {
            Ok(_) => Ok(Some(Found {
                at: at.to_owned(),
                name: at
                    .rsplit('/')
                    .next()
                    .filter(|n| !n.is_empty())
                    .unwrap_or("Song")
                    .to_owned(),
            })),
            Err(FolderError::Missing) => Ok(None),
            Err(error) => Err(Error::Source {
                reference: song,
                error,
            }),
        }
    }

    fn inputs(&self, _source: &dyn Source, found: &Found) -> Result<Vec<String>, Error> {
        Ok(vec![
            join(&found.at, "song.lines"),
            join(&found.at, "hit.wav"),
        ])
    }

    fn to_tutti(
        &self,
        source: &dyn Source,
        found: &Found,
        progress: &mut dyn Progress,
    ) -> Result<Conversion, Error> {
        let song_ref = join(&found.at, "song.lines");
        let hit_ref = join(&found.at, "hit.wav");
        let song = String::from_utf8(read(source, &song_ref)?).map_err(|e| Error::Unreadable {
            reference: song_ref.clone(),
            why: e.to_string(),
        })?;
        progress.step(1, 2, &song_ref);
        let hit = read(source, &hit_ref)?;

        let device = derived_id("dev", &["1"]);
        let mut events = Vec::new();
        for (n, line) in song.lines().enumerate() {
            let v: Vec<i64> = line
                .split_whitespace()
                .map(|x| {
                    x.parse().map_err(|_| Error::Unreadable {
                        reference: song_ref.clone(),
                        why: format!("line {}: {line}", n + 1),
                    })
                })
                .collect::<Result<_, _>>()?;
            events.push(
                json!({"type": "note", "id": derived_id("evt", &[&(n + 1).to_string()]).as_str(),
                "at": v[1], "length": v[2], "pitch": v[0], "velocity": 0.8}),
            );
        }
        events.push(
            json!({"type": "note", "id": "evt_hit", "at": 0, "length": 960, "pitch": 36,
            "velocity": 1.0, "audio": {"file": "audio/hit.wav", "sha256": sha256_hex(&hit),
            "sample_rate": 48000, "channels": 1, "frames": FRAMES, "from": 0, "to": FRAMES,
            "map": [{"frame": 0, "tick": 0}], "bpm": null}}),
        );
        let track: Track = serde_json::from_value(json!({
            "id": derived_id("trk", &["1"]).as_str(), "name": "Lines",
            "devices": [{"id": device.as_str(), "role": "instrument",
                "plugin": "clap:org.example.sampler",
                "state": {"sha256": sha256_hex(STATE), "kind": "bin"}}],
            "patterns": [{"id": "pat_1", "length": 3840, "events": events}],
            "parts": [{"id": "prt_1", "pattern": "pat_1", "at": 0, "length": 3840}],
        }))
        .expect("a test track");
        let mut document = Document::new(generator());
        document.tracks.push(track);

        let mut report = Report::new();
        report.unplayed("the hit's choke group");
        Ok(Conversion {
            name: found.name.clone(),
            document,
            files: vec![
                Put {
                    place: Place::Audio {
                        file: "hit.wav".to_owned(),
                    },
                    contents: Contents::Copy {
                        from: hit_ref.clone(),
                    },
                },
                Put {
                    place: Place::DeviceFile {
                        device,
                        path: "samples/hit.wav".to_owned(),
                    },
                    contents: Contents::Copy { from: hit_ref },
                },
                Put {
                    place: Place::State {
                        kind: "bin".to_owned(),
                    },
                    contents: Contents::Bytes(STATE.to_vec()),
                },
            ],
            report,
        })
    }

    fn from_tutti(
        &self,
        document: &Document,
        _project: &dyn Source,
        _progress: &mut dyn Progress,
    ) -> Result<Export, Error> {
        let mut song = String::new();
        for event in &document.tracks[0].patterns[0].events {
            if let Event::Note(n) = event {
                if n.audio.is_none() {
                    song.push_str(&format!("{} {} {}\n", n.pitch, n.at, n.length));
                }
            }
        }
        Ok(Export {
            files: vec![
                ("song.lines".to_owned(), Contents::Bytes(song.into_bytes())),
                (
                    "hit.wav".to_owned(),
                    Contents::Copy {
                        from: "audio/hit.wav".to_owned(),
                    },
                ),
            ],
            report: Report::new(),
        })
    }
}

fn source() -> MemoryStore {
    let store = MemoryStore::new();
    store.put("Songs/a/song.lines", SONG.as_bytes().to_vec());
    store.put("Songs/a/hit.wav", wav(FRAMES));
    store.put("Songs/notes.txt", b"not a project".to_vec());
    store
}

fn found() -> Found {
    Found {
        at: "Songs/a".to_owned(),
        name: "a".to_owned(),
    }
}

fn conversion(source: &dyn Source) -> Conversion {
    Lines.to_tutti(source, &found(), &mut Quiet).unwrap()
}

/// Converted and written into `out`, then opened again: valid, as the format built it.
fn converted_into<S: Store>(source: &dyn Source, out: S) -> Document {
    write(
        conversion(source),
        source,
        out.clone(),
        &generator(),
        &mut Quiet,
    )
    .unwrap();
    let opened = Project::open(out).unwrap();
    assert!(opened.report.is_valid(), "{:?}", opened.report.issues);
    opened.project.document().clone()
}

#[test]
fn detects_its_projects_and_names_their_inputs() {
    let source = source();
    assert_eq!(
        Lines.detect(&source, "Songs/a", Kind::Folder).unwrap(),
        Some(found())
    );
    assert_eq!(Lines.detect(&source, "Songs", Kind::Folder).unwrap(), None);
    assert_eq!(
        Lines
            .detect(&source, "Songs/notes.txt", Kind::File)
            .unwrap(),
        None
    );
    assert_eq!(
        Lines.inputs(&source, &found()).unwrap(),
        ["Songs/a/song.lines", "Songs/a/hit.wav"]
    );
}

#[test]
fn a_conversion_is_written_into_memory_and_opens_again() {
    let source = source();
    let out = MemoryStore::new();
    let document = converted_into(&source, out.clone());
    let files = out.files();
    let hit = wav(FRAMES);
    assert_eq!(files["audio/hit.wav"], hit);
    assert_eq!(files["plugins/files/dev_1/samples/hit.wav"], hit);
    assert_eq!(
        files[&format!("plugins/states/{}.bin", sha256_hex(STATE))],
        STATE
    );
    assert!(files.contains_key("a.tutti") && files.contains_key("README.md"));
    let notes = document.tracks[0].patterns[0].events.len();
    assert_eq!(notes, 4);
}

#[test]
fn a_conversion_is_written_on_disk_from_disk() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("tutti-convert-disk");
    let _ = std::fs::remove_dir_all(&dir);
    let source = DirStore::new(dir.join("card"));
    source.make_folder("Songs/a").unwrap();
    source
        .write_new("Songs/a/song.lines", SONG.as_bytes())
        .unwrap();
    source.write_new("Songs/a/hit.wav", &wav(FRAMES)).unwrap();
    let document = converted_into(&source, DirStore::new(dir.join("out")));
    let in_memory = converted_into(&self::source(), MemoryStore::new());
    assert_eq!(fingerprint(&document), fingerprint(&in_memory));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn converting_twice_writes_the_same_document() {
    let source = source();
    let a = converted_into(&source, MemoryStore::new());
    let b = converted_into(&source, MemoryStore::new());
    assert!(tutti::equal(&a, &b));
    assert_eq!(fingerprint(&a), fingerprint(&b));
}

#[test]
fn the_fingerprint_leaves_the_generator_out_and_nothing_else() {
    let a = converted_into(&source(), MemoryStore::new());
    let mut b = a.clone();
    b.generator = Generator::new("another program", "9", "https://example.org");
    assert_eq!(fingerprint(&a), fingerprint(&b));
    if let Event::Note(n) = &mut b.tracks[0].patterns[0].events[0] {
        n.pitch += 1;
    }
    assert_ne!(fingerprint(&a), fingerprint(&b));
}

#[test]
fn a_recording_notes_exactly_what_was_read_and_copied() {
    let source = source();
    let recording = Recording::new(&source);
    let c = Lines.to_tutti(&recording, &found(), &mut Quiet).unwrap();
    write(c, &recording, MemoryStore::new(), &generator(), &mut Quiet).unwrap();
    let served = recording.served().unwrap();
    assert_eq!(
        served.keys().collect::<Vec<_>>(),
        ["Songs/a/hit.wav", "Songs/a/song.lines"]
    );
    let hit = &served["Songs/a/hit.wav"];
    assert_eq!(hit.sha256, sha256_hex(&wav(FRAMES)));
    assert_eq!(hit.size, wav(FRAMES).len() as u64);
    assert_eq!(
        served["Songs/a/song.lines"].sha256,
        sha256_hex(SONG.as_bytes())
    );
}

/// A source whose one file changes after it is first read.
struct Moving {
    reads: Mutex<u32>,
}

impl Folder for Moving {
    fn read(&self, _reference: &str) -> Result<Vec<u8>, FolderError> {
        let mut reads = self.reads.lock().unwrap();
        *reads += 1;
        Ok(if *reads == 1 {
            b"60 0 960".to_vec()
        } else {
            b"61 0 960".to_vec()
        })
    }
}

impl Source for Moving {
    fn entries(&self, _folder: &str) -> Result<Vec<Entry>, FolderError> {
        Ok(Vec::new())
    }
}

#[test]
fn a_file_that_changes_between_two_reads_is_refused() {
    let moving = Moving {
        reads: Mutex::new(0),
    };
    let recording = Recording::new(&moving);
    assert!(recording.read("song.lines").is_ok());
    assert!(matches!(
        recording.read("song.lines"),
        Err(FolderError::Other(_))
    ));
    assert!(matches!(recording.served(), Err(Error::Changed(r)) if r == "song.lines"));
}

#[test]
fn an_output_holding_a_file_is_refused_and_private_files_are_kept() {
    let source = source();
    let taken = MemoryStore::new();
    taken.put("README.md", b"someone's".to_vec());
    let refused = write(
        conversion(&source),
        &source,
        taken,
        &generator(),
        &mut Quiet,
    );
    assert!(matches!(refused, Err(Error::OutputNotEmpty(r)) if r == "README.md"));

    let staged = MemoryStore::new();
    staged.put("private/dk.example.watcher/owner", b"the watcher".to_vec());
    write(
        conversion(&source),
        &source,
        staged.clone(),
        &generator(),
        &mut Quiet,
    )
    .unwrap();
    assert_eq!(
        staged.files()["private/dk.example.watcher/owner"],
        b"the watcher"
    );
}

/// The conversion with one change, refused before anything is written.
fn refused(change: impl FnOnce(&mut Conversion), why: &str) {
    let source = source();
    let mut c = conversion(&source);
    change(&mut c);
    let out = MemoryStore::new();
    match write(c, &source, out.clone(), &generator(), &mut Quiet) {
        Err(Error::Placement { why: got, .. }) => assert!(got.contains(why), "{got}"),
        other => panic!("expected a placement refusal ({why}), got {other:?}"),
    }
    assert!(
        out.files().is_empty(),
        "nothing is written: {:?}",
        out.files().keys()
    );
}

#[test]
fn each_misplaced_file_is_refused_before_anything_is_written() {
    refused(
        |c| {
            let again = c.files[0].clone();
            c.files.push(again);
        },
        "two files go to this place",
    );
    refused(
        |c| {
            c.files.push(Put {
                place: Place::DeviceFile {
                    device: derived_id("dev", &["1"]),
                    path: "samples/HIT.wav".to_owned(),
                },
                contents: Contents::Bytes(Vec::new()),
            });
        },
        "two files go to this place",
    );
    refused(
        |c| {
            c.files[0].place = Place::Audio {
                file: "hit 1.wav".to_owned(),
            };
        },
        "a name the project would change",
    );
    refused(
        |c| {
            c.files[0].place = Place::Audio {
                file: "other.wav".to_owned(),
            };
        },
        "no audio piece",
    );
    refused(
        |c| {
            c.files[1].place = Place::DeviceFile {
                device: Id::new("dev_9").unwrap(),
                path: "hit.wav".to_owned(),
            };
        },
        "no such device",
    );
    refused(
        |c| {
            c.files[1].place = Place::DeviceFile {
                device: derived_id("dev", &["1"]),
                path: "../hit.wav".to_owned(),
            };
        },
        "",
    );
    refused(
        |c| c.files[2].contents = Contents::Bytes(b"another state".to_vec()),
        "no device of the document refers to this state",
    );
}

struct DetectOnly;

impl Format for DetectOnly {
    fn id(&self) -> &'static str {
        "detect-only"
    }
    fn name(&self) -> &'static str {
        "Detect only"
    }
    fn version(&self) -> u32 {
        1
    }
    fn directions(&self) -> Directions {
        Directions {
            to_tutti: false,
            from_tutti: false,
        }
    }
    fn detect(&self, _: &dyn Source, _: &str, _: Kind) -> Result<Option<Found>, Error> {
        Ok(None)
    }
    fn inputs(&self, _: &dyn Source, _: &Found) -> Result<Vec<String>, Error> {
        Ok(Vec::new())
    }
}

#[test]
fn a_format_converts_no_way_it_does_not_implement() {
    let source = source();
    assert!(matches!(
        DetectOnly.to_tutti(&source, &found(), &mut Quiet),
        Err(Error::Unsupported {
            format: "detect-only",
            direction: "into tutti"
        })
    ));
    let document = Document::new(generator());
    assert!(matches!(
        DetectOnly.from_tutti(&document, &source, &mut Quiet),
        Err(Error::Unsupported {
            format: "detect-only",
            direction: "out of tutti"
        })
    ));
}

struct StopAfter(u64);

impl Progress for StopAfter {
    fn step(&mut self, _done: u64, _total: u64, _what: &str) {
        self.0 = self.0.saturating_sub(1);
    }
    fn cancelled(&self) -> bool {
        self.0 == 0
    }
}

#[test]
fn a_cancelled_write_stops_between_files() {
    let source = source();
    let out = MemoryStore::new();
    let stopped = write(
        conversion(&source),
        &source,
        out.clone(),
        &generator(),
        &mut StopAfter(1),
    );
    assert!(matches!(stopped, Err(Error::Cancelled)));
    assert!(!out.files().keys().any(|k| k.ends_with(".tutti")));
}

#[test]
fn derived_ids_are_readable_when_they_can_be_and_always_well_formed() {
    assert_eq!(derived_id("pat", &["3", "t1"]).as_str(), "pat_3_t1");
    let hashed = [
        derived_id("pat", &["a_b"]),
        derived_id("pat", &["a", "b"]),
        derived_id("pat", &[""]),
        derived_id("pat", &["a b"]),
        derived_id(&"p".repeat(70), &["1"]),
        derived_id("pat", &[&"9".repeat(80)]),
    ];
    for id in &hashed {
        assert!(id.is_well_formed(), "{}", id.as_str());
    }
    assert_ne!(hashed[0], hashed[1]);
    assert!(hashed[0].as_str().starts_with("pat_") && hashed[0].as_str().len() == 4 + 32);
    assert!(hashed[4].as_str().starts_with("id_"));
    assert_eq!(derived_id("pat", &["a_b"]), derived_id("pat", &["a_b"]));
}

#[test]
fn an_export_is_written_new_and_refused_when_misplaced() {
    let source = source();
    let project = MemoryStore::new();
    let document = converted_into(&source, project.clone());
    let export = Lines.from_tutti(&document, &project, &mut Quiet).unwrap();
    let out = MemoryStore::new();
    write_export(export.clone(), &project, out.clone()).unwrap();
    assert_eq!(out.files()["song.lines"], SONG.as_bytes());
    assert_eq!(out.files()["hit.wav"], wav(FRAMES));
    assert!(matches!(
        write_export(export.clone(), &project, out.clone()),
        Err(Error::Store {
            error: FolderError::Exists,
            ..
        })
    ));

    let mut twice = export.clone();
    twice
        .files
        .push(("SONG.lines".to_owned(), Contents::Bytes(Vec::new())));
    assert!(matches!(
        write_export(twice, &project, MemoryStore::new()),
        Err(Error::Placement { .. })
    ));
    let mut outside = export;
    outside.files[0].0 = "../song.lines".to_owned();
    assert!(matches!(
        write_export(outside, &project, MemoryStore::new()),
        Err(Error::Placement { .. })
    ));
}

#[test]
fn a_report_keeps_its_remarks_through_serde() {
    let c = conversion(&source());
    assert_eq!(c.report.remarks[0].kind, RemarkKind::Unplayed);
    let text = serde_json::to_string(&c.report).unwrap();
    assert!(text.contains("\"unplayed\""), "{text}");
    let back: Report = serde_json::from_str(&text).unwrap();
    assert_eq!(back, c.report);
}
