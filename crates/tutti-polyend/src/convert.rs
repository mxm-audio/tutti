//! A Polyend Tracker project into tutti, and back to the model.
//!
//! - Each Polyend track that plays is a folder track, holding one track per instrument it plays
//!   (a step names its instrument; a tutti track has one instrument). Instruments the song never
//!   plays sit, each on its own track, in a muted folder, so nothing is left out.
//! - The song's order is parts: a part per slot on each instrument track that plays in it. A
//!   pattern's notes are worked out per slot ([`crate::song`]); slots of one pattern whose notes
//!   are identical share one tutti pattern, and the others get variants (`P03 b`), so a pattern
//!   repeated with a different successor keeps the note that rings into it.
//! - A note outlasting its pattern keeps its length, past the pattern's, and its part is
//!   lengthened to sound it: the part plays a note for as long as the part lasts.
//! - A sample instrument is an MXM Creative Sampler device, its sample a WAV in the device's own
//!   files (tutti 7.8). Its settings are kept under the extension: the sampler's parameters are
//!   mapped once their table is reviewed.
//! - Everything tutti does not hold is kept under [`NAMESPACE`] on the object it belongs to:
//!   the project file and every step no note stands for, on the document; a note's step, on the
//!   note; an instrument's header, on its device. [`to_model`] reads the model back from them,
//!   byte for byte.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use tutti::render::sha256_hex;
use tutti::{Device, Document, Event, Generator, Id, Note, Part, Pattern, TempoPoint, Track};
use tutti_convert::{
    Contents, Conversion, Error, Found, Place, Progress, Put, Report, Source, derived_id,
};

use crate::files::{
    Fx, Header, InstrumentFile, PCM_OFFSET, PatternFile, ProjectFile, STEPS, Step,
    Track as PolyendTrack,
};
use crate::model::{
    Instrument, Locator, Locators, Model, Pcm, SAMPLE_RATE, interleave, read, samples,
};
use crate::song::{
    self, MIDI_INSTRUMENTS_END, PPQ, Played, SAMPLE_INSTRUMENTS, SAMPLE_TRACKS, Song,
    TICKS_PER_STEP, fx,
};

/// The extension's namespace (tutti 8.1), registered in tutti's `spec/1.0/extensions.md`.
pub const NAMESPACE: &str = "dk.mxm.polyend-tracker";
/// The conversion's version: raised whenever what it writes changes, so earlier outputs are out
/// of date.
pub const VERSION: u32 = 1;
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

/// The sampler's device for an instrument, its sample file, and what to put there.
fn sampler_device(
    id: Id,
    instrument_index: u8,
    instrument: &Instrument,
    samples_bytes: &[u8],
) -> (Device, Put) {
    let mut device = Device::new(id.clone(), "instrument", SAMPLER);
    device.name = format!("{SAMPLER_NAME}: {}", instrument.name());
    device.extensions.insert(
        NAMESPACE.to_owned(),
        json!({
            "instrument": instrument_index,
            "file": instrument.file,
            "head": hex(&instrument.head.to_bytes()),
            "pcm": {
                "channels": instrument.pcm.channels,
                "frames": instrument.pcm.frames,
                "bytes": instrument.pcm.bytes,
                "sha256": instrument.pcm.sha256,
            },
        }),
    );
    let data = if instrument.pcm.channels == 2 {
        interleave(samples_bytes)
    } else {
        samples_bytes.to_vec()
    };
    let put = Put {
        place: Place::DeviceFile {
            device: id,
            path: SAMPLE_FILE.to_owned(),
        },
        contents: Contents::Bytes(wav(instrument.pcm.channels as u16, &data)),
    };
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
    document.tempo = song
        .tempo
        .iter()
        .map(|&(at, bpm)| TempoPoint::new(at, bpm))
        .collect();

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

    let mut used_instruments = BTreeSet::new();
    let mut folders: BTreeMap<usize, Track> = BTreeMap::new();
    for (&(track, instrument), slots) in &by_child {
        used_instruments.insert(instrument);
        let t = format!("t{}", track + 1);
        let i = format!("i{}", instrument as u16 + 1);
        let folder = folders.entry(track).or_insert_with(|| {
            let mut folder = Track::new(ids.id("trk", &[&t]), &folder_name(&model, track));
            folder.channel.mute = track < SAMPLE_TRACKS && model.project.track_muted(track);
            folder
                .extensions
                .insert(NAMESPACE.to_owned(), json!({ "track": track }));
            folder
        });
        let mut child = Track::new(
            ids.id("trk", &[&t, &i]),
            &instrument_track_name(&model, instrument),
        );
        child
            .extensions
            .insert(NAMESPACE.to_owned(), json!({ "instrument": instrument }));
        if instrument < SAMPLE_INSTRUMENTS {
            match (
                model.instruments.get(&instrument),
                locators.get(&instrument),
            ) {
                (Some(inst), Some(locator)) => {
                    let pcm = samples(source, locator)?;
                    let (device, put) =
                        sampler_device(ids.id("dev", &[&t, &i]), instrument, inst, &pcm);
                    child.devices.push(device);
                    files.push(put);
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
                        n.velocity.to_bits(),
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
                            n.velocity,
                        );
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
        child.parts.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        folder.children.push(child);
    }
    document.tracks = folders.into_values().collect();

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
        let pcm = samples(source, &locators[&index])?;
        let (device, put) = sampler_device(ids.id("dev", &["unused", &i]), index, instrument, &pcm);
        child.devices.push(device);
        files.push(put);
        unused.children.push(child);
    }
    if !unused.children.is_empty() {
        document.tracks.push(unused);
    }

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

    remarks(&model, &song, &mut report);
    progress.step(3, 4, "converted");
    Ok(Conversion {
        name: found.name.clone(),
        document,
        files,
        report,
    })
}

fn remarks(model: &Model, song: &Song, report: &mut Report) {
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
    if song.volume_changes > 0 {
        report.unplayed(format!(
            "{} steps without a note change a sounding note's volume: kept, not played",
            song.volume_changes
        ));
    }
    if !model.instruments.is_empty() {
        report.unplayed(
            "The instruments' samples are in their devices' own files and every setting (play \
             mode, start, end and loop points, slices, tune, volume, panning, filter, envelopes, \
             LFOs) is kept; MXM Creative Sampler does not load them from there yet",
        );
    }
    if (0..SAMPLE_TRACKS).any(|t| model.project.track_volume(t) != 100) {
        report.unplayed("The tracks' volumes are kept, not applied");
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
            let head = InstrumentFile::parse(&head_bytes, PCM_OFFSET as u64)
                .map_err(|why| bad(format!("an instrument's header: {}", why.0)))?;
            let pcm = e.get("pcm").ok_or_else(|| bad("an instrument's samples"))?;
            let pcm = Pcm {
                channels: pcm.get("channels").and_then(Value::as_u64).unwrap_or(1) as u8,
                frames: pcm.get("frames").and_then(Value::as_u64).unwrap_or(0) as u32,
                bytes: pcm.get("bytes").and_then(Value::as_u64).unwrap_or(0),
                sha256: pcm
                    .get("sha256")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            };
            locators.insert(
                index,
                Locator {
                    reference: format!("plugins/files/{}/{SAMPLE_FILE}", device.id.as_str()),
                    offset: WAV_DATA,
                    bytes: pcm.bytes,
                    interleaved: pcm.channels == 2,
                },
            );
            instruments.insert(
                index,
                Instrument {
                    file: e
                        .get("file")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    head,
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
