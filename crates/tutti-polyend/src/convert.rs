//! A Polyend Tracker project into tutti, and back to the model.
//!
//! - A Polyend track playing one instrument is that instrument's track; one playing several is a
//!   folder holding a track per instrument (a step names its instrument; a tutti track has one
//!   instrument). Instruments the song never plays sit, each on its own track, in a muted folder,
//!   so nothing is left out.
//! - The song's order is parts: a part per slot on each instrument track that plays in it. A
//!   pattern's notes are worked out per slot ([`crate::song`]); slots of one pattern whose notes
//!   are identical share one tutti pattern, and the others get variants (`P03 b`), so a pattern
//!   repeated with a different successor keeps the note that rings into it.
//! - A note outlasting its pattern keeps its length, past the pattern's, and its part is
//!   lengthened to sound it: the part plays a note for as long as the part lasts.
//! - A sample instrument is an MXM Creative Sampler device, its sample a WAV in the device's own
//!   files (tutti 7.8): a `.pti`'s samples written as one, an older instrument's WAV as it is. What
//!   maps of its settings is the device's parameter snapshot ([`sampler_params`]); all of them are
//!   kept under the extension. A slice instrument plays its selected slice; a beat-slice
//!   instrument's notes play their slices as audio pieces of its sample (tutti 6.7).
//! - Volumes play as the firmware's tables play them ([`crate::files::amp_db`], `mix_db`): a
//!   Polyend track's volume and the dry mix's are its track's gain, and a note sounds at its step's
//!   volume, or else its
//!   instrument's, split into the sampler's level, the note's velocity and the instrument track's
//!   gain ([`Levels`]), so a boost past unity sounds too.
//! - LFOs: the first two on volume, cutoff or finetune are the sampler's two ([`lfos`]); a pan
//!   LFO is the track's pan automation, restarted with each note ([`pan_lfo`]).
//! - The reverb and delay sends go to returns of MXM Classic Verb and MXM FX Delay
//!   ([`reverb_device`], [`delay_device`]), their settings as CLAP values.
//! - Everything tutti does not hold is kept under [`NAMESPACE`] on the object it belongs to:
//!   the project file and every step no note stands for, on the document; a note's step, on the
//!   note; an instrument's header, on its device. [`to_model`] reads the model back from them,
//!   byte for byte.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use tutti::render::sha256_hex;
use tutti::{
    Device, Document, Event, Generator, Id, Lane, Note, Part, Pattern, TempoPoint, Track, Unknown,
    Zone,
};
use tutti_convert::{
    Contents, Conversion, Error, Found, Place, Progress, Put, Report, Source, derived_id,
};

use crate::files::{
    Fx, Header, InstrumentFile, PatternFile, ProjectFile, STEPS, Step, Track as PolyendTrack,
};
use crate::model::{
    Instrument, Layout, Locator, Locators, Model, Pcm, SAMPLE_RATE, interleave, read, samples,
};
use crate::song::{
    self, MIDI_INSTRUMENTS_END, PPQ, Played, SAMPLE_INSTRUMENTS, SAMPLE_TRACKS, Song,
    TICKS_PER_STEP, fx,
};

/// The extension's namespace (tutti 8.1), registered in tutti's `spec/1.0/extensions.md`.
pub const NAMESPACE: &str = "dk.mxm.polyend-tracker";
/// The conversion's version: raised whenever what it writes changes, so earlier outputs are out
/// of date.
pub const VERSION: u32 = 10;
/// The slowest tempo written: the slowest most programs take.
pub const SLOWEST_BPM: f64 = 20.0;
/// The most gain a channel is written with, +12 dB: the most most programs take.
pub const LOUDEST_GAIN: f64 = 3.981_071_705_534_972; // 10^(12/20), rounded down

/// The reverb and delay the Tracker's sends go to.
pub const REVERB: &str = "clap:dk.mxm.mxm-classic-verb";
pub const DELAY: &str = "clap:dk.mxm.mxm-fx-delay";
/// The instrument every sample instrument becomes.
pub const SAMPLER: &str = "clap:dk.mxm.mxm-creative-sampler";
const SAMPLER_NAME: &str = "MXM Creative Sampler";
/// A sampler device's sample, in its own files.
pub const SAMPLE_FILE: &str = "sample.wav";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok())
        .collect()
}

/// A WAV of 16-bit samples at the Tracker's rate: `data` already in a WAV's order.
fn wav(channels: u16, data: &[u8]) -> Vec<u8> {
    let rate = SAMPLE_RATE;
    let block = channels * 2;
    let pad = data.len() % 2;
    let mut out = Vec::with_capacity(44 + data.len() + pad);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + data.len() + pad) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * block as u32).to_le_bytes());
    out.extend_from_slice(&block.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    if pad == 1 {
        out.push(0);
    }
    out
}

/// A slot's notes as a pattern variant tells them apart: each note's tick in the slot,
/// length, pitch, velocity and the step of the pattern that played it.
type Signature = Vec<(i64, i64, u8, u64, u32)>;

/// How loud an instrument track's notes play, as the firmware plays them: a note sounds at its
/// step's volume, or else its instrument's ([`crate::files::amp_db`]). The loudest of them,
/// `loudest`, is split into the instrument's level (at most 1) times the track's gain (1 or
/// more), and each note's velocity is its share of the loudest; so level × velocity × gain is
/// every note's own gain, a boost past unity included.
struct Levels {
    own: f64,
    loudest: f64,
}

fn gain(db: Option<f64>) -> f64 {
    db.map_or(0.0, |db| 10f64.powf(db / 20.0))
}

impl Levels {
    fn new<'a>(head: &InstrumentFile, notes: impl Iterator<Item = &'a Played>) -> Levels {
        let own = gain(head.volume_db());
        let loudest = notes
            .map(|n| n.volume.map_or(own, |v| gain(crate::files::amp_db(v))))
            .fold(own, f64::max);
        Levels { own, loudest }
    }

    /// The track's gain: what of the loudest is past unity.
    fn track(&self) -> f64 {
        self.loudest.max(1.0)
    }

    /// The instrument's level.
    fn level(&self) -> f64 {
        self.loudest / self.track()
    }

    /// A note's velocity.
    fn velocity(&self, n: &Played) -> f64 {
        if self.loudest <= 0.0 {
            return 1.0;
        }
        let own = n.volume.map_or(self.own, |v| gain(crate::files::amp_db(v)));
        (own / self.loudest).clamp(0.0, 1.0)
    }
}

/// A parameter of one of MXM's effects as a CLAP host sets it: nice-plug's normalized value of
/// `plain` on its range (`Linear`, or `Skewed` with its factor), or a choice's index; written as
/// the snapshot and as an override, so a host that loads the effect without a state sets it
/// either way (tutti 7.3, 7.5).
enum Range {
    Linear(f64, f64),
    Skewed(f64, f64, f64),
    Choice(u32),
}

fn effect_param(id: &str, name: &str, plain: f64, range: Range, text: String) -> tutti::Param {
    let (value, normalized, max) = match range {
        Range::Linear(lo, hi) => {
            let n = ((plain.clamp(lo, hi) - lo) / (hi - lo)).clamp(0.0, 1.0);
            (n, n, 1.0)
        }
        Range::Skewed(lo, hi, factor) => {
            let n = ((plain.clamp(lo, hi) - lo) / (hi - lo))
                .powf(factor)
                .clamp(0.0, 1.0);
            (n, n, 1.0)
        }
        Range::Choice(steps) => (plain, plain / steps.max(1) as f64, steps as f64),
    };
    let mut p: tutti::Param =
        serde_json::from_value(json!({"id": clap_id(id), "value": value})).expect("a parameter");
    p.name = Some(name.to_owned());
    p.normalized = Some(normalized);
    p.min = Some(0.0);
    p.max = Some(max);
    p.unit = Some(if max > 1.0 { "enum" } else { "normalized" }.to_owned());
    p.text = Some(text);
    p
}

/// A return's device: its parameters as the snapshot and as overrides.
fn effect_device(id: Id, plugin: &str, name: &str, params: Vec<tutti::Param>) -> Device {
    // tutti's role for an audio effect (6.4); any other is bypassed.
    let mut device = Device::new(id, "audio_effect", plugin);
    device.name = name.to_owned();
    device.overrides = params
        .iter()
        .map(|p| {
            serde_json::from_value(
                json!({"param": p.id, "value": p.value, "normalized": p.normalized}),
            )
            .expect("an override")
        })
        .collect();
    // Sorted by parameter, as tutti's are (its writer puts them in that order anyway).
    device.overrides.sort_by(|a, b| a.param.cmp(&b.param));
    device.params = params;
    device.params.sort_by(|a, b| a.id.cmp(&b.id));
    device
}

/// MXM Classic Verb as the Tracker's reverb: fully wet; the decay its tank's, the size its
/// longest delay, the pre-delay, the diffusion over the firmware's most, the treble's decay
/// shortened as its damping darkens the tank. Approximate: another reverb.
fn reverb_device(id: Id, r: &crate::files::Reverb) -> Device {
    let centred = |lo: f64, hi: f64, c: f64| 0.5f64.ln() / ((c - lo) / (hi - lo)).ln();
    let decay = r.decay_s().clamp(0.1, 60.0);
    let treble = (1.0 - r.damp as f64).clamp(0.1, 1.0);
    let size = 6_211.0 / 44_117.647;
    let predelay = r.predelay_s();
    let diffusion = (r.diffusion as f64 / 0.85).clamp(0.0, 1.0);
    let params = vec![
        effect_param("mix", "Mix", 1.0, Range::Linear(0.0, 1.0), "100 %".into()),
        effect_param(
            "decay",
            "Decay",
            decay,
            Range::Skewed(0.1, 60.0, centred(0.1, 60.0, 2.0)),
            format!("{decay:.2} s"),
        ),
        effect_param(
            "treble",
            "Treble decay",
            treble,
            Range::Skewed(0.1, 10.0, centred(0.1, 10.0, 1.0)),
            format!("{treble:.2}x"),
        ),
        effect_param(
            "size",
            "Size",
            size,
            Range::Skewed(0.002, 0.3, centred(0.002, 0.3, 0.05)),
            format!("{:.0} ms", size * 1000.0),
        ),
        effect_param(
            "predelay",
            "Pre-delay",
            predelay,
            Range::Skewed(0.0, 0.5, centred(0.0, 0.5, 0.05)),
            format!("{:.0} ms", predelay * 1000.0),
        ),
        effect_param(
            "diffusion",
            "Diffusion",
            diffusion,
            Range::Linear(0.0, 1.0),
            format!("{:.0} %", diffusion * 100.0),
        ),
    ];
    effect_device(id, REVERB, "MXM Classic Verb: the Tracker's reverb", params)
}

/// MXM FX Delay as the Tracker's delay: fully wet, clean; its time (a synced one at `bpm`),
/// feedback, ping-pong and filter, the low cut open.
fn delay_device(id: Id, d: &crate::files::Delay, bpm: f64) -> Device {
    let time = d.time_s(bpm).clamp(0.005, 8.0);
    let feedback = (d.feedback.min(99) as f64 / 100.0).clamp(0.0, 1.0);
    let high = (d.filter_hz as f64).clamp(200.0, 20_000.0);
    let routing = if d.ping_pong { 2.0 } else { 0.0 };
    let params = vec![
        effect_param("mix", "Mix", 1.0, Range::Linear(0.0, 1.0), "100 %".into()),
        effect_param(
            "time",
            "Time",
            time,
            Range::Skewed(0.005, 8.0, 0.25),
            format!("{:.0} ms", time * 1000.0),
        ),
        effect_param("sync", "Time sync", 0.0, Range::Choice(1), "Off".into()),
        effect_param(
            "feedback",
            "Feedback",
            feedback,
            Range::Linear(0.0, 1.0),
            format!("{:.0} %", feedback * 100.0),
        ),
        effect_param(
            "highcut",
            "High cut",
            high,
            Range::Skewed(200.0, 20_000.0, 0.5),
            format!("{high:.0} Hz"),
        ),
        effect_param(
            "lowcut",
            "Low cut",
            10.0,
            Range::Skewed(10.0, 8_000.0, 2f64.powf(-1.6)),
            "10 Hz".into(),
        ),
        effect_param(
            "routing",
            "Routing",
            routing,
            Range::Choice(2),
            if d.ping_pong { "Ping-pong" } else { "Standard" }.into(),
        ),
        effect_param("model", "Model", 0.0, Range::Choice(5), "Clean".into()),
    ];
    effect_device(id, DELAY, "MXM FX Delay: the Tracker's delay", params)
}

/// The fewest ticks a pan LFO's cycle takes in its lane (a sixteenth's twentieth): the Tracker's
/// fastest, a 64th of a step, is held to it.
const PAN_LFO_FASTEST: i64 = 12;

/// A pan LFO's points over the notes it plays under: from each note's start, the firmware's
/// shape (`calcLfoBasedEnvelope`) from 0 to its amount (pan's 100 the whole of tutti's −1 to 1
/// twice over, so `2 × amount`, held to ±1), back to the middle when the note ends; random a
/// value held each cycle, the same each conversion. True when a cycle had to be lengthened.
fn pan_lfo(lfo: &crate::files::Lfo, notes: &[&Played]) -> (Vec<Value>, bool) {
    let exact = (lfo.period_steps(false) * TICKS_PER_STEP as f64).round() as i64;
    let period = exact.max(PAN_LFO_FASTEST);
    let peak = (2.0 * lfo.amount as f64).clamp(-1.0, 1.0);
    let mut points: Vec<(i64, f64, &str)> = Vec::new();
    let mut push = |at: i64, value: f64, curve: &'static str| match points.last() {
        Some(&(last, _, _)) if at < last => {}
        Some(&(last, _, _)) if at == last => {
            *points.last_mut().expect("one") = (at, value, curve);
        }
        _ => points.push((at, value, curve)),
    };
    let mut seed: u32 = 0x9E37_79B9;
    for n in notes {
        let end = n.at + n.length;
        let mut s = n.at;
        while s < end {
            match lfo.shape {
                0 => {
                    push(s, peak, "linear");
                    push(s + period - 1, 0.0, "hold");
                }
                1 => {
                    push(s, 0.0, "linear");
                    push(s + period - 1, peak, "hold");
                }
                2 => {
                    push(s, 0.0, "linear");
                    push(s + period / 2, peak, "linear");
                }
                3 => {
                    push(s, peak, "hold");
                    push(s + period / 2, 0.0, "hold");
                }
                _ => {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    push(s, peak * (seed >> 8) as f64 / (1u32 << 24) as f64, "hold");
                }
            }
            s += period;
        }
        push(end, 0.0, "hold");
    }
    let points = points
        .into_iter()
        .map(|(at, value, curve)| json!({"at": at, "value": value, "curve": curve}))
        .collect();
    (points, period != exact)
}

/// A WAV with its `data` cut to whole frames, and nothing after it: an older instrument's sample
/// may end in part of a frame, which the sampler reads past and other programs refuse.
fn whole_frames(wav: Vec<u8>) -> Vec<u8> {
    let le32 = |b: &[u8], at: usize| u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]);
    let (mut at, mut block) = (12usize, None);
    while at + 8 <= wav.len() {
        let size = le32(&wav, at + 4) as usize;
        let body = at + 8;
        if &wav[at..at + 4] == b"fmt " && body + 14 <= wav.len() {
            block = Some(u16::from_le_bytes([wav[body + 12], wav[body + 13]]) as usize);
        }
        if &wav[at..at + 4] == b"data" {
            let Some(block) = block.filter(|&b| b > 0) else {
                return wav;
            };
            let have = size.min(wav.len() - body);
            let whole = have - have % block;
            if whole == size && body + size + size % 2 == wav.len() {
                return wav;
            }
            let mut out = wav[..body].to_vec();
            out[at + 4..at + 8].copy_from_slice(&(whole as u32).to_le_bytes());
            out.extend_from_slice(&wav[body..body + whole]);
            if whole % 2 == 1 {
                out.push(0);
            }
            let riff = (out.len() - 8) as u32;
            out[4..8].copy_from_slice(&riff.to_le_bytes());
            return out;
        }
        at = body + size + size % 2;
    }
    wav
}

/// Keeps every channel at or under [`LOUDEST_GAIN`] with the mix as it was: where an instrument
/// track needs more, every instrument track is lowered by the same factor (a pre-fader send with
/// it, as it is taken before the gain) and the master raised by it, as far as it goes; the
/// instruments the song never plays, muted, are only held to it.
fn within_reach(document: &mut Document, returns: &[&Id], report: &mut Report) {
    fn leaves<'a>(tracks: &'a mut [Track], out: &mut Vec<&'a mut Track>) {
        for t in tracks {
            if t.children.is_empty() {
                out.push(t);
            } else {
                leaves(&mut t.children, out);
            }
        }
    }
    let unused = document
        .tracks
        .iter()
        .position(|t| t.id.as_str().ends_with("unused") || t.name == "Instruments not in the song");
    let mut played = Vec::new();
    for (i, t) in document.tracks.iter_mut().enumerate() {
        if Some(i) == unused {
            for child in &mut t.children {
                child.channel.gain = child.channel.gain.min(LOUDEST_GAIN);
            }
        } else if !returns.contains(&&t.id) {
            leaves(std::slice::from_mut(t), &mut played);
        }
    }
    let worst = played.iter().map(|t| t.channel.gain).fold(0.0, f64::max);
    if worst <= LOUDEST_GAIN {
        return;
    }
    let factor = worst / LOUDEST_GAIN;
    for t in played {
        // Held to it after the division too, which may round a hair over.
        t.channel.gain = (t.channel.gain / factor).min(LOUDEST_GAIN);
        for send in t.channel.sends.iter_mut().filter(|s| s.pre_fader) {
            send.gain /= factor;
        }
    }
    let master = factor.min(LOUDEST_GAIN);
    document.master.gain *= master;
    let db = |g: f64| 20.0 * g.log10();
    report.approximated(if master < factor {
        format!(
            "A track's gain would be {:+.1} dB: every instrument track is {:.1} dB lower and the \
             master {:+.1} dB, the most most programs take, so the song is {:.1} dB quieter than \
             the Tracker plays it",
            db(worst),
            db(factor),
            db(master),
            db(factor / master)
        )
    } else {
        format!(
            "A track's gain would be {:+.1} dB, more than most programs take: every instrument \
             track is {:.1} dB lower and the master {:+.1} dB, the mix as it was",
            db(worst),
            db(factor),
            db(master)
        )
    });
}

/// A beat-slice instrument's sample in `audio/`, and the frames each slice plays.
struct BeatSlices {
    file: String,
    wav: Vec<u8>,
    sha256: String,
    rate: u32,
    channels: u32,
    frames: u64,
    spans: Vec<(u64, u64)>,
}

impl BeatSlices {
    fn new(index: u8, instrument: &Instrument, pcm: &[u8]) -> Option<BeatSlices> {
        let wav = match (&instrument.layout, &instrument.pcm) {
            (Layout::Pti, Some(p)) => {
                let data = if p.channels == 2 {
                    interleave(pcm)
                } else {
                    pcm.to_vec()
                };
                wav(p.channels as u16, &data)
            }
            _ => pcm.to_vec(),
        };
        let wav = whole_frames(wav);
        let (channels, frames, rate) = crate::model::wav_facts(&wav)?;
        if frames == 0 || channels == 0 {
            return None;
        }
        let head = &instrument.head;
        let count = head.slices().len().max(1);
        let at = |p: u16| (p as u64 * frames as u64 / 65535).min(frames as u64);
        let spans = (0..count)
            .map(|i| {
                let (from, to) = head.slice_span(i);
                (at(from), at(to).max(at(from) + 1).min(frames as u64))
            })
            .collect();
        Some(BeatSlices {
            file: format!("audio/instrument-{}.wav", index as u16 + 1),
            sha256: sha256_hex(&wav),
            wav,
            rate,
            channels: channels as u32,
            frames: frames as u64,
            spans,
        })
    }

    /// The audio piece a note at `pitch`, at tick `at` of its pattern, plays, at the
    /// instrument's level (its velocity applies too, tutti 6.7).
    fn piece(&self, pitch: u8, at: i64, gain: f64) -> tutti::AudioPiece {
        let slice = (pitch as usize)
            .saturating_sub(24)
            .min(self.spans.len() - 1);
        let (from, to) = self.spans[slice];
        serde_json::from_value(json!({
            "file": self.file, "sha256": self.sha256, "sample_rate": self.rate,
            "channels": self.channels, "frames": self.frames, "from": from, "to": to,
            "map": [{"frame": from, "tick": at}], "bpm": null, "gain": gain,
        }))
        .expect("an audio piece")
    }
}

/// Where [`wav`] puts the samples.
const WAV_DATA: u64 = 44;

/// The ids of one project: derived from the source (tutti_convert::derived_id), with a token of
/// the project's name so two converted projects never share an id.
struct Ids {
    token: String,
}

impl Ids {
    fn new(name: &str) -> Self {
        Ids {
            token: sha256_hex(name.as_bytes())[..8].to_owned(),
        }
    }
    fn id(&self, prefix: &str, parts: &[&str]) -> Id {
        let mut all = vec![self.token.as_str()];
        all.extend_from_slice(parts);
        derived_id(prefix, &all)
    }
}

/// The key a Polyend track's folder plays its first sample instrument from: C1, so the Tracker's
/// 48 sample instruments fit in MIDI's keys (36 to 83).
const FOLDER_FIRST_KEY: u8 = 36;

/// The instrument a folder's member plays, as its extension records it.
fn instrument_of(child: &Track) -> Option<u8> {
    child
        .extensions
        .get(NAMESPACE)
        .and_then(|e| e.get("instrument"))
        .and_then(Value::as_u64)
        .and_then(|i| u8::try_from(i).ok())
}

/// An instrument's track inside a Polyend track's folder.
fn instrument_track_name(model: &Model, instrument: u8) -> String {
    match instrument {
        i if i < SAMPLE_INSTRUMENTS => model
            .instruments
            .get(&i)
            .map(Instrument::name)
            .unwrap_or_else(|| format!("Instrument {}", i + 1)),
        i if i < MIDI_INSTRUMENTS_END => format!("MIDI channel {}", i - SAMPLE_INSTRUMENTS + 1),
        i => format!("Synth {}", i - MIDI_INSTRUMENTS_END + 1),
    }
}

fn folder_name(model: &Model, track: usize) -> String {
    if track < SAMPLE_TRACKS {
        let name = model.project.track_name(track);
        if name.is_empty() {
            format!("Track {}", track + 1)
        } else {
            name
        }
    } else {
        format!("MIDI Track {}", track - SAMPLE_TRACKS + 1)
    }
}

fn step_data(step: &Step) -> Value {
    json!([
        step.note,
        step.instrument,
        step.fx[0].kind,
        step.fx[0].value,
        step.fx[1].kind,
        step.fx[1].value,
        step.volume,
        step.micromove
    ])
}

fn step_from(data: &Value) -> Option<Step> {
    let a = data.as_array()?;
    let n = |i: usize| a.get(i)?.as_i64();
    Some(Step {
        note: n(0)? as i8,
        instrument: n(1)? as u8,
        fx: [
            Fx {
                kind: n(2)? as u8,
                value: n(3)? as u8,
            },
            Fx {
                kind: n(4)? as u8,
                value: n(5)? as u8,
            },
        ],
        volume: n(6)? as i8,
        micromove: n(7)? as i8,
    })
}

/// A parameter's CLAP id in a nice-plug plugin, as decimal text: nice-plug hashes the string id
/// (31 times the hash plus each byte, the top bit cleared).
pub fn clap_id(id: &str) -> String {
    let mut hash: u32 = 0;
    for b in id.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(b as u32);
    }
    (hash & !(1 << 31)).to_string()
}

fn param(id: &str, name: &str, value: f64, range: (f64, f64), unit: &str) -> tutti::Param {
    let mut p: tutti::Param =
        serde_json::from_value(json!({"id": clap_id(id), "value": value})).expect("a parameter");
    p.name = Some(name.to_owned());
    p.min = Some(range.0);
    p.max = Some(range.1);
    p.unit = Some(unit.to_owned());
    p
}

/// What of an instrument the sampler takes, as its parameter snapshot (tutti 7.3): layer A plays
/// the sample; note 60 plays it at its own pitch, as on the Tracker; tune and finetune its
/// transpose; start, end and the loop points its own; the play mode its loop (a backward loop
/// plays forward) and its reader (granular its grain); `level` its level ([`Levels`]), panning its
/// pan; the amplitude envelope its ADSR; the filter its filter; the bit depth its converter's
/// bits. The master at full, the gain following velocity exactly, and Init's cutoff routes off,
/// so the instrument sounds as the Tracker plays it; its LFOs as [`lfos`] puts them.
fn sampler_params(head: &InstrumentFile, level: f64, bpm: f64) -> Vec<tutti::Param> {
    use crate::files::PlayMode as M;
    let (loop_mode, reader) = match head.play_mode() {
        M::LoopForward | M::LoopBackward => (1.0, 0.0),
        M::LoopPingPong => (2.0, 0.0),
        M::Granular => (0.0, 2.0),
        _ => (0.0, 0.0),
    };
    let [start, loop1, loop2, end] = head.points().map(|p| p as f64 / 65535.0);
    // A slice instrument plays its selected slice, pitched by the note.
    let (start, end) = if head.play_mode() == M::Slice && !head.slices().is_empty() {
        let (from, to) = head.slice_span(head.selected_slice());
        (from as f64 / 65535.0, to as f64 / 65535.0)
    } else {
        (start, end)
    };
    let (tune, finetune) = head.tune();
    let transpose = (tune as f64 + finetune as f64 / 100.0).clamp(-24.0, 24.0);
    let pan = ((head.panning() as f64 - 50.0) / 50.0).clamp(-1.0, 1.0);
    let unit = (0.0, 1.0);
    // The amplitude envelope, an ADSR in seconds; with none, a gate: on at once, off at once.
    let env = head.amp_envelope();
    let (attack, decay, sustain, release) = if env.is_adsr() {
        (
            env.attack as f64 / 1000.0,
            env.decay as f64 / 1000.0,
            (env.sustain as f64).clamp(0.0, 1.0),
            env.release as f64 / 1000.0,
        )
    } else {
        (0.0, 0.0, 1.0, 0.0)
    };
    // The filter: off, low, band or high pass; cutoff in hertz; resonance from Q's 0.7..=5.
    use crate::files::FilterType as F;
    let (mode, cutoff, resonance) = match head.filter() {
        None => (0.0, 22_000.0, 0.0),
        Some(f) => {
            let mode = match f.kind {
                F::LowPass => 1.0,
                F::BandPass => 2.0,
                F::HighPass => 3.0,
            };
            (mode, f.hz.clamp(20.0, 22_000.0), (f.q - 0.7) / (5.0 - 0.7))
        }
    };
    // The bit depth: the sampler's converter plays `16 - sqrt(c) * 11` bits, 5 the fewest.
    let converter = ((16.0 - head.bit_depth().max(5) as f64) / 11.0).powi(2);
    let mut out = vec![
        param("converter", "Bit depth", converter, unit, "normalized"),
        param("attack", "Attack", attack.min(10.0), (0.0, 10.0), "seconds"),
        param("decay", "Decay", decay.min(10.0), (0.0, 10.0), "seconds"),
        param("sustain", "Sustain", sustain, unit, "normalized"),
        param(
            "release",
            "Release",
            release.min(20.0),
            (0.0, 20.0),
            "seconds",
        ),
        param("filtermode", "Filter mode", mode, (0.0, 3.0), "enum"),
        param("cutoff", "Cutoff", cutoff, (20.0, 22_000.0), "hertz"),
        param("resonance", "Resonance", resonance, unit, "normalized"),
        param("areader", "A Playback mode", reader, (0.0, 2.0), "enum"),
        param("aroot", "A Root", 60.0, (0.0, 127.0), "other"),
        param(
            "atranspose",
            "A Transpose",
            transpose,
            (-24.0, 24.0),
            "semitones",
        ),
        param("astart", "A Start", start, unit, "normalized"),
        param("aend", "A End", end, unit, "normalized"),
        param("aloop", "A Loop", loop_mode, (0.0, 2.0), "enum"),
        param("aloopstart", "A Loop start", loop1, unit, "normalized"),
        param("aloopend", "A Loop end", loop2, unit, "normalized"),
        param(
            "alevel",
            "A Level",
            level.clamp(0.0, 1.0),
            unit,
            "normalized",
        ),
        param("master", "Master", 1.0, unit, "normalized"),
        param(
            "mod_amp_velon",
            "Amplitude from Velocity",
            1.0,
            unit,
            "bool",
        ),
        param(
            "mod_amp_vel",
            "Amplitude from Velocity amount",
            1.0,
            (-1.0, 1.0),
            "linear",
        ),
        param(
            "mod_cutoff_envon",
            "Cutoff from Envelope",
            0.0,
            unit,
            "bool",
        ),
        param("mod_cutoff_lfo2on", "Cutoff from LFO 2", 0.0, unit, "bool"),
        param("apan", "A Pan", pan, (-1.0, 1.0), "linear"),
    ];
    lfos(head, level, bpm, &mut out);
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Replaces the parameter of `p`'s id in `out`, or adds it.
fn set(out: &mut Vec<tutti::Param>, p: tutti::Param) {
    match out.iter_mut().find(|q| q.id == p.id) {
        Some(q) => *q = p,
        None => out.push(p),
    }
}

/// The instrument's LFOs on the sampler's two: the first of volume, cutoff (with the filter on)
/// and finetune to have one, in that order. A Tracker LFO swings from 0 to its amount; the
/// sampler's swing both ways, so each is its middle (the level halved, the cutoff raised, the
/// pitch raised) and a route of half its reach: the cutoff's 0..=1 spans the firmware's 477 / 50
/// octaves, finetune's 100 cents. The rate is the period's steps at the song's first tempo, the
/// shape the nearest; the sampler's LFOs run free, where the Tracker's restart with each note.
fn lfos(head: &InstrumentFile, level: f64, bpm: f64, out: &mut Vec<tutti::Param>) {
    use crate::files::{ENV_AMP, ENV_CUTOFF, ENV_FINETUNE};
    let unit = (0.0, 1.0);
    let route = (-1.0, 1.0);
    let mut slot = 0;
    for target in [ENV_AMP, ENV_CUTOFF, ENV_FINETUNE] {
        if slot == 2 {
            break;
        }
        let Some(lfo) = head.lfo(target) else {
            continue;
        };
        if lfo.amount == 0.0 || (target == ENV_CUTOFF && head.filter().is_none()) {
            continue;
        }
        slot += 1;
        let a = lfo.amount as f64;
        let hz = (bpm / (15.0 * lfo.period_steps(target == ENV_AMP))).clamp(0.01, 40.0);
        // The sampler's shapes: sine, triangle, ramp up, ramp down, square, sample and hold.
        let shape = match lfo.shape {
            0 => 3.0,
            1 => 2.0,
            2 => 1.0,
            3 => 4.0,
            _ => 5.0,
        };
        let (rate, shape_id, sync) = if slot == 1 {
            ("lforate", "lfo1shape", "lfo1sync")
        } else {
            ("lfo2rate", "lfo2shape", "lfo2sync")
        };
        set(out, param(rate, "LFO rate", hz, (0.01, 40.0), "hertz"));
        set(out, param(shape_id, "LFO shape", shape, (0.0, 5.0), "enum"));
        set(out, param(sync, "LFO sync", 0.0, unit, "bool"));
        let n = slot;
        let (name, amount) = match target {
            ENV_AMP => {
                set(
                    out,
                    param(
                        "alevel",
                        "A Level",
                        (level * a.abs() / 2.0).clamp(0.0, 1.0),
                        unit,
                        "normalized",
                    ),
                );
                ("amp", a.signum())
            }
            ENV_CUTOFF => {
                let f = head.filter().expect("on");
                let octaves = a * 477.0 / 50.0 / 2.0;
                let hz = (f.hz * 2f64.powf(octaves)).clamp(20.0, 22_000.0);
                set(
                    out,
                    param("cutoff", "Cutoff", hz, (20.0, 22_000.0), "hertz"),
                );
                ("cutoff", (octaves / 4.0).clamp(-1.0, 1.0))
            }
            _ => {
                let (tune, finetune) = head.tune();
                let transpose =
                    (tune as f64 + finetune as f64 / 100.0 + a / 2.0).clamp(-24.0, 24.0);
                set(
                    out,
                    param(
                        "atranspose",
                        "A Transpose",
                        transpose,
                        (-24.0, 24.0),
                        "semitones",
                    ),
                );
                ("pitch", (a / 2.0 / 12.0).clamp(-1.0, 1.0))
            }
        };
        set(
            out,
            param(
                &format!("mod_{name}_lfo{n}on"),
                "LFO route",
                1.0,
                unit,
                "bool",
            ),
        );
        set(
            out,
            param(
                &format!("mod_{name}_lfo{n}"),
                "LFO route amount",
                amount,
                route,
                "linear",
            ),
        );
    }
}

/// The sampler's device for an instrument, and its sample file to put in the device's own files:
/// a `.pti`'s samples as a WAV, an older instrument's WAV as it is; none for an older instrument
/// whose sample file is missing.
fn sampler_device(
    id: Id,
    instrument_index: u8,
    instrument: &Instrument,
    samples_bytes: Option<&[u8]>,
    level: f64,
    bpm: f64,
) -> (Device, Option<Put>) {
    let mut device = Device::new(id.clone(), "instrument", SAMPLER);
    device.name = format!("{SAMPLER_NAME}: {}", instrument.name());
    device.params = sampler_params(&instrument.head, level, bpm);
    let (layout, wav_name) = match &instrument.layout {
        Layout::Pti => ("pti", None),
        Layout::Mti { wav } => ("mti", wav.clone()),
    };
    let pcm = instrument.pcm.as_ref().map(|p| {
        json!({"channels": p.channels, "frames": p.frames, "bytes": p.bytes, "sha256": p.sha256})
    });
    device.extensions.insert(
        NAMESPACE.to_owned(),
        json!({
            "instrument": instrument_index,
            "file": instrument.file,
            "layout": layout,
            "wav": wav_name,
            "head": hex(&instrument.head.to_bytes()),
            "pcm": pcm,
        }),
    );
    let put = samples_bytes.map(|bytes| {
        let contents = match (&instrument.layout, &instrument.pcm) {
            (Layout::Pti, Some(pcm)) => {
                let data = if pcm.channels == 2 {
                    interleave(bytes)
                } else {
                    bytes.to_vec()
                };
                wav(pcm.channels as u16, &data)
            }
            _ => bytes.to_vec(),
        };
        Put {
            place: Place::DeviceFile {
                device: id,
                path: SAMPLE_FILE.to_owned(),
            },
            contents: Contents::Bytes(contents),
        }
    });
    (device, put)
}

/// Converts the project found into tutti.
pub fn to_tutti(
    source: &dyn Source,
    found: &Found,
    progress: &mut dyn Progress,
) -> Result<Conversion, Error> {
    progress.step(0, 4, "reading the project");
    let (model, locators) = read(source, found)?;
    if progress.cancelled() {
        return Err(Error::Cancelled);
    }
    progress.step(1, 4, "working out the song");
    let song = song::song(&model);
    progress.step(2, 4, "making the tutti project");
    let ids = Ids::new(&found.name);
    let mut report = Report::new();
    let mut files = Vec::new();

    let mut document = Document::new(Generator::new(
        "tutti-polyend",
        env!("CARGO_PKG_VERSION"),
        "https://github.com/mxm-audio/tutti",
    ));
    document.metadata.title = Some(found.name.clone());
    document.ppq = PPQ as u32;
    // The Tracker plays down to 10 BPM; most programs, newDAWn among them, take 20 at the least.
    document.tempo = song
        .tempo
        .iter()
        .map(|&(at, bpm)| TempoPoint::new(at, bpm.clamp(SLOWEST_BPM, 999.0)))
        .collect();
    for &(at, bpm) in song.tempo.iter().filter(|&&(_, b)| b < SLOWEST_BPM) {
        report.approximated(format!(
            "The Tracker's {bpm} BPM at step {} plays at {SLOWEST_BPM} BPM, the slowest most \
             programs take",
            at / TICKS_PER_STEP + 1
        ));
    }

    // The steps the notes stand for; every other step is kept on the document.
    let represented: BTreeSet<(u8, usize, u8)> = song
        .notes
        .iter()
        .map(|n| (song.slots[n.slot].pattern, n.track, n.step))
        .collect();

    // (Polyend track, instrument) → slot → notes.
    let mut by_child: BTreeMap<(usize, u8), BTreeMap<usize, Vec<&Played>>> = BTreeMap::new();
    for note in &song.notes {
        by_child
            .entry((note.track, note.instrument))
            .or_default()
            .entry(note.slot)
            .or_default()
            .push(note);
    }

    // The song's first tempo, which an LFO's rate is worked out at.
    let bpm = song.tempo.first().map_or(120.0, |&(_, b)| b);
    let mut used_instruments = BTreeSet::new();
    // The beat-slice samples put in audio/, each once.
    let mut beat_files = BTreeSet::new();
    // Each Polyend track's instrument tracks, assembled after, with the tick each is first played
    // at and its instrument.
    let mut groups: BTreeMap<usize, Vec<(i64, u8, Track)>> = BTreeMap::new();
    for (&(track, instrument), slots) in &by_child {
        used_instruments.insert(instrument);
        let first = slots
            .values()
            .flatten()
            .map(|n| n.at)
            .min()
            .unwrap_or(i64::MAX);
        let t = format!("t{}", track + 1);
        let i = format!("i{}", instrument as u16 + 1);
        let mut child = Track::new(
            ids.id("trk", &[&t, &i]),
            &instrument_track_name(&model, instrument),
        );
        child
            .extensions
            .insert(NAMESPACE.to_owned(), json!({ "instrument": instrument }));
        let levels = model
            .instruments
            .get(&instrument)
            .filter(|_| instrument < SAMPLE_INSTRUMENTS)
            .map(|inst| Levels::new(&inst.head, slots.values().flatten().copied()));
        if let Some(l) = &levels {
            child.channel.gain = l.track();
        }
        if instrument < SAMPLE_INSTRUMENTS {
            match (
                model.instruments.get(&instrument),
                locators.get(&instrument),
            ) {
                (Some(inst), locator) => {
                    let pcm = locator.map(|l| samples(source, l)).transpose()?;
                    let level = levels.as_ref().map_or(1.0, Levels::level);
                    let (device, put) = sampler_device(
                        ids.id("dev", &[&t, &i]),
                        instrument,
                        inst,
                        pcm.as_deref(),
                        level,
                        bpm,
                    );
                    child.devices.push(device);
                    match put {
                        Some(put) => files.push(put),
                        None => report.unplayed(format!(
                            "{}: its sample file is missing, so it plays nothing",
                            inst.name()
                        )),
                    }
                }
                _ => report.unplayed(format!(
                    "Track {}: instrument {} has no file, so its notes play nowhere",
                    track + 1,
                    instrument + 1
                )),
            }
        } else {
            report.unplayed(format!(
                "Track {}: {} is kept, and plays nowhere (MIDI and synth engines are not converted)",
                track + 1,
                instrument_track_name(&model, instrument)
            ));
        }

        // A beat-slice instrument's notes play their slices as audio pieces of its sample, at the
        // sample's own pitch (the firmware's `playBeatSlice`: slice `note - 24`, held to the
        // slices there are).
        let beat = match model.instruments.get(&instrument) {
            Some(inst) if inst.head.play_mode() == crate::files::PlayMode::BeatSlice => locators
                .get(&instrument)
                .map(|l| samples(source, l))
                .transpose()?
                .and_then(|pcm| BeatSlices::new(instrument, inst, &pcm)),
            _ => None,
        };
        let mut beat_used = false;
        let velocity = |n: &Played| levels.as_ref().map_or(n.velocity, |l| l.velocity(n));

        // Each slot's notes, as a pattern of their own; identical slots of one pattern share it.
        let mut variants: BTreeMap<u8, Vec<Signature>> = BTreeMap::new();
        let mut patterns: BTreeMap<String, Pattern> = BTreeMap::new();
        for (&slot_index, notes) in slots {
            let slot = &song.slots[slot_index];
            let signature: Signature = notes
                .iter()
                .map(|n| {
                    (
                        n.at - slot.at,
                        n.length,
                        n.pitch,
                        velocity(n).to_bits(),
                        n.global_step,
                    )
                })
                .collect();
            let known = variants.entry(slot.pattern).or_default();
            let variant = match known.iter().position(|s| *s == signature) {
                Some(v) => v,
                None => {
                    known.push(signature);
                    known.len() - 1
                }
            };
            let p = format!("p{}", slot.pattern);
            let v = format!("v{}", variant + 1);
            let pattern_id = ids.id("pat", &[&p, &t, &i, &v]);
            let pattern = patterns
                .entry(pattern_id.as_str().to_owned())
                .or_insert_with(|| {
                    let mut pattern =
                        Pattern::new(pattern_id.clone(), slot.steps as i64 * TICKS_PER_STEP);
                    pattern.name = if variant == 0 {
                        format!("P{:02}", slot.pattern)
                    } else {
                        format!("P{:02} {}", slot.pattern, (b'a' + variant as u8) as char)
                    };
                    pattern
                        .extensions
                        .insert(NAMESPACE.to_owned(), json!({ "pattern": slot.pattern }));
                    for n in notes {
                        let g = format!("g{}", n.global_step);
                        let mut note = Note::new(
                            ids.id("evt", &[&p, &t, &i, &v, &g]),
                            n.at - slot.at,
                            n.length,
                            n.pitch,
                            velocity(n),
                        );
                        if let Some(b) = &beat {
                            let level = levels.as_ref().map_or(1.0, Levels::level);
                            note.audio = Some(b.piece(n.pitch, n.at - slot.at, level));
                            beat_used = true;
                        }
                        let step =
                            model.patterns[&slot.pattern].tracks[n.track].steps[n.step as usize];
                        note.extensions.insert(
                            NAMESPACE.to_owned(),
                            json!({
                                "pattern": slot.pattern,
                                "track": n.track,
                                "step": n.step,
                                "data": step_data(&step),
                                "end": n.end.as_str(),
                            }),
                        );
                        pattern.events.push(Event::Note(note));
                    }
                    pattern.events.sort_by(|a, b| a.id().cmp(b.id()));
                    pattern
                });
            let reach = notes
                .iter()
                .map(|n| n.at - slot.at + n.length)
                .max()
                .unwrap_or(0)
                .max(pattern.length);
            let s = format!("s{}", slot_index + 1);
            child.parts.push(Part::new(
                ids.id("prt", &[&s, &t, &i]),
                pattern_id,
                slot.at,
                reach,
            ));
        }
        child.patterns = patterns.into_values().collect();
        // A pan LFO is the track's pan automation: a Polyend track plays one note at a time, so
        // the lane restarts it with each note, as the Tracker does.
        if let Some(lfo) = model
            .instruments
            .get(&instrument)
            .and_then(|inst| inst.head.lfo(crate::files::ENV_PAN))
            .filter(|l| l.amount != 0.0)
        {
            let mut notes: Vec<&Played> = slots.values().flatten().copied().collect();
            notes.sort_by_key(|n| n.at);
            let (points, held) = pan_lfo(&lfo, &notes);
            if held {
                report.approximated(format!(
                    "{}: its pan LFO is faster than the lane's points can follow, so it runs at \
                     {PAN_LFO_FASTEST} ticks a cycle",
                    instrument_track_name(&model, instrument)
                ));
            }
            child.automation.push(
                serde_json::from_value(json!({
                    "id": ids.id("aut", &[&t, &i, "pan"]),
                    "target": {"channel": "pan"},
                    "points": points,
                }))
                .expect("a lane"),
            );
        }
        if let Some(b) = beat.filter(|b| beat_used && beat_files.insert(b.file.clone())) {
            files.push(Put {
                place: Place::Audio {
                    file: b.file.trim_start_matches("audio/").to_owned(),
                },
                contents: Contents::Bytes(b.wav),
            });
        }
        child.parts.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        groups
            .entry(track)
            .or_default()
            .push((first, instrument, child));
    }
    // The returns the sends go to, made when any track sends to them.
    let reverb_id = ids.id("trk", &["reverb"]);
    let delay_id = ids.id("trk", &["delay"]);
    let (mut reverb_used, mut delay_used) = (false, false);
    // A Polyend track playing one instrument is that instrument's track, named for both; one
    // playing several is a folder of them. Its mute is the Polyend track's.
    let mut zoned = false;
    for (track, mut members) in groups {
        // A folder's members in the order the track first plays them, its sample instruments
        // first: the order its keys play them in.
        members.sort_by_key(|&(first, instrument, _)| {
            (instrument >= SAMPLE_INSTRUMENTS, first, instrument)
        });
        let mut children: Vec<Track> = members.into_iter().map(|(_, _, child)| child).collect();
        let muted = track < SAMPLE_TRACKS && model.project.track_muted(track);
        // The track's volume and the dry mix's, as the mixer plays them (`modVolume`).
        let gain = if track < SAMPLE_TRACKS {
            self::gain(model.project.track_volume_db(track))
                * self::gain(model.project.dry_mix_db())
        } else {
            1.0
        };
        let name = folder_name(&model, track);
        // The sends, as the firmware's `modReverbSend` and `modDelaySend` take them: after the
        // volumes, the send's level times the return's volume, the dry mix not in it; before
        // them where the project says so, the instrument's own volume taken out.
        if track < SAMPLE_TRACKS {
            let single = children.len() == 1;
            for child in &mut children {
                let Some(inst) = child
                    .extensions
                    .get(NAMESPACE)
                    .and_then(|e| e.get("instrument"))
                    .and_then(Value::as_u64)
                    .and_then(|i| model.instruments.get(&(i as u8)))
                else {
                    continue;
                };
                let (reverb, delay) = inst.head.sends();
                for (send, to, is_reverb) in [(reverb, &reverb_id, true), (delay, &delay_id, false)]
                {
                    let level = self::gain(crate::files::send_db(send))
                        * self::gain(model.project.fx_volume_db(is_reverb));
                    if level <= 0.0 {
                        continue;
                    }
                    let pre = model.project.sends_pre_fader();
                    let gain = if pre {
                        let own = self::gain(inst.head.volume_db());
                        if own > 0.0 {
                            level * child.channel.gain / own
                        } else {
                            0.0
                        }
                    } else if single {
                        let dry = self::gain(model.project.dry_mix_db());
                        if dry > 0.0 { level / dry } else { 0.0 }
                    } else {
                        level * self::gain(model.project.track_volume_db(track))
                    };
                    child.channel.sends.push(
                        serde_json::from_value(json!({
                            "to": to, "gain": gain, "pre_fader": pre, "enabled": !muted || pre,
                        }))
                        .expect("a send"),
                    );
                    if is_reverb {
                        reverb_used = true;
                    } else {
                        delay_used = true;
                    }
                }
                child
                    .channel
                    .sends
                    .sort_by(|a, b| a.to.as_str().cmp(b.to.as_str()));
            }
        }
        if children.len() == 1 {
            let mut only = children.pop().expect("one");
            only.name = format!("{name}: {}", only.name);
            only.channel.mute = muted;
            only.channel.gain *= gain;
            if let Some(Value::Object(e)) = only.extensions.get_mut(NAMESPACE) {
                e.insert("track".to_owned(), json!(track));
            }
            document.tracks.push(only);
        } else {
            let t = format!("t{}", track + 1);
            let mut folder = Track::new(ids.id("trk", &[&t]), &name);
            folder.channel.mute = muted;
            folder.channel.gain = gain;
            folder
                .extensions
                .insert(NAMESPACE.to_owned(), json!({ "track": track }));
            // Played from its keys (tutti 6.12): each sample instrument on its own key from C1
            // on, at its sample's own pitch (note 60), all in one choke group, the Tracker's one
            // voice per track; the drum map names the keys. A MIDI or synth member plays nowhere,
            // and takes none.
            let mut lanes = Vec::new();
            for (place, child) in children
                .iter_mut()
                .filter(|child| instrument_of(child).is_some_and(|i| i < SAMPLE_INSTRUMENTS))
                .enumerate()
            {
                let key = FOLDER_FIRST_KEY + place as u8;
                child.zone = Some(Zone {
                    keys: [key, key],
                    transpose: 60 - i32::from(key),
                    choke: 1,
                    ..Zone::default()
                });
                lanes.push(Lane {
                    pitch: key,
                    name: child.name.clone(),
                    unknown: Unknown::new(),
                });
            }
            if !lanes.is_empty() {
                folder.drum_map = Some(lanes);
                zoned = true;
            }
            folder.children = children;
            document.tracks.push(folder);
        }
    }

    if reverb_used {
        let mut r = Track::new(reverb_id.clone(), "Reverb");
        r.channel.mute = model.project.fx_volume_db(true).is_none();
        r.devices.push(reverb_device(
            ids.id("dev", &["reverb"]),
            &model.project.reverb(),
        ));
        document.tracks.push(r);
    }
    if delay_used {
        let mut d = Track::new(delay_id.clone(), "Delay");
        d.channel.mute = model.project.fx_volume_db(false).is_none();
        d.devices.push(delay_device(
            ids.id("dev", &["delay"]),
            &model.project.delay(),
            bpm,
        ));
        document.tracks.push(d);
    }

    // The instruments the song never plays, so the project keeps every one.
    let mut unused = Track::new(ids.id("trk", &["unused"]), "Instruments not in the song");
    unused.channel.mute = true;
    for (&index, instrument) in &model.instruments {
        if used_instruments.contains(&index) {
            continue;
        }
        let i = format!("i{}", index as u16 + 1);
        let mut child = Track::new(ids.id("trk", &["unused", &i]), &instrument.name());
        child
            .extensions
            .insert(NAMESPACE.to_owned(), json!({ "instrument": index }));
        let pcm = locators
            .get(&index)
            .map(|l| samples(source, l))
            .transpose()?;
        let levels = Levels::new(&instrument.head, std::iter::empty());
        child.channel.gain = levels.track();
        let (device, put) = sampler_device(
            ids.id("dev", &["unused", &i]),
            index,
            instrument,
            pcm.as_deref(),
            levels.level(),
            bpm,
        );
        child.devices.push(device);
        files.extend(put);
        unused.children.push(child);
    }
    if !unused.children.is_empty() {
        document.tracks.push(unused);
    }
    within_reach(&mut document, &[&reverb_id, &delay_id], &mut report);

    // The project's files, and every step no note stands for.
    let mut patterns = serde_json::Map::new();
    for (&number, file) in &model.patterns {
        let mut steps = serde_json::Map::new();
        for (t, track) in file.tracks.iter().enumerate() {
            let kept: Vec<Value> = track
                .steps
                .iter()
                .enumerate()
                .filter(|(s, step)| {
                    **step != Step::EMPTY && !represented.contains(&(number, t, *s as u8))
                })
                .map(|(s, step)| {
                    let mut data = step_data(step);
                    data.as_array_mut().expect("an array").insert(0, json!(s));
                    data
                })
                .collect();
            if !kept.is_empty() {
                steps.insert(t.to_string(), Value::Array(kept));
            }
        }
        patterns.insert(
            number.to_string(),
            json!({
                "header": hex(&file.header.to_bytes()),
                "reserved": hex(&file.reserved),
                "crc": file.crc.map(|c| hex(&c)),
                "lasts": file.tracks.iter().map(|t| t.last).collect::<Vec<_>>(),
                "steps": steps,
            }),
        );
    }
    document.extensions.insert(
        NAMESPACE.to_owned(),
        json!({
            "version": VERSION,
            "project": hex(&model.project.to_bytes()),
            "metadata": model.metadata.as_deref().map(hex),
            "patterns": patterns,
        }),
    );
    document.extensions_used = vec![NAMESPACE.to_owned()];

    remarks(&model, &song, zoned, &mut report);
    progress.step(3, 4, "converted");
    Ok(Conversion {
        name: found.name.clone(),
        document,
        files,
        report,
    })
}

fn remarks(model: &Model, song: &Song, zoned: bool, report: &mut Report) {
    for number in &song.missing {
        report.approximated(format!(
            "The song plays pattern {number}, which has no file: played empty for {} steps",
            song::DEFAULT_STEPS
        ));
    }
    for (&kind, &count) in &song.unplayed_fx {
        report.unplayed(format!(
            "{count} played steps use {}: kept, not played",
            fx::name(kind)
        ));
    }
    if zoned {
        report.approximated(
            "A Polyend track playing several instruments is a folder of them, played from its \
             keys: each sample instrument on a key of its own from C1 up, in the order the track \
             first plays them, at its sample's own pitch, all in one choke group, the Tracker's \
             one voice per track, so a note played there stops the one before it in a host that \
             chokes. The song's notes stay on the instruments, each ending where the track's \
             next begins, so a note's release rings on past it in every host, where the Tracker \
             cuts it",
        );
    }
    if song.volume_changes > 0 {
        report.unplayed(format!(
            "{} steps without a note change a sounding note's volume: kept, not played",
            song.volume_changes
        ));
    }
    if !model.instruments.is_empty() {
        report.approximated(
            "The instruments are MXM Creative Samplers: their samples, play modes (a backward \
             loop plays forward, granular as grain), start, end and loop points, tune, volume, \
             panning, amplitude envelopes (attack, decay, sustain, release), filters, bit \
             depth (4 bits as 5, the sampler's fewest) and slices carried over (a slice \
             instrument plays its selected slice; beat slices play as audio pieces of their \
             slices, their points not snapped to zero crossings as the Tracker does) and the \
             first two LFOs on volume, cutoff or finetune (the sampler's run free at the song's \
             first tempo, the Tracker's restart with each note), and a pan LFO as the track's \
             pan automation, restarted with each note; their wavetable and grain LFOs, other \
             envelopes, an envelope's delay and hold, wavetable and granular \
             settings, glide (a time on the Tracker, per octave on the sampler) and overdrive \
             kept, not applied, as are the master's EQ, boosts and limiter. The reverb and \
             delay sends go to returns of MXM Classic Verb and MXM FX Delay, their settings \
             mapped as near as those effects allow (a synced delay at the song's first tempo) Each note plays at its step's volume, or its instrument's, through the \
             sampler's level, the note's velocity and the track's gain",
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Back to the model.

fn bad(why: impl Into<String>) -> Error {
    Error::Unreadable {
        reference: "the document".to_owned(),
        why: why.into(),
    }
}

fn hex_field(value: &Value, key: &str) -> Result<Vec<u8>, Error> {
    value
        .get(key)
        .and_then(Value::as_str)
        .and_then(unhex)
        .ok_or_else(|| bad(format!("no {key} under {NAMESPACE}")))
}

fn every_track(tracks: &[Track]) -> Vec<&Track> {
    let mut out = Vec::new();
    for t in tracks {
        out.push(t);
        out.extend(every_track(&t.children));
    }
    out
}

/// The model a converted document holds, and where its samples are in `project` (the tutti
/// project's files): what a Polyend writer builds on, and what proves nothing was lost.
pub fn to_model(document: &Document, _project: &dyn Source) -> Result<(Model, Locators), Error> {
    let root = document
        .extensions
        .get(NAMESPACE)
        .ok_or_else(|| bad(format!("no {NAMESPACE} on the document")))?;
    let project = ProjectFile::parse(&hex_field(root, "project")?)
        .map_err(|e| bad(format!("its project file: {}", e.0)))?;
    let metadata = match root.get("metadata") {
        Some(Value::String(text)) => Some(unhex(text).ok_or_else(|| bad("its metadata"))?),
        _ => None,
    };

    let mut patterns = BTreeMap::new();
    let listed = root
        .get("patterns")
        .and_then(Value::as_object)
        .ok_or_else(|| bad("no patterns"))?;
    for (number, p) in listed {
        let number: u8 = number.parse().map_err(|_| bad("a pattern's number"))?;
        let header = Header::parse(&hex_field(p, "header")?).map_err(|e| bad(e.0))?;
        let mut reserved = [0u8; 12];
        reserved.copy_from_slice(&hex_field(p, "reserved")?);
        let crc = match p.get("crc") {
            Some(Value::String(text)) => {
                let b = unhex(text).ok_or_else(|| bad("a pattern's CRC"))?;
                Some([b[0], b[1], b[2], b[3]])
            }
            _ => None,
        };
        let lasts = p
            .get("lasts")
            .and_then(Value::as_array)
            .ok_or_else(|| bad("a pattern's lengths"))?;
        let mut tracks: Vec<PolyendTrack> = lasts
            .iter()
            .map(|l| PolyendTrack {
                last: l.as_u64().unwrap_or(0) as u8,
                steps: vec![Step::EMPTY; STEPS],
            })
            .collect();
        if let Some(steps) = p.get("steps").and_then(Value::as_object) {
            for (t, list) in steps {
                let t: usize = t.parse().map_err(|_| bad("a track's number"))?;
                for item in list.as_array().into_iter().flatten() {
                    let a = item.as_array().ok_or_else(|| bad("a kept step"))?;
                    let s = a
                        .first()
                        .and_then(Value::as_u64)
                        .ok_or_else(|| bad("a step"))?;
                    let step = step_from(&Value::Array(a[1..].to_vec()))
                        .ok_or_else(|| bad("a kept step"))?;
                    tracks[t].steps[s as usize] = step;
                }
            }
        }
        patterns.insert(
            number,
            PatternFile {
                header,
                reserved,
                tracks,
                crc,
            },
        );
    }

    let mut instruments = BTreeMap::new();
    let mut locators = Locators::new();
    for track in every_track(&document.tracks) {
        for pattern in &track.patterns {
            for event in &pattern.events {
                let Event::Note(note) = event else { continue };
                let Some(e) = note.extensions.get(NAMESPACE) else {
                    continue;
                };
                let (Some(p), Some(t), Some(s)) = (
                    e.get("pattern").and_then(Value::as_u64),
                    e.get("track").and_then(Value::as_u64),
                    e.get("step").and_then(Value::as_u64),
                ) else {
                    continue;
                };
                let step = e
                    .get("data")
                    .and_then(step_from)
                    .ok_or_else(|| bad("a note's step"))?;
                if let Some(file) = patterns.get_mut(&(p as u8)) {
                    file.tracks[t as usize].steps[s as usize] = step;
                }
            }
        }
        for device in &track.devices {
            let Some(e) = device.extensions.get(NAMESPACE) else {
                continue;
            };
            let index = e
                .get("instrument")
                .and_then(Value::as_u64)
                .ok_or_else(|| bad("a device's instrument"))? as u8;
            if instruments.contains_key(&index) {
                continue;
            }
            let head_bytes = hex_field(e, "head")?;
            let head = InstrumentFile::parse(&head_bytes, head_bytes.len() as u64)
                .map_err(|why| bad(format!("an instrument's header: {}", why.0)))?;
            let pcm = match e.get("pcm") {
                Some(Value::Object(p)) => Some(Pcm {
                    channels: p.get("channels").and_then(Value::as_u64).unwrap_or(0) as u8,
                    frames: p.get("frames").and_then(Value::as_u64).unwrap_or(0) as u32,
                    bytes: p.get("bytes").and_then(Value::as_u64).unwrap_or(0),
                    sha256: p
                        .get("sha256")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                }),
                _ => None,
            };
            let layout = match e.get("layout").and_then(Value::as_str) {
                Some("mti") => Layout::Mti {
                    wav: e.get("wav").and_then(Value::as_str).map(str::to_owned),
                },
                _ => Layout::Pti,
            };
            if let Some(pcm) = &pcm {
                let file = format!("plugins/files/{}/{SAMPLE_FILE}", device.id.as_str());
                let (offset, interleaved) = match layout {
                    Layout::Pti => (WAV_DATA, pcm.channels == 2),
                    Layout::Mti { .. } => (0, false),
                };
                locators.insert(
                    index,
                    Locator {
                        reference: file,
                        offset,
                        bytes: pcm.bytes,
                        interleaved,
                    },
                );
            }
            instruments.insert(
                index,
                Instrument {
                    file: e
                        .get("file")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    head,
                    layout,
                    pcm,
                },
            );
        }
    }

    Ok((
        Model {
            project,
            patterns,
            metadata,
            instruments,
        },
        locators,
    ))
}
