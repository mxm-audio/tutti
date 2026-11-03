//! Made projects, built byte by byte here (no recording, no one's song): the files read and
//! written back exactly; what is refused; detection; the song as the device plays it, rule by
//! rule; the conversion, valid, the same twice, and read back to the same model.

use std::collections::BTreeMap;

use tutti::folder::{Folder, Kind, MemoryStore};
use tutti::{Document, Event, Generator, Project, Track};
use tutti_convert::{Error, Format, Found, Quiet, RemarkKind, fingerprint, write};
use tutti_polyend::files::{
    Fx, Header, NOTE_CUT, NOTE_FADE, NOTE_OFF, PCM_OFFSET, PatternFile, ProjectFile, STEPS, Step,
    Track as PTrack,
};
use tutti_polyend::model::samples;
use tutti_polyend::song::{End, TICKS_PER_STEP, fx, micromove_ticks, song};
use tutti_polyend::{PolyendTracker, read, to_model};

// --- building files ----------------------------------------------------------------------------

fn header(id: &[u8; 2], kind: u16, version: u8, size: usize) -> Header {
    Header {
        id: *id,
        kind,
        firmware: [2, 2, 1, 0],
        version: [version; 4],
        size: size as u16,
        padding: [0, 0],
    }
}

/// A project file of version 16 or 17: the song, the tempo, the track names.
fn project(version: u8, playlist: &[u8], tempo: f32) -> Vec<u8> {
    let len = 0x4D8 + 32;
    let mut bytes = vec![0u8; len];
    bytes[..16].copy_from_slice(&header(b"MT", 0, version, len).to_bytes());
    bytes[0x10..0x10 + playlist.len()].copy_from_slice(playlist);
    bytes[0x117] = 24;
    bytes[0x1C0..0x1C4].copy_from_slice(&tempo.to_le_bytes());
    for (t, name) in ["Drums", "Bass"].iter().enumerate() {
        let at = 0x428 + 21 * t;
        bytes[at..at + name.len()].copy_from_slice(name.as_bytes());
    }
    for t in 0..8 {
        bytes[0x4D0 + t] = 100;
    }
    bytes[len - 4..].copy_from_slice(&[1, 2, 3, 4]);
    bytes
}

fn pattern(version: u8, tracks: usize, last: u8, crc: bool) -> PatternFile {
    let step = if version >= 6 { 8 } else { 6 };
    let size = 16 + 12 + tracks * (1 + STEPS * step) + 4;
    PatternFile {
        header: header(b"PM", 0, version, size),
        reserved: [0, 0, 200, 66, 0, 0, 72, 66, 0, 0, 0, 0],
        tracks: (0..tracks)
            .map(|_| PTrack {
                last,
                steps: vec![Step::EMPTY; STEPS],
            })
            .collect(),
        crc: crc.then_some([9, 8, 7, 6]),
    }
}

fn note(n: i8, instrument: u8) -> Step {
    Step {
        note: n,
        instrument,
        ..Step::EMPTY
    }
}

fn with_fx(mut step: Step, slot: usize, kind: u8, value: u8) -> Step {
    step.fx[slot] = Fx { kind, value };
    step
}

/// An instrument file: version 9, `frames` frames, mono or stereo, the samples a ramp.
fn instrument(frames: u32, channels: u8) -> Vec<u8> {
    let mut bytes = vec![0u8; PCM_OFFSET];
    bytes[..16].copy_from_slice(&header(b"TI", 1, 9, 372).to_bytes());
    bytes[16 + 44..16 + 48].copy_from_slice(&frames.to_le_bytes());
    bytes[16 + 256] = 100; // volume
    bytes[16 + 260] = 50; // panning
    bytes[PCM_OFFSET - 4..].copy_from_slice(&[5, 6, 7, 8]);
    for i in 0..frames as usize * channels as usize {
        bytes.extend_from_slice(&((i as i16).wrapping_mul(13)).to_le_bytes());
    }
    bytes
}

/// A project folder in a store: `Projects/<name>/…`.
struct Card {
    store: MemoryStore,
}

impl Card {
    fn new() -> Card {
        Card {
            store: MemoryStore::new(),
        }
    }
    fn put(&self, project: &str, path: &str, bytes: Vec<u8>) {
        self.store.put(&format!("Projects/{project}/{path}"), bytes);
    }
    fn pattern(&self, project: &str, number: u8, file: &PatternFile) {
        self.put(
            project,
            &format!("patterns/pattern_{number:02}.mtp"),
            file.to_bytes().unwrap(),
        );
    }
    fn found(&self, project: &str) -> Found {
        PolyendTracker
            .detect(&self.store, &format!("Projects/{project}"), Kind::Folder)
            .unwrap()
            .expect("a project")
    }
}

// --- the files ---------------------------------------------------------------------------------

#[test]
fn every_file_is_written_back_byte_for_byte() {
    for (version, tracks, crc) in [
        (4, 8, true),
        (5, 16, true),
        (5, 12, false),
        (6, 16, true),
        (6, 8, true),
    ] {
        let mut file = pattern(version, tracks, 31, crc);
        file.tracks[0].steps[3] = with_fx(note(60, 1), 1, fx::VOLUME, 40);
        if version >= 6 {
            file.tracks[1].steps[5] = Step {
                volume: 70,
                micromove: 20,
                ..note(48, 2)
            };
        }
        let bytes = file.to_bytes().unwrap();
        let back = PatternFile::parse(&bytes).unwrap();
        assert_eq!(back, file, "version {version}, {tracks} tracks");
        assert_eq!(back.to_bytes().unwrap(), bytes);
    }
    let bytes = project(16, &[1, 2], 120.0);
    let p = ProjectFile::parse(&bytes).unwrap();
    assert_eq!(p.to_bytes(), bytes);
    assert_eq!(p.playlist(), [1, 2]);
    assert_eq!(p.tempo(), 120.0);
    assert_eq!(p.track_name(1), "Bass");
}

#[test]
fn unknown_versions_and_the_older_layout_are_refused_by_name() {
    let card = Card::new();
    card.put("Old", "project.mt", project(16, &[1], 120.0));
    card.pattern("Old", 1, &pattern(3, 8, 15, true));
    match read(&card.store, &card.found("Old")) {
        Err(Error::Unreadable { reference, why }) => {
            assert!(reference.ends_with("pattern_01.mtp"), "{reference}");
            assert!(why.contains("version 3"), "{why}");
        }
        other => panic!("{other:?}"),
    }
    card.put("Older", "project.mt", project(16, &[1], 120.0));
    card.put("Older", "instruments/instrument_01.mti", vec![0; 64]);
    assert!(matches!(
        read(&card.store, &card.found("Older")),
        Err(Error::Unreadable { why, .. }) if why.contains(".mti")
    ));
    card.put("Ancient", "project.mt", project(15, &[1], 120.0));
    assert!(matches!(
        read(&card.store, &card.found("Ancient")),
        Err(Error::Unreadable { why, .. }) if why.contains("version 15")
    ));
}

#[test]
fn projects_are_found_and_the_workspace_is_not() {
    let card = Card::new();
    card.put("Song", "project.mt", project(16, &[1], 120.0));
    card.store
        .put("Workspace/project.mt", project(16, &[1], 120.0));
    let s = &card.store;
    assert_eq!(card.found("Song").name, "Song");
    assert!(
        PolyendTracker
            .detect(s, "Workspace", Kind::Folder)
            .unwrap()
            .is_none()
    );
    assert!(
        PolyendTracker
            .detect(s, "Projects", Kind::Folder)
            .unwrap()
            .is_none()
    );
    assert!(
        PolyendTracker
            .detect(s, "Projects/Song/project.mt", Kind::File)
            .unwrap()
            .is_none()
    );
    let inputs = PolyendTracker.inputs(s, &card.found("Song")).unwrap();
    assert_eq!(inputs, ["Projects/Song/project.mt"]);
}

// --- the song ----------------------------------------------------------------------------------

/// A one-pattern project: track 1 holds the steps given.
fn one_pattern(version: u8, steps: &[(usize, usize, Step)], last: u8, playlist: &[u8]) -> Card {
    let card = Card::new();
    card.put("S", "project.mt", project(16, playlist, 120.0));
    let mut file = pattern(version, 16, last, true);
    for &(t, s, step) in steps {
        file.tracks[t].steps[s] = step;
    }
    card.pattern("S", 1, &file);
    card
}

#[test]
fn a_step_is_a_sixteenth_and_a_note_rings_to_the_tracks_next_event() {
    let card = one_pattern(
        5,
        &[
            (0, 0, note(60, 0)),
            (0, 4, note(62, 0)),
            (0, 6, note(NOTE_OFF, 0)),
            (0, 8, note(64, 0)),
            (0, 9, note(NOTE_CUT, 0)),
            (0, 10, note(65, 0)),
            (0, 12, note(NOTE_FADE, 0)),
            (0, 14, note(67, 0)),
        ],
        15,
        &[1],
    );
    let (model, _) = read(&card.store, &card.found("S")).unwrap();
    let s = song(&model);
    let got: Vec<(i64, i64, u8, End)> = s
        .notes
        .iter()
        .map(|n| (n.at, n.length, n.pitch, n.end))
        .collect();
    let step = TICKS_PER_STEP;
    assert_eq!(
        got,
        [
            (0, 4 * step, 60, End::Next),
            (4 * step, 2 * step, 62, End::Off),
            (8 * step, step, 64, End::Cut),
            (10 * step, 2 * step, 65, End::Fade),
            (14 * step, 2 * step, 67, End::SongEnd),
        ]
    );
    assert_eq!(s.end, 16 * step);
}

#[test]
fn micro_move_and_velocity_take_the_first_slot_then_the_second_then_the_step() {
    let card = one_pattern(
        6,
        &[
            (
                0,
                0,
                with_fx(
                    Step {
                        micromove: 10,
                        volume: 30,
                        ..note(60, 0)
                    },
                    1,
                    fx::MICROMOVE,
                    50,
                ),
            ),
            (
                1,
                0,
                with_fx(with_fx(note(60, 0), 1, fx::VOLUME, 20), 0, fx::VOLUME, 40),
            ),
            (
                2,
                0,
                Step {
                    micromove: 100,
                    volume: 80,
                    ..note(60, 0)
                },
            ),
            (3, 0, note(60, 0)),
        ],
        3,
        &[1],
    );
    let (model, _) = read(&card.store, &card.found("S")).unwrap();
    let s = song(&model);
    let by_track: BTreeMap<usize, (i64, f64)> = s
        .notes
        .iter()
        .map(|n| (n.track, (n.at, n.velocity)))
        .collect();
    assert_eq!(by_track[&0], (micromove_ticks(50), 0.3));
    assert_eq!(by_track[&1], (0, 0.4));
    assert_eq!(by_track[&2], (micromove_ticks(100), 0.8));
    assert_eq!(by_track[&3], (0, 1.0));
    assert_eq!(micromove_ticks(0), 0);
    assert_eq!(micromove_ticks(100), 47 * 5);
}

#[test]
fn every_track_plays_track_ones_length_and_break_ends_the_pattern() {
    let mut file = pattern(5, 16, 7, true);
    file.tracks[2].last = 127;
    file.tracks[2].steps[20] = note(60, 0);
    let card = Card::new();
    card.put("S", "project.mt", project(16, &[1, 2], 120.0));
    card.pattern("S", 1, &file);
    let mut broken = pattern(5, 16, 31, true);
    broken.tracks[5].steps[3] = with_fx(Step::EMPTY, 0, fx::BREAK_PATTERN, 1);
    card.pattern("S", 2, &broken);
    let (model, _) = read(&card.store, &card.found("S")).unwrap();
    let s = song(&model);
    assert_eq!(s.slots.iter().map(|x| x.steps).collect::<Vec<_>>(), [8, 4]);
    assert!(
        s.notes.is_empty(),
        "a step past track 1's length never plays"
    );
}

#[test]
fn tempo_changes_and_version_four_clears_the_midi_tracks() {
    let card = Card::new();
    card.put("S", "project.mt", project(16, &[1], 100.0));
    let mut file = pattern(4, 16, 7, true);
    file.tracks[0].steps[4] = with_fx(Step::EMPTY, 0, fx::TEMPO, 70);
    file.tracks[9].steps[0] = note(60, 50);
    card.pattern("S", 1, &file);
    let (model, _) = read(&card.store, &card.found("S")).unwrap();
    let s = song(&model);
    assert_eq!(s.tempo, [(0, 100.0), (4 * TICKS_PER_STEP, 140.0)]);
    assert!(s.notes.is_empty());
}

#[test]
fn a_missing_pattern_plays_empty() {
    let card = one_pattern(5, &[(0, 0, note(60, 0))], 3, &[1, 9, 1]);
    let (model, _) = read(&card.store, &card.found("S")).unwrap();
    let s = song(&model);
    assert_eq!(s.missing, [9]);
    assert_eq!(
        s.slots.iter().map(|x| x.steps).collect::<Vec<_>>(),
        [4, 32, 4]
    );
    assert_eq!(
        s.notes[0].length,
        36 * TICKS_PER_STEP,
        "rings through the empty slot"
    );
}

// --- the conversion ----------------------------------------------------------------------------

/// A song of two patterns: pattern 1 twice, once before pattern 2 (its last note ringing on
/// into it), once at the end; a mono and a stereo instrument, and one the song never plays.
fn song_card() -> Card {
    let card = Card::new();
    card.put("Tune", "project.mt", project(17, &[1, 2, 1], 120.0));
    let mut p1 = pattern(6, 16, 7, true);
    p1.tracks[0].steps[0] = note(60, 0);
    p1.tracks[0].steps[6] = Step {
        volume: 50,
        ..note(62, 0)
    };
    p1.tracks[1].steps[2] = note(36, 1);
    p1.tracks[1].steps[5] = with_fx(note(NOTE_OFF, 0), 0, 3, 4); // a roll on an off, kept
    p1.tracks[1].steps[100] = note(40, 1); // never played
    p1.tracks[8].steps[1] = note(60, 49); // a MIDI channel
    card.pattern("Tune", 1, &p1);
    let mut p2 = pattern(6, 16, 3, true);
    p2.tracks[0].steps[2] = note(NOTE_CUT, 0);
    card.pattern("Tune", 2, &p2);
    card.pattern("Tune", 3, &pattern(5, 8, 31, false)); // not in the song
    card.put(
        "Tune",
        "patterns/patternsMetadata",
        b"PAMD\x01\x00made".to_vec(),
    );
    card.put("Tune", "instruments/1 Lead.pti", instrument(300, 1));
    card.put("Tune", "instruments/2 Kick.pti", instrument(200, 2));
    card.put("Tune", "instruments/17 Spare.pti", instrument(50, 1));
    card
}

fn convert(card: &Card) -> (MemoryStore, Document, tutti_convert::Report) {
    let found = card.found("Tune");
    let conversion = PolyendTracker
        .to_tutti(&card.store, &found, &mut Quiet)
        .unwrap();
    let report = conversion.report.clone();
    let out = MemoryStore::new();
    let generator = Generator::new("made", "0", "https://example.org");
    write(conversion, &card.store, out.clone(), &generator, &mut Quiet).unwrap();
    let opened = Project::open(out.clone()).unwrap();
    assert!(
        opened.report.is_valid(),
        "{:?}",
        opened.report.errors().collect::<Vec<_>>()
    );
    let document = opened.project.document().clone();
    (out, document, report)
}

fn named<'a>(tracks: &'a [Track], name: &str) -> &'a Track {
    tracks
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("no track {name}"))
}

#[test]
fn a_project_converts_into_folders_of_instrument_tracks() {
    let card = song_card();
    let (out, document, report) = convert(&card);
    let drums = named(&document.tracks, "Drums");
    let lead = named(&drums.children, "Lead");
    assert_eq!(lead.devices[0].plugin, "clap:dk.mxm.mxm-creative-sampler");
    let device = lead.devices[0].id.as_str();
    let sample = &out.files()[&format!("plugins/files/{device}/sample.wav")];
    assert_eq!(&sample[..4], b"RIFF");
    assert_eq!(sample.len(), 44 + 600);

    // Pattern 1 plays twice: before pattern 2 its last note rings 2 steps into it (cut there),
    // at the song's end it stops with the song. Two variants, one part each.
    assert_eq!(lead.patterns.len(), 2);
    assert_eq!(lead.parts.len(), 2);
    let lengths: Vec<i64> = lead
        .patterns
        .iter()
        .map(|p| match &p.events.last().unwrap() {
            Event::Note(n) => n.length,
            _ => 0,
        })
        .collect();
    assert!(lengths.contains(&(4 * TICKS_PER_STEP)), "{lengths:?}");
    assert!(lengths.contains(&(2 * TICKS_PER_STEP)), "{lengths:?}");
    let first = lead.parts.iter().find(|p| p.at == 0).unwrap();
    assert_eq!(
        first.length,
        10 * TICKS_PER_STEP,
        "lengthened over the tail"
    );

    // The kick (stereo) ends at the off on track 2's step 5; the spare is kept, muted.
    let bass = named(&document.tracks, "Bass");
    assert_eq!(named(&bass.children, "Kick").patterns.len(), 1);
    let unused = named(&document.tracks, "Instruments not in the song");
    assert!(unused.channel.mute);
    assert_eq!(unused.children[0].name, "Spare");

    // The MIDI channel and the roll are named in the report.
    let texts: Vec<&str> = report.remarks.iter().map(|r| r.text.as_str()).collect();
    assert!(
        texts.iter().any(|t| t.contains("MIDI channel 2")),
        "{texts:?}"
    );
    assert!(
        report
            .remarks
            .iter()
            .all(|r| r.kind == RemarkKind::Unplayed || r.kind == RemarkKind::Approximated)
    );
}

#[test]
fn converting_again_writes_the_same_document() {
    let card = song_card();
    let (_, a, _) = convert(&card);
    let (_, b, _) = convert(&card);
    assert_eq!(fingerprint(&a), fingerprint(&b));
}

#[test]
fn the_model_comes_back_byte_for_byte_samples_and_all() {
    let card = song_card();
    let found = card.found("Tune");
    let (model, locators) = read(&card.store, &found).unwrap();
    let (out, document, _) = convert(&card);
    let (back, back_locators) = to_model(&document, &out).unwrap();
    assert!(back == model, "the model read back differs");
    for (index, locator) in &locators {
        assert_eq!(
            samples(&card.store, locator).unwrap(),
            samples(&out, &back_locators[index]).unwrap()
        );
    }
    let files = back.files(&out, &back_locators).unwrap();
    for (path, bytes) in files {
        let original = card.store.read(&format!("Projects/Tune/{path}")).unwrap();
        assert!(original == bytes, "{path} differs");
    }
}
