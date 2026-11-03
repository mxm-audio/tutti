//! The Tracker's files, byte for byte: each parsed into what a conversion reads, every other byte
//! kept as it was, and written back exactly as read (`to_bytes`), which the tests hold for every
//! file. The layouts are the Tracker Community Edition firmware's (`polyend/tracker-ce`, read
//! 2026-11-03), checked against the files of a Tracker Mini's card.
//!
//! Every file starts with the same 16 bytes ([`Header`]): two letters (`MT` a project, `PM` or
//! `KS` a pattern, `TI` an instrument), a type, the firmware's version, the file's structure
//! version four times, its size, and two bytes of padding.

use serde::{Deserialize, Serialize};

/// The project file versions read, each carried by a fixture: 7, 11 and 13 (the factory Artists
/// projects of the Tracker's earlier firmware), 16 (the Mini's 2.x), 17 (CE 1.0.0). The fields a
/// conversion reads sit where they do in every one (the firmware reads an older file into its
/// newest struct); a project before 10 has its pad root note reset to 24. Every other refused.
pub const PROJECT_VERSIONS: &[u8] = &[7, 11, 13, 16, 17];
/// The pattern file versions read: 1 to 5 (six-byte steps), 6 (CE 1.0.0, a step's own volume
/// and micro-move). Versions 1 to 3 are played through the firmware's translations
/// ([`crate::song`]).
pub const PATTERN_VERSIONS: &[u8] = &[1, 2, 3, 4, 5, 6];
/// The instrument file versions read: 1, 3, 6, 7 and 8 (the older `.mti` files), 9 (`.pti`). Before
/// version 8 the struct is four bytes shorter; version 1 numbers its play modes from 5 on one
/// lower; volumes before 9 are converted as the firmware converts them.
pub const INSTRUMENT_VERSIONS: &[u8] = &[1, 3, 6, 7, 8, 9];

/// Steps in every track of a pattern file, whatever its length.
pub const STEPS: usize = 128;
/// The size of the instrument struct; before version 8 it may also be four bytes less.
pub const INSTRUMENT_STRUCT: usize = 372;
pub const INSTRUMENT_STRUCT_BEFORE_8: usize = 368;
/// Where a version 9 instrument's samples start: the header, the struct and its CRC.
pub const PCM_OFFSET: usize = 16 + INSTRUMENT_STRUCT + 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable(pub String);

fn bad(why: impl Into<String>) -> Unreadable {
    Unreadable(why.into())
}

/// The 16 bytes every file starts with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub id: [u8; 2],
    pub kind: u16,
    pub firmware: [u8; 4],
    pub version: [u8; 4],
    pub size: u16,
    pub padding: [u8; 2],
}

impl Header {
    pub const LEN: usize = 16;

    pub fn parse(bytes: &[u8]) -> Result<Header, Unreadable> {
        if bytes.len() < Self::LEN {
            return Err(bad("shorter than a header"));
        }
        Ok(Header {
            id: [bytes[0], bytes[1]],
            kind: u16::from_le_bytes([bytes[2], bytes[3]]),
            firmware: [bytes[4], bytes[5], bytes[6], bytes[7]],
            version: [bytes[8], bytes[9], bytes[10], bytes[11]],
            size: u16::from_le_bytes([bytes[12], bytes[13]]),
            padding: [bytes[14], bytes[15]],
        })
    }

    pub fn to_bytes(&self) -> [u8; 16] {
        let k = self.kind.to_le_bytes();
        let s = self.size.to_le_bytes();
        [
            self.id[0],
            self.id[1],
            k[0],
            k[1],
            self.firmware[0],
            self.firmware[1],
            self.firmware[2],
            self.firmware[3],
            self.version[0],
            self.version[1],
            self.version[2],
            self.version[3],
            s[0],
            s[1],
            self.padding[0],
            self.padding[1],
        ]
    }

    /// The structure version: the first of its four bytes, as the firmware reads it.
    pub fn version(&self) -> u8 {
        self.version[0]
    }

    fn known(&self, what: &str, versions: &[u8]) -> Result<(), Unreadable> {
        if versions.contains(&self.version()) {
            Ok(())
        } else {
            Err(bad(format!(
                "{what} version {}, which this converter does not read (it reads {versions:?})",
                self.version()
            )))
        }
    }
}

// ---------------------------------------------------------------------------------------------
// project.mt

/// `project.mt`: the song's order, the tempo, the pad board's root note, the tracks' names,
/// mutes and volumes, and much else, kept as read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectFile {
    pub header: Header,
    /// Everything after the header, its CRC included.
    pub body: Vec<u8>,
}

/// Offsets in the file (the firmware's `strMtProjectRemote` after the header).
mod project_at {
    pub const PLAYLIST: usize = 0x10;
    pub const PLAYLIST_LEN: usize = 255;
    pub const ROOT_NOTE: usize = 0x117;
    pub const TRACK_MUTE: usize = 0x12C;
    pub const TEMPO: usize = 0x1C0;
    pub const TRACK_NAMES: usize = 0x428;
    pub const TRACK_NAME_LEN: usize = 21;
    pub const TRACK_VOLUME: usize = 0x4D0;
    /// The end of the last field read.
    pub const END: usize = TRACK_VOLUME + 8;
}

impl ProjectFile {
    pub fn parse(bytes: &[u8]) -> Result<ProjectFile, Unreadable> {
        let header = Header::parse(bytes)?;
        if &header.id != b"MT" {
            return Err(bad("not a project file (it does not start with MT)"));
        }
        header.known("project file", PROJECT_VERSIONS)?;
        if bytes.len() < project_at::END {
            return Err(bad(format!(
                "{} bytes, too short for a project",
                bytes.len()
            )));
        }
        Ok(ProjectFile {
            header,
            body: bytes[Header::LEN..].to_vec(),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.header.to_bytes().to_vec();
        out.extend_from_slice(&self.body);
        out
    }

    fn at(&self, offset: usize) -> &[u8] {
        &self.body[offset - Header::LEN..]
    }

    /// The song: pattern numbers in play order, up to the first 0.
    pub fn playlist(&self) -> Vec<u8> {
        self.at(project_at::PLAYLIST)[..project_at::PLAYLIST_LEN]
            .iter()
            .copied()
            .take_while(|&p| p != 0)
            .collect()
    }

    /// The tempo in bpm.
    pub fn tempo(&self) -> f32 {
        let b = self.at(project_at::TEMPO);
        f32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }

    /// The pad board's root note: the firmware resets it to 24 in a project before version 10.
    pub fn root_note(&self) -> u8 {
        if self.header.version() < 10 {
            24
        } else {
            self.at(project_at::ROOT_NOTE)[0]
        }
    }

    /// Whether sample track `t` (0 to 7) is muted.
    pub fn track_muted(&self, t: usize) -> bool {
        self.at(project_at::TRACK_MUTE)[t] != 0
    }

    /// Sample track `t`'s volume, 0 to 100, as stored.
    pub fn track_volume(&self, t: usize) -> u8 {
        self.at(project_at::TRACK_VOLUME)[t]
    }

    /// Sample track `t`'s volume as the mixer plays it, in decibels ([`mix_db`]), a project up to
    /// version 11 converted first as the firmware converts it; none for silence.
    pub fn track_volume_db(&self, t: usize) -> Option<f64> {
        let mut v = self.track_volume(t);
        if self.header.version() <= 11 {
            v = mixer_1300_to_1302(v);
        }
        mix_db(v)
    }

    /// Sample track `t`'s name, up to its first NUL.
    pub fn track_name(&self, t: usize) -> String {
        let start = project_at::TRACK_NAMES + t * project_at::TRACK_NAME_LEN;
        let raw = &self.at(start)[..project_at::TRACK_NAME_LEN];
        c_string(raw)
    }
}

pub(crate) fn c_string(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).trim().to_owned()
}

// ---------------------------------------------------------------------------------------------
// pattern_NN.mtp

/// A step's note: a note number, or one of these.
pub const NOTE_EMPTY: i8 = -1;
pub const NOTE_FADE: i8 = -2;
pub const NOTE_CUT: i8 = -3;
pub const NOTE_OFF: i8 = -4;

/// One of a step's two effects: its type (the firmware's `FX_TYPE_*`, 0 none) and value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fx {
    pub kind: u8,
    pub value: u8,
}

/// One step of a track, as stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub note: i8,
    pub instrument: u8,
    pub fx: [Fx; 2],
    /// The step's own volume, 0 to 100, or -1 for none: version 6 only (older files: -1).
    pub volume: i8,
    /// The step's own micro-move, 0 to 100, or -1 for none: version 6 only (older files: -1).
    pub micromove: i8,
}

impl Step {
    /// A step as the firmware leaves an empty one.
    pub const EMPTY: Step = Step {
        note: NOTE_EMPTY,
        instrument: 0,
        fx: [Fx { kind: 0, value: 0 }; 2],
        volume: -1,
        micromove: -1,
    };
}

/// One track of a pattern: its length (the last step played, counting from 0) and its 128 steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    pub last: u8,
    pub steps: Vec<Step>,
}

/// A pattern file: 8, 12 or 16 tracks of 128 steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternFile {
    pub header: Header,
    /// The firmware's unused tempo, swing and four bytes, kept as read.
    pub reserved: [u8; 12],
    pub tracks: Vec<Track>,
    /// Absent in some files of version 5 or older, which ended before it.
    pub crc: Option<[u8; 4]>,
}

impl PatternFile {
    fn step_len(version: u8) -> usize {
        if version >= 6 { 8 } else { 6 }
    }

    pub fn parse(bytes: &[u8]) -> Result<PatternFile, Unreadable> {
        let header = Header::parse(bytes)?;
        if &header.id != b"PM" && &header.id != b"KS" {
            return Err(bad("not a pattern file (it does not start with PM or KS)"));
        }
        header.known("pattern file", PATTERN_VERSIONS)?;
        let version = header.version();
        let step_len = Self::step_len(version);
        let track_len = 1 + STEPS * step_len;
        let fixed = Header::LEN + 12;
        let declared = header.size as usize;
        let crc_present = if bytes.len() == declared {
            true
        } else if version <= 5 && bytes.len() + 4 == declared {
            false
        } else {
            return Err(bad(format!(
                "{} bytes where its header says {declared}",
                bytes.len()
            )));
        };
        let body = declared - fixed - 4;
        if !body.is_multiple_of(track_len) || ![8, 12, 16].contains(&(body / track_len)) {
            return Err(bad(format!("{declared} bytes is no known pattern size")));
        }
        let mut reserved = [0u8; 12];
        reserved.copy_from_slice(&bytes[Header::LEN..fixed]);
        let mut at = fixed;
        let mut tracks = Vec::new();
        for _ in 0..body / track_len {
            let last = bytes[at];
            at += 1;
            let mut steps = Vec::with_capacity(STEPS);
            for _ in 0..STEPS {
                let s = &bytes[at..at + step_len];
                steps.push(Step {
                    note: s[0] as i8,
                    instrument: s[1],
                    fx: [
                        Fx {
                            kind: s[2],
                            value: s[3],
                        },
                        Fx {
                            kind: s[4],
                            value: s[5],
                        },
                    ],
                    volume: if step_len == 8 { s[6] as i8 } else { -1 },
                    micromove: if step_len == 8 { s[7] as i8 } else { -1 },
                });
                at += step_len;
            }
            tracks.push(Track { last, steps });
        }
        let crc = crc_present.then(|| [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
        Ok(PatternFile {
            header,
            reserved,
            tracks,
            crc,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, Unreadable> {
        let step_len = Self::step_len(self.header.version());
        let mut out = self.header.to_bytes().to_vec();
        out.extend_from_slice(&self.reserved);
        for track in &self.tracks {
            out.push(track.last);
            for s in &track.steps {
                out.extend_from_slice(&[
                    s.note as u8,
                    s.instrument,
                    s.fx[0].kind,
                    s.fx[0].value,
                    s.fx[1].kind,
                    s.fx[1].value,
                ]);
                if step_len == 8 {
                    out.extend_from_slice(&[s.volume as u8, s.micromove as u8]);
                } else if s.volume != -1 || s.micromove != -1 {
                    return Err(bad(
                        "a step's own volume or micro-move in a pattern of version 5 or older",
                    ));
                }
            }
        }
        if let Some(crc) = self.crc {
            out.extend_from_slice(&crc);
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------------------------
// instruments/N name.pti

/// How an instrument plays its sample (the firmware's `instrumentPlayMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayMode {
    OneShot,
    LoopForward,
    LoopBackward,
    LoopPingPong,
    Slice,
    BeatSlice,
    Wavetable,
    Granular,
    Other(u8),
}

impl PlayMode {
    pub fn name(self) -> String {
        match self {
            PlayMode::OneShot => "one-shot".into(),
            PlayMode::LoopForward => "forward loop".into(),
            PlayMode::LoopBackward => "backward loop".into(),
            PlayMode::LoopPingPong => "ping-pong loop".into(),
            PlayMode::Slice => "slice".into(),
            PlayMode::BeatSlice => "beat slice".into(),
            PlayMode::Wavetable => "wavetable".into(),
            PlayMode::Granular => "granular".into(),
            PlayMode::Other(n) => format!("play mode {n}"),
        }
    }
}

/// An instrument file's header, struct and CRC: everything before its samples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstrumentFile {
    pub header: Header,
    /// The firmware's `strInstrument`, as read.
    pub data: Vec<u8>,
    pub crc: [u8; 4],
}

/// Offsets in the instrument struct (the firmware's `strInstrument`, 32-bit alignment).
mod instrument_at {
    pub const SAMPLE_NAME: usize = 5;
    pub const SAMPLE_LENGTH: usize = 44;
    pub const PLAY_MODE: usize = 60;
    pub const START: usize = 62;
    pub const LOOP1: usize = 64;
    pub const LOOP2: usize = 66;
    pub const END: usize = 68;
    pub const AMP_ENVELOPE: usize = 76;
    pub const CUTOFF: usize = 244;
    pub const RESONANCE: usize = 248;
    pub const FILTER_TYPE: usize = 252;
    pub const FILTER_ENABLE: usize = 253;
    pub const TUNE: usize = 254;
    pub const FINETUNE: usize = 255;
    pub const VOLUME: usize = 256;
    pub const PANNING: usize = 260;
    pub const SLICES: usize = 264;
    pub const SLICE_COUNT: usize = 360;
    pub const SELECTED_SLICE: usize = 361;
    pub const SLICE_PLAY_THRU: usize = 263;
}

impl InstrumentFile {
    /// An instrument file's first bytes, up to its samples (all of a `.mti`), and the file's
    /// whole length.
    pub fn parse(head: &[u8], file_len: u64) -> Result<InstrumentFile, Unreadable> {
        let header = Header::parse(head)?;
        if &header.id != b"TI" || header.kind != 1 {
            return Err(bad("not an instrument file (it does not start with TI)"));
        }
        header.known("instrument file", INSTRUMENT_VERSIONS)?;
        if header.version.iter().any(|&v| v != header.version[0]) {
            return Err(bad("an instrument whose four version bytes differ"));
        }
        // The full struct at any version; the shorter one only before version 8 (the firmware's
        // `InstrumentFileValidation`).
        let size = header.size as usize;
        let legacy = header.version() < 8 && size == INSTRUMENT_STRUCT_BEFORE_8;
        if size != INSTRUMENT_STRUCT && !legacy {
            return Err(bad(format!(
                "an instrument struct of {size} bytes in version {}",
                header.version()
            )));
        }
        let end = Header::LEN + size + 4;
        if (file_len as usize) < end || head.len() < end {
            return Err(bad("shorter than an instrument"));
        }
        let mut crc = [0u8; 4];
        crc.copy_from_slice(&head[end - 4..end]);
        Ok(InstrumentFile {
            header,
            data: head[Header::LEN..end - 4].to_vec(),
            crc,
        })
    }

    /// Everything before the samples.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.header.to_bytes().to_vec();
        out.extend_from_slice(&self.data);
        out.extend_from_slice(&self.crc);
        out
    }

    fn u16_at(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.data[at], self.data[at + 1]])
    }

    /// The sample's length in frames, as the struct says.
    pub fn sample_length(&self) -> u32 {
        let d = &self.data[instrument_at::SAMPLE_LENGTH..];
        u32::from_le_bytes([d[0], d[1], d[2], d[3]])
    }

    pub fn sample_name(&self) -> String {
        c_string(&self.data[instrument_at::SAMPLE_NAME..instrument_at::SAMPLE_NAME + 32])
    }

    /// The length of everything before the samples: where a `.pti`'s samples start.
    pub fn len(&self) -> usize {
        Header::LEN + self.data.len() + 4
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// The play mode, as the firmware reads it (version 1's from beat slice on one lower).
    pub fn play_mode(&self) -> PlayMode {
        let raw = self.data[instrument_at::PLAY_MODE];
        let raw = if self.header.version() == 1 && raw > 4 {
            raw + 1
        } else {
            raw
        };
        match raw {
            0 => PlayMode::OneShot,
            1 => PlayMode::LoopForward,
            2 => PlayMode::LoopBackward,
            3 => PlayMode::LoopPingPong,
            4 => PlayMode::Slice,
            5 => PlayMode::BeatSlice,
            6 => PlayMode::Wavetable,
            7 => PlayMode::Granular,
            n => PlayMode::Other(n),
        }
    }

    /// Start, the two loop points and end, each 0 to 65535 along the sample.
    pub fn points(&self) -> [u16; 4] {
        [
            self.u16_at(instrument_at::START),
            self.u16_at(instrument_at::LOOP1),
            self.u16_at(instrument_at::LOOP2),
            self.u16_at(instrument_at::END),
        ]
    }

    fn f32_at(&self, at: usize) -> f32 {
        f32::from_le_bytes([
            self.data[at],
            self.data[at + 1],
            self.data[at + 2],
            self.data[at + 3],
        ])
    }

    /// The amplitude envelope (the firmware's `envelope[envAmp]`), in milliseconds.
    pub fn amp_envelope(&self) -> Envelope {
        let at = instrument_at::AMP_ENVELOPE;
        Envelope {
            amount: self.f32_at(at),
            delay: self.u16_at(at + 4),
            attack: self.u16_at(at + 6),
            hold: self.u16_at(at + 8),
            decay: self.u16_at(at + 10),
            sustain: self.f32_at(at + 12),
            release: self.u16_at(at + 16),
            mode: self.data[at + 18],
            enabled: self.data[at + 19] != 0,
        }
    }

    /// The filter, if on: its type and, as the firmware plays them, its cutoff in hertz
    /// (`20 * 2^(i / 50)` for the table index `i` the 0..=1 value falls on, held to 20 Hz and
    /// 44 100 / 2.5) and its Q (the stored resonance plus 0.7, held to 0.7..=5).
    pub fn filter(&self) -> Option<Filter> {
        if self.data[instrument_at::FILTER_ENABLE] == 0 {
            return None;
        }
        let cutoff = self.f32_at(instrument_at::CUTOFF).clamp(0.0, 1.0);
        let index = (cutoff * 477.0) as i32;
        let hz = (20.0f64 * 2f64.powf(index as f64 / 50.0)).clamp(20.0, 44_100.0 / 2.5);
        let q = (self.f32_at(instrument_at::RESONANCE) as f64 + 0.7).clamp(0.7, 5.0);
        let kind = match self.data[instrument_at::FILTER_TYPE] {
            0 => FilterType::LowPass,
            1 => FilterType::HighPass,
            _ => FilterType::BandPass,
        };
        Some(Filter { kind, hz, q })
    }

    /// Tune in semitones and finetune in cents.
    pub fn tune(&self) -> (i8, i8) {
        (
            self.data[instrument_at::TUNE] as i8,
            self.data[instrument_at::FINETUNE] as i8,
        )
    }

    /// Volume, 0 to 100, as the firmware reads it: converted from versions 4 and 8's scales.
    pub fn volume(&self) -> u8 {
        let mut v = self.data[instrument_at::VOLUME];
        if self.header.version() <= 4 {
            v = volume_1117_to_1118(v);
        }
        if self.header.version() <= 8 {
            v = volume_1300_to_1302(v);
        }
        v
    }

    /// The volume as gain in decibels: 1 to 100 is -12 to +12 dB, 0 silence.
    pub fn volume_db(&self) -> Option<f64> {
        amp_db(self.volume())
    }

    /// Panning, 0 (left) to 100 (right), 50 the middle.
    pub fn panning(&self) -> i16 {
        i16::from_le_bytes([
            self.data[instrument_at::PANNING],
            self.data[instrument_at::PANNING + 1],
        ])
    }

    /// The slice a slice instrument plays.
    pub fn selected_slice(&self) -> usize {
        self.data[instrument_at::SELECTED_SLICE] as usize
    }

    /// Whether a slice plays on through the slices after it, to the sample's end.
    pub fn slice_play_thru(&self) -> bool {
        self.data[instrument_at::SLICE_PLAY_THRU] != 0
    }

    /// The span slice `i` plays, as 0..=65535 along the sample: from its start to the next
    /// slice's, or to the end when it is the last or plays through (the firmware's
    /// `playSlice`/`playBeatSlice`); the whole sample when there are none.
    pub fn slice_span(&self, i: usize) -> (u16, u16) {
        let slices = self.slices();
        if slices.is_empty() {
            return (0, u16::MAX);
        }
        let i = i.min(slices.len() - 1);
        let end = if self.slice_play_thru() || i + 1 == slices.len() {
            u16::MAX
        } else {
            slices[i + 1]
        };
        (slices[i], end)
    }

    /// The slices' starts, 0 to 65535 along the sample, as many as are set.
    pub fn slices(&self) -> Vec<u16> {
        let n = (self.data[instrument_at::SLICE_COUNT] as usize).min(48);
        (0..n)
            .map(|i| self.u16_at(instrument_at::SLICES + 2 * i))
            .collect()
    }
}

/// The firmware's `ConvertOldFileVersionVolume::convertVolume`: a 1-to-100 value on one dB range
/// to the same level on another, rounded, 0 kept.
fn convert_volume(v: u8, old: (f32, f32), new: (f32, f32)) -> u8 {
    if v == 0 {
        return 0;
    }
    // In f32, as the firmware computes it, so a value at a rounding edge lands where it does there.
    let map32 = |x: f32, a: f32, b: f32, c: f32, d: f32| (x - a) * (d - c) / (b - a) + c;
    let db = map32(v as f32, 1.0, 100.0, old.0, old.1);
    let mut r = map32(db, new.0, new.1, 1.0, 100.0);
    if r > 0.0 {
        r += 0.5;
    }
    r.clamp(1.0, 100.0) as u8
}

/// An instrument's volume, or a volume effect's value, from firmware 1.1.17's scale to 1.1.18's.
pub fn volume_1117_to_1118(v: u8) -> u8 {
    convert_volume(v, (-19.8, 0.0), (-24.0, 24.0))
}

/// The same, from 1.3.0's scale to 1.3.2's.
pub fn volume_1300_to_1302(v: u8) -> u8 {
    convert_volume(v, (-24.0, 24.0), (-12.0, 12.0))
}

/// An instrument's envelope: times in milliseconds; `mode` 0 an ADSR, others an LFO or a
/// wavetable envelope.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Envelope {
    pub amount: f32,
    pub delay: u16,
    pub attack: u16,
    pub hold: u16,
    pub decay: u16,
    pub sustain: f32,
    pub release: u16,
    pub mode: u8,
    pub enabled: bool,
}

impl Envelope {
    /// An envelope that shapes the amplitude as an ADSR does.
    pub fn is_adsr(&self) -> bool {
        self.enabled && self.mode == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterType {
    LowPass,
    HighPass,
    BandPass,
}

/// A filter as the firmware plays it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Filter {
    pub kind: FilterType,
    pub hz: f64,
    pub q: f64,
}

/// The gain in decibels an instrument's or a note's volume plays at, 0 to 100 (the firmware's
/// `ampLogValues`): 0.48 dB a step, 0 dB at 50, -24 at 1 and +24 at 100; none at 0, silence.
/// The firmware's own figures are ten times a logarithm, so it shows half these.
pub fn amp_db(v: u8) -> Option<f64> {
    let v = v.min(100) as f64;
    match v {
        0.0 => None,
        _ if v < 50.0 => Some(0.48 * (v - 50.0) - 0.48),
        _ => Some(0.48 * (v - 50.0)),
    }
}

/// The gain in decibels a track's volume plays at, 0 to 100 (the firmware's `mixAmpLogValues`):
/// 0.42 dB a step, -36 at 1, 0 dB at 86, +6 at 100; none at 0, silence.
pub fn mix_db(v: u8) -> Option<f64> {
    let v = v.min(100) as f64;
    match v {
        0.0 => None,
        _ if v < 86.0 => Some(-36.0 + 0.42 * (v - 1.0)),
        86.0 => Some(0.0),
        _ => Some(6.0 - 0.42 * (100.0 - v)),
    }
}

/// A track's volume from firmware 1.3.0's mixer scale to 1.3.2's.
pub fn mixer_1300_to_1302(v: u8) -> u8 {
    convert_volume(v, (-36.0, 6.0), (-18.0, 3.0))
}
