//! A Polyend Tracker project as its files hold it ([`Model`]), and where each instrument's
//! samples are found ([`Locators`], kept beside the model, so two models compare equal whatever
//! source they were read from).

use std::collections::BTreeMap;

use tutti::folder::{FolderError, Kind};
use tutti::render::sha256_hex;
use tutti_convert::{Error, Found, Source};

use crate::files::{InstrumentFile, PatternFile, ProjectFile, Unreadable};

/// A project: its files, every byte of them but the instruments' samples, which [`Pcm`]
/// describes and a [`Locator`] finds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    pub project: ProjectFile,
    /// By pattern number (`pattern_NN.mtp`).
    pub patterns: BTreeMap<u8, PatternFile>,
    /// `patterns/patternsMetadata`, kept as read.
    pub metadata: Option<Vec<u8>>,
    /// By instrument index, 0 to 47 (the file `N name.pti` holds index N - 1).
    pub instruments: BTreeMap<u8, Instrument>,
}

/// One instrument: its file's name, everything before its samples, where its samples are kept,
/// and its samples described (none for an older instrument whose sample file is missing).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instrument {
    /// As on the card: `1 Kick.pti`, or the older `instrument_01.mti`.
    pub file: String,
    pub head: InstrumentFile,
    pub layout: Layout,
    pub pcm: Option<Pcm>,
}

/// Where an instrument's samples are kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Layout {
    /// After its header in its `.pti`, 16-bit at 44 100 Hz, a stereo sample's channels one after
    /// the other.
    Pti,
    /// In its own WAV in `samples/` (`instrNN.wav`), as the older firmware kept them, or none.
    Mti { wav: Option<String> },
}

impl Instrument {
    /// The name the device shows: the file's name without its number and extension, or, for an
    /// older instrument, its sample's name.
    pub fn name(&self) -> String {
        if matches!(self.layout, Layout::Mti { .. }) {
            let name = self.head.sample_name();
            let name = name
                .rsplit_once('.')
                .map(|(s, _)| s)
                .unwrap_or(&name)
                .trim();
            return if name.is_empty() {
                self.file.clone()
            } else {
                name.to_owned()
            };
        }
        let stem = self.file.strip_suffix(".pti").unwrap_or(&self.file);
        stem.split_once(' ')
            .map(|(_, n)| n)
            .unwrap_or(stem)
            .trim()
            .to_owned()
    }
}

/// An instrument's samples: in a `.pti`, 16-bit signed little-endian at 44 100 Hz, a stereo
/// sample's channels one after the other (all of the left, then all of the right), as the Tracker
/// stores them; in the older layout, the whole WAV file, its channels and frames as its header
/// says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pcm {
    pub channels: u8,
    pub frames: u32,
    /// Their length in bytes and SHA-256.
    pub bytes: u64,
    pub sha256: String,
}

/// The rate every Tracker sample plays at.
pub const SAMPLE_RATE: u32 = 44_100;

/// Where an instrument's samples are: `bytes` bytes from `offset` in the file `reference`, in the
/// Tracker's order, or, `interleaved`, a stereo sample's frames one after the other (a WAV's
/// order), which [`samples`] gives back in the Tracker's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locator {
    pub reference: String,
    pub offset: u64,
    pub bytes: u64,
    pub interleaved: bool,
}

pub type Locators = BTreeMap<u8, Locator>;

fn join(at: &str, name: &str) -> String {
    if at.is_empty() {
        name.to_owned()
    } else {
        format!("{at}/{name}")
    }
}

fn read_file(source: &dyn Source, reference: &str) -> Result<Vec<u8>, Error> {
    source.read(reference).map_err(|error| Error::Source {
        reference: reference.to_owned(),
        error,
    })
}

fn unreadable(reference: &str) -> impl Fn(Unreadable) -> Error + '_ {
    move |Unreadable(why)| Error::Unreadable {
        reference: reference.to_owned(),
        why,
    }
}

/// The names in a folder, files only, sorted; none when the folder is not there.
pub(crate) fn files_in(source: &dyn Source, folder: &str) -> Result<Vec<String>, Error> {
    match source.entries(folder) {
        Ok(entries) => {
            let mut names: Vec<String> = entries
                .into_iter()
                .filter(|e| e.kind == Kind::File)
                .map(|e| e.name)
                .collect();
            names.sort();
            Ok(names)
        }
        Err(FolderError::Missing) => Ok(Vec::new()),
        Err(error) => Err(Error::Source {
            reference: folder.to_owned(),
            error,
        }),
    }
}

/// A pattern file's number, from `pattern_NN.mtp`.
pub(crate) fn pattern_number(name: &str) -> Option<u8> {
    let n = name.strip_prefix("pattern_")?.strip_suffix(".mtp")?;
    n.parse().ok().filter(|&n| n > 0)
}

/// An older instrument file's index, from `instrument_NN.mti`: NN - 1.
pub(crate) fn old_instrument_index(name: &str) -> Option<u8> {
    let n: u8 = name
        .strip_prefix("instrument_")?
        .strip_suffix(".mti")?
        .parse()
        .ok()?;
    (1..=48).contains(&n).then(|| n - 1)
}

/// A WAV's channels and frames, from its `fmt ` and `data` chunks; none for what is not one.
pub(crate) fn wav_facts(bytes: &[u8]) -> Option<(u8, u32)> {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let (mut at, mut channels, mut block) = (12usize, None, None);
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]])
            as usize;
        let body = at + 8;
        if id == b"fmt " && body + 14 <= bytes.len() {
            channels = Some(u16::from_le_bytes([bytes[body + 2], bytes[body + 3]]) as u8);
            block = Some(u16::from_le_bytes([bytes[body + 12], bytes[body + 13]]) as u32);
        }
        if id == b"data" {
            let block = block?.max(1);
            return Some((channels?, (size.min(bytes.len() - body) as u32) / block));
        }
        at = body + size + size % 2;
    }
    None
}

/// An instrument file's index, from `N name.pti` (`01 name.pti` too): N - 1.
pub(crate) fn instrument_index(name: &str) -> Option<u8> {
    let stem = name.strip_suffix(".pti")?;
    let number = stem.split(' ').next()?;
    let n: u8 = number.parse().ok()?;
    (1..=48).contains(&n).then(|| n - 1)
}

/// The project's files, as [`crate::PolyendTracker::inputs`] names them.
pub(crate) fn inputs(source: &dyn Source, at: &str) -> Result<Vec<String>, Error> {
    let mut out = vec![join(at, "project.mt")];
    let patterns = join(at, "patterns");
    for name in files_in(source, &patterns)? {
        if pattern_number(&name).is_some() || name == "patternsMetadata" {
            out.push(join(&patterns, &name));
        }
    }
    let instruments = join(at, "instruments");
    for name in files_in(source, &instruments)? {
        if name.ends_with(".pti") || name.ends_with(".mti") {
            out.push(join(&instruments, &name));
        }
    }
    let samples = join(at, "samples");
    for name in files_in(source, &samples)? {
        if name.to_ascii_lowercase().ends_with(".wav") {
            out.push(join(&samples, &name));
        }
    }
    Ok(out)
}

/// Reads a project: every file, the samples described and located, not held.
pub fn read(source: &dyn Source, found: &Found) -> Result<(Model, Locators), Error> {
    let at = found.at.as_str();
    let project_ref = join(at, "project.mt");
    let project =
        ProjectFile::parse(&read_file(source, &project_ref)?).map_err(unreadable(&project_ref))?;

    let instruments_dir = join(at, "instruments");
    let instrument_files = files_in(source, &instruments_dir)?;
    let samples_dir = join(at, "samples");
    let sample_files: Vec<String> = files_in(source, &samples_dir)?
        .into_iter()
        .filter(|n| n.to_ascii_lowercase().ends_with(".wav"))
        .collect();

    let patterns_dir = join(at, "patterns");
    let mut patterns = BTreeMap::new();
    let mut metadata = None;
    for name in files_in(source, &patterns_dir)? {
        let reference = join(&patterns_dir, &name);
        if let Some(number) = pattern_number(&name) {
            let file = PatternFile::parse(&read_file(source, &reference)?)
                .map_err(unreadable(&reference))?;
            patterns.insert(number, file);
        } else if name == "patternsMetadata" {
            metadata = Some(read_file(source, &reference)?);
        }
    }

    let mut instruments = BTreeMap::new();
    let mut locators = Locators::new();
    let mut named_samples = Vec::new();
    for name in instrument_files {
        let (index, older) = match (instrument_index(&name), old_instrument_index(&name)) {
            (Some(i), _) => (i, false),
            (None, Some(i)) => (i, true),
            _ => continue,
        };
        let reference = join(&instruments_dir, &name);
        if instruments.contains_key(&index) {
            return Err(Error::Unreadable {
                reference,
                why: format!("a second file for instrument {}", index + 1),
            });
        }
        let bytes = read_file(source, &reference)?;
        let head =
            InstrumentFile::parse(&bytes, bytes.len() as u64).map_err(unreadable(&reference))?;
        let (layout, pcm) = if older {
            if bytes.len() != head.len() {
                return Err(Error::Unreadable {
                    reference,
                    why: "an older instrument longer than its header".to_owned(),
                });
            }
            let wanted = format!("instr{:02}.wav", index + 1);
            let wav = sample_files
                .iter()
                .find(|n| n.eq_ignore_ascii_case(&wanted))
                .cloned();
            let pcm = match &wav {
                Some(file) => {
                    named_samples.push(file.clone());
                    let wav_ref = join(&samples_dir, file);
                    let data = read_file(source, &wav_ref)?;
                    let (channels, frames) = wav_facts(&data).unwrap_or((0, 0));
                    locators.insert(
                        index,
                        Locator {
                            reference: wav_ref,
                            offset: 0,
                            bytes: data.len() as u64,
                            interleaved: false,
                        },
                    );
                    Some(Pcm {
                        channels,
                        frames,
                        bytes: data.len() as u64,
                        sha256: sha256_hex(&data),
                    })
                }
                None => None,
            };
            (Layout::Mti { wav }, pcm)
        } else {
            let data = &bytes[head.len()..];
            let frames = head.sample_length();
            let (channels, frames) = if frames as u64 * 4 == data.len() as u64 {
                (2, frames)
            } else if frames as u64 * 2 == data.len() as u64 {
                (1, frames)
            } else {
                // As the firmware does: a length that matches neither is taken as mono, from the
                // data.
                (1, (data.len() / 2) as u32)
            };
            locators.insert(
                index,
                Locator {
                    reference,
                    offset: head.len() as u64,
                    bytes: data.len() as u64,
                    interleaved: false,
                },
            );
            let pcm = Pcm {
                channels,
                frames,
                bytes: data.len() as u64,
                sha256: sha256_hex(data),
            };
            (Layout::Pti, Some(pcm))
        };
        instruments.insert(
            index,
            Instrument {
                file: name,
                head,
                layout,
                pcm,
            },
        );
    }
    if let Some(stray) = sample_files.iter().find(|s| !named_samples.contains(s)) {
        return Err(Error::Unreadable {
            reference: join(&samples_dir, stray),
            why: "a sample no instrument names, which a conversion would leave behind".to_owned(),
        });
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

/// The samples a locator points at, in the Tracker's order.
pub fn samples(source: &dyn Source, locator: &Locator) -> Result<Vec<u8>, Error> {
    let bytes = read_file(source, &locator.reference)?;
    let start = locator.offset as usize;
    let end = start + locator.bytes as usize;
    let found = bytes.get(start..end).ok_or_else(|| Error::Unreadable {
        reference: locator.reference.clone(),
        why: "shorter than its samples".to_owned(),
    })?;
    Ok(if locator.interleaved {
        planar(found)
    } else {
        found.to_vec()
    })
}

/// Stereo 16-bit frames, left and right one after the other, as all of the left then all of
/// the right.
pub fn planar(interleaved: &[u8]) -> Vec<u8> {
    let frames = interleaved.len() / 4;
    let mut out = vec![0u8; frames * 4];
    for f in 0..frames {
        out[2 * f..2 * f + 2].copy_from_slice(&interleaved[4 * f..4 * f + 2]);
        out[2 * (frames + f)..2 * (frames + f) + 2]
            .copy_from_slice(&interleaved[4 * f + 2..4 * f + 4]);
    }
    out
}

/// All of the left then all of the right, as stereo frames.
pub fn interleave(planar: &[u8]) -> Vec<u8> {
    let frames = planar.len() / 4;
    let mut out = vec![0u8; frames * 4];
    for f in 0..frames {
        out[4 * f..4 * f + 2].copy_from_slice(&planar[2 * f..2 * f + 2]);
        out[4 * f + 2..4 * f + 4].copy_from_slice(&planar[2 * (frames + f)..2 * (frames + f) + 2]);
    }
    out
}

impl Model {
    /// Every file of the project, written back: by its path inside the project folder, the
    /// instruments' samples taken through `locators` from `source`.
    pub fn files(
        &self,
        source: &dyn Source,
        locators: &Locators,
    ) -> Result<BTreeMap<String, Vec<u8>>, Error> {
        let mut out = BTreeMap::new();
        out.insert("project.mt".to_owned(), self.project.to_bytes());
        for (number, pattern) in &self.patterns {
            let name = format!("patterns/pattern_{number:02}.mtp");
            let bytes = pattern.to_bytes().map_err(unreadable(&name))?;
            out.insert(name, bytes);
        }
        if let Some(metadata) = &self.metadata {
            out.insert("patterns/patternsMetadata".to_owned(), metadata.clone());
        }
        for (index, instrument) in &self.instruments {
            let mut bytes = instrument.head.to_bytes();
            let located = || {
                locators.get(index).ok_or_else(|| Error::Unreadable {
                    reference: instrument.file.clone(),
                    why: "no samples located".to_owned(),
                })
            };
            match &instrument.layout {
                Layout::Pti => bytes.extend(samples(source, located()?)?),
                Layout::Mti { wav: Some(wav) } => {
                    out.insert(format!("samples/{wav}"), samples(source, located()?)?);
                }
                Layout::Mti { wav: None } => {}
            }
            out.insert(format!("instruments/{}", instrument.file), bytes);
        }
        Ok(out)
    }
}
