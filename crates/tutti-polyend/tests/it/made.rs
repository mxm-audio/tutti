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
    bytes[0x4DD] = 86; // the dry mix at 0 dB
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
    bytes[16 + 370] = 16; // bit depth
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
fn unknown_versions_are_refused_by_name() {
    let card = Card::new();
    card.put("Odd", "project.mt", project(16, &[1], 120.0));
    let mut odd = pattern(5, 8, 15, true);
    odd.header.version = [9; 4];
    card.put("Odd", "patterns/pattern_01.mtp", odd.to_bytes().unwrap());
    match read(&card.store, &card.found("Odd")) {
        Err(Error::Unreadable { reference, why }) => {
            assert!(reference.ends_with("pattern_01.mtp"), "{reference}");
            assert!(why.contains("version 9"), "{why}");
        }
        other => panic!("{other:?}"),
    }
    card.put("Ancient", "project.mt", project(15, &[1], 120.0));
    assert!(matches!(
        read(&card.store, &card.found("Ancient")),
        Err(Error::Unreadable { why, .. }) if why.contains("version 15")
    ));
    card.put("Two", "project.mt", project(16, &[1], 120.0));
    card.put(
        "Two",
        "instruments/instrument_01.mti",
        older_instrument(2, 368, 0, 50),
    );
    assert!(matches!(
        read(&card.store, &card.found("Two")),
        Err(Error::Unreadable { why, .. }) if why.contains("version 2")
    ));
    card.put("Stray", "project.mt", project(16, &[1], 120.0));
    card.put("Stray", "samples/instr05.wav", made_wav(10));
    assert!(matches!(
        read(&card.store, &card.found("Stray")),
        Err(Error::Unreadable { why, .. }) if why.contains("no instrument names")
    ));
}

/// An older instrument, `.mti`: the header and struct alone, its play mode and volume set.
fn older_instrument(version: u8, size: usize, play_mode: u8, volume: u8) -> Vec<u8> {
    let mut bytes = vec![0u8; 16 + size + 4];
    bytes[..16].copy_from_slice(&header(b"TI", 1, version, size).to_bytes());
    bytes[16 + 5..16 + 13].copy_from_slice(b"Kick.wav");
    bytes[16 + 60] = play_mode;
    bytes[16 + 256] = volume;
    bytes[16 + 260] = 50;
    bytes
}

/// A made WAV: 16-bit mono at 44.1 kHz, a ramp.
fn made_wav(frames: u32) -> Vec<u8> {
    let data = frames * 2;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&44_100u32.to_le_bytes());
    out.extend_from_slice(&88_200u32.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for i in 0..frames {
        out.extend_from_slice(&((i as i16).wrapping_mul(31)).to_le_bytes());
    }
    out
}

#[test]
fn the_older_layout_reads_converts_and_comes_back_byte_for_byte() {
    use tutti_polyend::files::{
        InstrumentFile, PlayMode, volume_1117_to_1118, volume_1300_to_1302,
    };
    // The firmware's conversions, at values off a rounding edge.
    assert_eq!(volume_1117_to_1118(50), 30);
    assert_eq!(volume_1117_to_1118(100), 51);
    assert_eq!(volume_1300_to_1302(100), 100);
    assert_eq!(volume_1300_to_1302(1), 1);
    assert_eq!(volume_1117_to_1118(0), 0);

    // A project of version 7 (its pad root note reset to 24), a pattern of version 2, an
    // instrument of version 6 playing beat slices and one of version 1 whose play mode 5 reads 6.
    let card = Card::new();
    let mut p = project(7, &[1], 120.0);
    p[0x117] = 40;
    card.put("Old", "project.mt", p);
    let mut file = pattern(2, 8, 7, false);
    file.tracks[0].steps[0] = note(60, 0);
    file.tracks[1].steps[0] = with_fx(note(60, 1), 0, fx::VOLUME, 100);
    card.pattern("Old", 1, &file);
    card.put(
        "Old",
        "instruments/instrument_01.mti",
        older_instrument(6, 372, 5, 50),
    );
    card.put(
        "Old",
        "instruments/instrument_02.mti",
        older_instrument(1, 368, 5, 50),
    );
    card.put("Old", "samples/instr01.wav", made_wav(300));
    card.put("Old", "samples/instr02.wav", made_wav(200));
    let found = card.found("Old");
    let (model, locators) = read(&card.store, &found).unwrap();
    assert_eq!(model.project.root_note(), 24, "reset before version 10");
    let head = |i: u8| -> &InstrumentFile { &model.instruments[&i].head };
    assert_eq!(head(0).play_mode(), PlayMode::BeatSlice);
    assert_eq!(
        head(1).play_mode(),
        PlayMode::Wavetable,
        "version 1's 5 reads 6"
    );
    assert_eq!(head(0).volume(), volume_1300_to_1302(50));
    assert_eq!(model.instruments[&0].name(), "Kick");

    // The beat slice note moved by 24 - (24 + 12); the volume effect converted, then played.
    let s = song(&model);
    let by_track: BTreeMap<usize, (u8, f64)> = s
        .notes
        .iter()
        .map(|n| (n.track, (n.pitch, n.velocity)))
        .collect();
    assert_eq!(by_track[&0].0, 48);
    assert_eq!(by_track[&1], (60, 1.0));

    // Every file written back, the WAVs included; converted, valid, and read back the same.
    for (path, bytes) in model.files(&card.store, &locators).unwrap() {
        let original = card.store.read(&format!("Projects/Old/{path}")).unwrap();
        assert!(original == bytes, "{path} differs");
    }
    let conversion = PolyendTracker
        .to_tutti(&card.store, &found, &mut Quiet)
        .unwrap();
    let out = MemoryStore::new();
    let generator = Generator::new("made", "0", "https://example.org");
    write(conversion, &card.store, out.clone(), &generator, &mut Quiet).unwrap();
    let opened = Project::open(out.clone()).unwrap();
    assert!(
        opened.report.is_valid(),
        "{:?}",
        opened.report.errors().collect::<Vec<_>>()
    );
    let (back, back_locators) = to_model(opened.project.document(), &out).unwrap();
    assert!(back == model, "the model read back differs");
    for (index, locator) in &locators {
        assert_eq!(
            samples(&card.store, locator).unwrap(),
            samples(&out, &back_locators[index]).unwrap()
        );
    }
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
    p1.tracks[0].steps[4] = note(48, 1); // a second instrument on track 1: a folder
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
    assert_eq!(drums.children.len(), 2, "two instruments, a folder");
    assert_eq!(lead.devices[0].plugin, "clap:dk.mxm.mxm-creative-sampler");
    // The sampler's snapshot: note 60 the sample's own pitch, the instrument's volume its level.
    let snapshot = |id: &str| {
        let clap = tutti_polyend::convert::clap_id(id);
        lead.devices[0]
            .params
            .iter()
            .find(|p| p.id == clap)
            .map(|p| p.value)
    };
    assert_eq!(snapshot("aroot"), Some(60.0));
    assert_eq!(snapshot("alevel"), Some(1.0));
    assert_eq!(tutti_polyend::convert::clap_id("aroot"), "93087939");
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
    let bass = named(&document.tracks, "Bass: Kick");
    assert!(
        bass.children.is_empty(),
        "one instrument, a track of its own"
    );
    assert_eq!(bass.patterns.len(), 1);
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

/// The pattern sizes the firmware validates against (`patternFileValidation.h`; version 6's 16
/// tracks are `sizeof(strPattern)`, which it asserts, and which it writes as it is): ours, for every
/// track count of either layout.
#[test]
fn pattern_files_are_the_sizes_the_firmware_knows() {
    for (version, sizes) in [(5, [6184, 9260, 12336]), (6, [8232, 12332, 16432])] {
        for (tracks, size) in [8, 12, 16].into_iter().zip(sizes) {
            let bytes = pattern(version, tracks, 15, true).to_bytes().unwrap();
            assert_eq!(bytes.len(), size, "version {version}, {tracks} tracks");
            assert_eq!(PatternFile::parse(&bytes).unwrap().tracks.len(), tracks);
        }
    }
}

// --- how the instruments sound -----------------------------------------------------------------

fn put_f32(bytes: &mut [u8], at: usize, v: f32) {
    bytes[16 + at..16 + at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_u16(bytes: &mut [u8], at: usize, v: u16) {
    bytes[16 + at..16 + at + 2].copy_from_slice(&v.to_le_bytes());
}

fn ends_with<'a>(tracks: &'a [Track], name: &str) -> &'a Track {
    tracks
        .iter()
        .find(|t| t.name.ends_with(name))
        .unwrap_or_else(|| panic!("no track ending {name}"))
}

fn param(track: &Track, id: &str) -> f64 {
    let clap = tutti_polyend::convert::clap_id(id);
    track.devices[0]
        .params
        .iter()
        .find(|p| p.id == clap)
        .unwrap_or_else(|| panic!("no {id}"))
        .value
}

fn db(gain: f64) -> f64 {
    20.0 * gain.log10()
}

#[test]
fn volumes_play_through_the_firmwares_tables() {
    use tutti_polyend::files::{amp_db, mix_db};
    assert_eq!(amp_db(0), None);
    for (v, want) in [(1, -24.0), (49, -0.96), (50, 0.0), (51, 0.48), (100, 24.0)] {
        assert!((amp_db(v).unwrap() - want).abs() < 1e-9, "{v}");
    }
    assert_eq!(mix_db(0), None);
    // The dry mix: 0 dB at 86, +6 at 100, none when muted.
    let mut bytes = project(16, &[1], 120.0);
    let dry = |b: &[u8]| ProjectFile::parse(b).unwrap().dry_mix_db();
    assert_eq!(dry(&bytes), Some(0.0));
    bytes[0x4DD] = 100;
    assert!((dry(&bytes).unwrap() - 6.0).abs() < 1e-9);
    bytes[0x4DC] = 1;
    assert_eq!(dry(&bytes), None);
    for (v, want) in [(1, -36.0), (85, -0.72), (86, 0.0), (87, 0.54), (100, 6.0)] {
        assert!((mix_db(v).unwrap() - want).abs() < 1e-9, "{v}");
    }
}

/// A pad with an envelope and a high-pass filter; a beat-sliced loop, one step louder than its
/// instrument; a sliced chop playing its selected slice.
#[test]
fn levels_envelopes_filters_and_slices_play_as_the_tracker_plays_them() {
    let card = one_pattern(
        6,
        &[
            (0, 0, note(60, 0)),
            (1, 0, note(24, 1)),
            (
                1,
                4,
                Step {
                    volume: 75,
                    ..note(26, 1)
                },
            ),
            (1, 8, note(40, 1)),
            (2, 0, note(60, 2)),
        ],
        15,
        &[1],
    );
    let mut pad = instrument(300, 1);
    pad[16 + 256] = 25; // -12.48 dB
    put_f32(&mut pad, 76, 1.0);
    put_u16(&mut pad, 76 + 6, 100);
    put_u16(&mut pad, 76 + 10, 250);
    put_f32(&mut pad, 76 + 12, 0.5);
    put_u16(&mut pad, 76 + 16, 400);
    pad[16 + 76 + 19] = 1;
    put_f32(&mut pad, 244, 0.5);
    put_f32(&mut pad, 248, 1.0);
    pad[16 + 252] = 1; // high pass
    pad[16 + 253] = 1;
    pad[16 + 370] = 12; // bits
    card.put("S", "instruments/1 Pad.pti", pad);
    let mut loop_ = instrument(300, 1);
    loop_[16 + 256] = 50; // 0 dB
    loop_[16 + 60] = 5; // beat slice
    loop_[16 + 360] = 3;
    for (i, at) in [0u16, 21845, 43690].into_iter().enumerate() {
        put_u16(&mut loop_, 264 + 2 * i, at);
    }
    card.put("S", "instruments/2 Loop.pti", loop_);
    let mut chop = instrument(300, 1);
    chop[16 + 256] = 50;
    chop[16 + 60] = 4; // slice
    chop[16 + 360] = 2;
    chop[16 + 361] = 1;
    put_u16(&mut chop, 264 + 2, 32768);
    card.put("S", "instruments/3 Chop.pti", chop);

    let found = card.found("S");
    let conversion = PolyendTracker
        .to_tutti(&card.store, &found, &mut Quiet)
        .unwrap();
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

    // The pad: its envelope in seconds, its filter as the firmware plays it, its level its
    // volume, the track's gain the mixer's +6 dB; the master full, velocity followed exactly.
    let pad = ends_with(&document.tracks, ": Pad");
    for (id, want) in [
        ("attack", 0.1),
        ("decay", 0.25),
        ("sustain", 0.5),
        ("release", 0.4),
        ("filtermode", 3.0),
        ("cutoff", 20.0 * 2f64.powf(238.0 / 50.0)),
        ("resonance", 1.0 / 4.3),
        ("master", 1.0),
        ("mod_amp_vel", 1.0),
        ("mod_amp_velon", 1.0),
        ("mod_cutoff_envon", 0.0),
        ("mod_cutoff_lfo2on", 0.0),
        ("converter", (4.0f64 / 11.0).powi(2)),
    ] {
        assert!(
            (param(pad, id) - want).abs() < 1e-4,
            "{id}: {}",
            param(pad, id)
        );
    }
    assert!((db(param(pad, "alevel")) + 12.48).abs() < 1e-6);
    assert!((db(pad.channel.gain) - 6.0).abs() < 1e-6);

    // The loop: each note its slice as an audio piece; the loud step (+12 dB) the loudest, so the
    // track's gain carries it and the other notes' velocities are their share of it.
    let loop_ = ends_with(&document.tracks, ": Loop");
    assert!(
        (db(loop_.channel.gain) - 18.0).abs() < 1e-6,
        "{}",
        db(loop_.channel.gain)
    );
    let mut notes: Vec<&tutti::Note> = loop_.patterns[0]
        .events
        .iter()
        .filter_map(|e| match e {
            Event::Note(n) => Some(n),
            _ => None,
        })
        .collect();
    notes.sort_by_key(|n| n.at);
    let pieces: Vec<(u64, u64, i64)> = notes
        .iter()
        .map(|n| {
            let a = n.audio.as_ref().expect("a slice");
            (a.from, a.to, a.map[0].tick)
        })
        .collect();
    assert_eq!(
        pieces,
        [
            (0, 100, 0),
            (200, 300, 4 * TICKS_PER_STEP),
            (200, 300, 8 * TICKS_PER_STEP)
        ]
    );
    let velocities: Vec<f64> = notes.iter().map(|n| db(n.velocity)).collect();
    assert!(
        (velocities[0] + 12.0).abs() < 1e-6 && velocities[1].abs() < 1e-9,
        "{velocities:?}"
    );
    let file = &notes[0].audio.as_ref().unwrap().file;
    assert!(out.files().contains_key(file.as_str()), "{file}");

    // The chop plays its selected slice, the second: from its start to the sample's end.
    let chop = ends_with(&document.tracks, ": Chop");
    assert!((param(chop, "astart") - 32768.0 / 65535.0).abs() < 1e-9);
    assert_eq!(param(chop, "aend"), 1.0);
    assert_eq!(param(chop, "converter"), 0.0, "16 bits: the converter off");
}

#[test]
fn instrument_files_are_placed_by_number_or_refused() {
    let card = one_pattern(6, &[(0, 0, note(60, 1))], 15, &[1]);
    card.put("S", "instruments/instrument_02.pti", instrument(10, 1));
    let (model, _) = read(&card.store, &card.found("S")).unwrap();
    assert!(
        model.instruments.contains_key(&1),
        "the device's own name, read"
    );
    card.put("S", "instruments/Kick.pti", instrument(10, 1));
    match read(&card.store, &card.found("S")) {
        Err(Error::Unreadable { reference, .. }) => assert!(reference.ends_with("Kick.pti")),
        other => panic!("{:?}", other.map(|_| ())),
    }
}
