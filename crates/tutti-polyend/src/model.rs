//! A Polyend Tracker project as its files hold it ([`Model`]), and where each instrument's
//! samples are found ([`Locators`], kept beside the model, so two models compare equal whatever
//! source they were read from).

use std::collections::BTreeMap;

use tutti::folder::{FolderError, Kind};
use tutti::render::sha256_hex;
use tutti_convert::{Error, Found, Source};

use crate::files::{InstrumentFile, PCM_OFFSET, PatternFile, ProjectFile, Unreadable};

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

/// One instrument: its file's name, everything before its samples, and its samples described.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instrument {
    /// As on the card, `1 Kick.pti`.
    pub file: String,
    pub head: InstrumentFile,
    pub pcm: Pcm,
}

impl Instrument {
    /// The name the device shows: the file's name without its number and extension.
    pub fn name(&self) -> String {
        let stem = self.file.strip_suffix(".pti").unwrap_or(&self.file);
        stem.split_once(' ')
            .map(|(_, n)| n)
            .unwrap_or(stem)
            .trim()
            .to_owned()
    }
}

/// An instrument's samples: 16-bit signed little-endian at 44 100 Hz, a stereo sample's channels
/// one after the other (all of the left, then all of the right), as the Tracker stores them.
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
    if let Some(old) = instrument_files.iter().find(|n| n.ends_with(".mti")) {
        let reference = join(&instruments_dir, old);
        return Err(Error::Unreadable {
            reference,
            why: "an instrument of the older layout (.mti), which this converter does not read"
                .to_owned(),
        });
    }
    let samples_dir = join(at, "samples");
    if let Some(old) = files_in(source, &samples_dir)?.first() {
        return Err(Error::Unreadable {
            reference: join(&samples_dir, old),
            why: "a sample of the older layout (samples/), which this converter does not read"
                .to_owned(),
        });
    }

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
    for name in instrument_files {
        let Some(index) = instrument_index(&name) else {
            continue;
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
        let data = &bytes[PCM_OFFSET..];
        let frames = head.sample_length();
        let (channels, frames) = if frames as u64 * 4 == data.len() as u64 {
            (2, frames)
        } else if frames as u64 * 2 == data.len() as u64 {
            (1, frames)
        } else {
            // As the firmware does: a length that matches neither is taken as mono, from the data.
            (1, (data.len() / 2) as u32)
        };
        let pcm = Pcm {
            channels,
            frames,
            bytes: data.len() as u64,
            sha256: sha256_hex(data),
        };
        locators.insert(
            index,
            Locator {
                reference,
                offset: PCM_OFFSET as u64,
                bytes: data.len() as u64,
                interleaved: false,
            },
        );
        instruments.insert(
            index,
            Instrument {
                file: name,
                head,
                pcm,
            },
        );
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
            let locator = locators.get(index).ok_or_else(|| Error::Unreadable {
                reference: instrument.file.clone(),
                why: "no samples located".to_owned(),
            })?;
            let mut bytes = instrument.head.to_bytes();
            bytes.extend(samples(source, locator)?);
            out.insert(format!("instruments/{}", instrument.file), bytes);
        }
        Ok(out)
    }
}
