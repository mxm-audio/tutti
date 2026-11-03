//! The song as the device plays it, worked out from the files: the song's order, each pattern's
//! played length, every note's tick, length, pitch and velocity, and the tempo changes. The rules
//! are the Tracker Community Edition firmware's sequencer (`mtSequencer.cpp`, read 2026-11-03):
//!
//! - A step lasts `15 / tempo` seconds, a sixteenth note: [`TICKS_PER_STEP`] at [`PPQ`]. It has
//!   48 microsteps; micro-move `m` (0 to 100) delays the step by `(m + 1) * 47 / 100` of them.
//! - Every track plays track 1's length (`last + 1` steps; the firmware's `switchRowStep` reads
//!   `track[0].length` for each, a track's own length mattering only in performance mode), and
//!   the pattern ends after it, or after a step holding Break Pattern (`x`).
//! - A track plays one note at a time: a note sounds until the track's next note, `OFF`, `CUT` or
//!   `FADE`, across patterns, or the song's end.
//! - Velocity and micro-move: an effect in the second slot, then the first, overrides the step's
//!   own value (version 6), as the sequencer applies them in that order.
//! - Patterns of version 4 or older clear tracks 9 to 16 and give them track 1's length; a file
//!   of 8 or 12 tracks gives the missing tracks track 1's length, empty.

use std::collections::BTreeMap;

use crate::files::{Fx, NOTE_CUT, NOTE_FADE, NOTE_OFF, PatternFile, Step};
use crate::model::Model;

/// Ticks per quarter note in the tutti project.
pub const PPQ: i64 = 960;
/// A step, a sixteenth note.
pub const TICKS_PER_STEP: i64 = PPQ / 4;
/// A step's 48 microsteps.
pub const TICKS_PER_MICROSTEP: i64 = TICKS_PER_STEP / 48;
/// Tracks the device plays: 8 sample tracks, then 8 MIDI tracks.
pub const TRACKS: usize = 16;
/// The tracks that play samples.
pub const SAMPLE_TRACKS: usize = 8;
/// Instruments 0 to 47 play samples; 48 to 63 are MIDI channels; 64 on, synth engines.
pub const SAMPLE_INSTRUMENTS: u8 = 48;
pub const MIDI_INSTRUMENTS_END: u8 = 64;
/// A pattern the song names but no file holds plays empty, this long (the firmware's default).
pub const DEFAULT_STEPS: u32 = 32;

/// The effect types the conversion plays (the firmware's `FX_TYPE_*`).
pub mod fx {
    pub const NONE: u8 = 0;
    pub const MICROMOVE: u8 = 2;
    pub const BREAK_PATTERN: u8 = 13;
    pub const TEMPO: u8 = 15;
    pub const VOLUME: u8 = 18;

    /// An effect's name, as the device shows it, for the report.
    pub fn name(kind: u8) -> String {
        const NAMES: &[&str] = &[
            "none",
            "Off (!)",
            "Micro-move (m)",
            "Roll (R)",
            "Chance (C)",
            "Random Note (n)",
            "Random Instrument (i)",
            "Random Volume (v)",
            "MIDI CC A (a)",
            "MIDI CC B (b)",
            "MIDI CC C (c)",
            "MIDI CC D (d)",
            "MIDI CC E (e)",
            "Break Pattern (x)",
            "MIDI Chord (0)",
            "Tempo (T)",
            "Random FX Value (x)",
            "Swing (I)",
            "Volume/Velocity (V)",
            "Glide (G)",
            "Gate Length (q)",
            "Arp (A)",
            "Position (p)",
            "Volume LFO (g)",
            "Panning LFO (h)",
            "Slice (S)",
            "Reverse Playback (r)",
            "Low-pass (L)",
            "High-pass (H)",
            "Band-pass (B)",
            "Delay Send (s)",
            "Panning (P)",
            "Reverb Send (t)",
            "Finetune LFO (l)",
            "Micro-tune/Pitchbend (M)",
            "Filter LFO (j)",
            "Position LFO (k)",
            "MIDI CC F (f)",
            "Overdrive (D)",
            "Bit Depth (E)",
            "Tune (U)",
            "Slide Up (F)",
            "Slide Down (J)",
        ];
        NAMES
            .get(kind as usize)
            .map(|n| (*n).to_owned())
            .unwrap_or_else(|| format!("effect {kind}"))
    }
}

/// How a note ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum End {
    /// The track's next note.
    Next,
    Off,
    Cut,
    Fade,
    /// The song's end.
    SongEnd,
}

impl End {
    pub fn as_str(self) -> &'static str {
        match self {
            End::Next => "next",
            End::Off => "off",
            End::Cut => "cut",
            End::Fade => "fade",
            End::SongEnd => "song_end",
        }
    }
}

/// A slot of the song: a pattern, where it starts, how many steps it plays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub pattern: u8,
    pub at: i64,
    pub steps: u32,
}

/// A note as the device plays it.
#[derive(Clone, Debug, PartialEq)]
pub struct Played {
    /// The Polyend track, 0 to 15.
    pub track: usize,
    pub instrument: u8,
    pub pitch: u8,
    pub velocity: f64,
    /// Absolute ticks.
    pub at: i64,
    pub length: i64,
    pub end: End,
    /// The slot it starts in, the step of the pattern that played it (0 on) and the step of the
    /// track it came from.
    pub slot: usize,
    pub global_step: u32,
    pub step: u8,
}

/// The song as the device plays it.
#[derive(Clone, Debug, Default)]
pub struct Song {
    pub slots: Vec<Slot>,
    pub notes: Vec<Played>,
    /// (tick, bpm), the first at 0.
    pub tempo: Vec<(i64, f64)>,
    /// Where the song ends.
    pub end: i64,
    /// Patterns the song names that no file holds.
    pub missing: Vec<u8>,
    /// How many played steps carry each effect the conversion does not play, by type.
    pub unplayed_fx: BTreeMap<u8, u32>,
    /// Steps without a note that set a volume on a sounding note (not played).
    pub volume_changes: u32,
}

/// A pattern's 16 tracks as the device plays them: (last step, steps), every one track 1's
/// length.
fn played_tracks(file: &PatternFile) -> Vec<(u8, Vec<Step>)> {
    let first_last = file
        .tracks
        .first()
        .map_or(DEFAULT_STEPS as u8 - 1, |t| t.last);
    let version = file.header.version();
    (0..TRACKS)
        .map(|t| match file.tracks.get(t) {
            Some(track) if !(version <= 4 && t >= SAMPLE_TRACKS) => {
                (first_last, track.steps.clone())
            }
            _ => (first_last, vec![Step::EMPTY; crate::files::STEPS]),
        })
        .collect()
}

/// The step a track plays at a pattern's global step.
fn step_at(tracks: &[(u8, Vec<Step>)], track: usize, global: u32) -> (u8, Step) {
    let (last, steps) = &tracks[track];
    let pos = (global % (*last as u32 + 1)) as u8;
    (pos, steps[pos as usize])
}

/// The value an effect type gives a step, if any: the second slot, then the first, override.
fn effect(step: &Step, kind: u8) -> Option<u8> {
    let found = |f: &Fx| (f.kind == kind).then_some(f.value);
    found(&step.fx[0]).or_else(|| found(&step.fx[1]))
}

/// How many steps a pattern plays.
fn played_steps(tracks: &[(u8, Vec<Step>)]) -> u32 {
    let full = tracks[TRACKS - 1].0 as u32 + 1;
    for global in 0..full {
        if (0..TRACKS).any(|t| effect(&step_at(tracks, t, global).1, fx::BREAK_PATTERN).is_some()) {
            return global + 1;
        }
    }
    full
}

/// The delay micro-move `m` gives a step, in ticks.
pub fn micromove_ticks(m: u8) -> i64 {
    ((m.min(100) as i64 + 1) * 47 / 100) * TICKS_PER_MICROSTEP
}

pub fn song(model: &Model) -> Song {
    let mut song = Song {
        tempo: vec![(0, model.project.tempo() as f64)],
        ..Song::default()
    };
    let empty = PatternFile {
        header: model.project.header.clone(),
        reserved: [0; 12],
        tracks: Vec::new(),
        crc: None,
    };
    // The note each track is sounding: its index in `song.notes`.
    let mut open: [Option<usize>; TRACKS] = [None; TRACKS];
    let mut at = 0i64;
    for (slot_index, &number) in model.project.playlist().iter().enumerate() {
        let file = match model.patterns.get(&number) {
            Some(file) => file,
            None => {
                if !song.missing.contains(&number) {
                    song.missing.push(number);
                }
                &empty
            }
        };
        let tracks = played_tracks(file);
        let steps = if file.tracks.is_empty() {
            DEFAULT_STEPS
        } else {
            played_steps(&tracks)
        };
        for global in 0..steps {
            let step_tick = at + global as i64 * TICKS_PER_STEP;
            for (t, sounding) in open.iter_mut().enumerate() {
                let (pos, step) = step_at(&tracks, t, global);
                let micromove = effect(&step, fx::MICROMOVE)
                    .map(|m| m as i64)
                    .or_else(|| (step.micromove >= 0).then_some(step.micromove as i64));
                let tick = step_tick + micromove.map_or(0, |m| micromove_ticks(m as u8));
                if let Some(bpm) = effect(&step, fx::TEMPO) {
                    song.tempo.push((step_tick, bpm as f64 * 2.0));
                }
                for f in &step.fx {
                    if ![
                        fx::NONE,
                        fx::MICROMOVE,
                        fx::BREAK_PATTERN,
                        fx::TEMPO,
                        fx::VOLUME,
                    ]
                    .contains(&f.kind)
                    {
                        *song.unplayed_fx.entry(f.kind).or_default() += 1;
                    }
                }
                let end = match step.note {
                    n if n >= 0 => Some(End::Next),
                    NOTE_OFF => Some(End::Off),
                    NOTE_CUT => Some(End::Cut),
                    NOTE_FADE => Some(End::Fade),
                    _ => None,
                };
                let Some(end) = end else {
                    if step.volume >= 0 || effect(&step, fx::VOLUME).is_some() {
                        song.volume_changes += 1;
                    }
                    continue;
                };
                if let Some(i) = sounding.take() {
                    let note = &mut song.notes[i];
                    note.length = (tick - note.at).max(1);
                    note.end = end;
                }
                if step.note >= 0 {
                    let volume = effect(&step, fx::VOLUME)
                        .map(|v| v as i64)
                        .or_else(|| (step.volume >= 0).then_some(step.volume as i64))
                        .unwrap_or(100);
                    song.notes.push(Played {
                        track: t,
                        instrument: step.instrument,
                        pitch: step.note as u8,
                        velocity: (volume as f64 / 100.0).clamp(0.0, 1.0),
                        at: tick,
                        length: 0,
                        end: End::SongEnd,
                        slot: slot_index,
                        global_step: global,
                        step: pos,
                    });
                    *sounding = Some(song.notes.len() - 1);
                }
            }
        }
        song.slots.push(Slot {
            pattern: number,
            at,
            steps,
        });
        at += steps as i64 * TICKS_PER_STEP;
    }
    song.end = at;
    for i in open.into_iter().flatten() {
        let note = &mut song.notes[i];
        note.length = (at - note.at).max(1);
        note.end = End::SongEnd;
    }
    // One tempo per tick, the last set winning; none repeating the one before.
    song.tempo.sort_by_key(|&(tick, _)| tick);
    let mut tempo: Vec<(i64, f64)> = Vec::new();
    for (tick, bpm) in song.tempo.drain(..) {
        match tempo.last_mut() {
            Some(last) if last.0 == tick => last.1 = bpm,
            Some(last) if last.1 == bpm => {}
            _ => tempo.push((tick, bpm)),
        }
    }
    song.tempo = tempo;
    song
}
