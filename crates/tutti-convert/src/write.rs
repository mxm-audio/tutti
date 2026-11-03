use std::collections::BTreeSet;
use std::io::Write as _;

use tutti::folder::{FolderError, Kind, Store, referenced_files};
use tutti::names::clean_name;
use tutti::project::DOCUMENT_EXTENSION;
use tutti::render::sha256_hex;
use tutti::validate::{check_device_file_path, is_file_name};
use tutti::{AudioPiece, Device, Document, Event, Generator, Project, Saved, Track};

use crate::{Contents, Conversion, Error, Export, Place, Progress, Source};

/// The one folder of a store a conversion may find already written: a program's own (spec 2.3).
const PRIVATE: &str = "private";

/// Writes a conversion into a store: every file put, in the order given, then
/// `Project::save`, the procedure every writer follows (which indexes and verifies the files,
/// writes the README, and refuses a document that refers to a file nobody put).
///
/// Before anything is written, each refusal an [`Error::Placement`] unless said otherwise:
/// - the store holds no document and no file outside `private/` ([`Error::OutputNotEmpty`]);
///   a watcher may have written its own files there first, and they are left as they are;
/// - no two files go to one place (compared ignoring case, as Windows and macOS do);
/// - an audio file's name is one the core keeps as it is, and the document refers to it;
/// - a device file's device is in the document, and its path is a safe one;
/// - a state is one a device of the document refers to, by its SHA-256 and kind.
///
/// `source` gives the files a [`Contents::Copy`] streams: a [`Recording`](crate::Recording)
/// notes them with the rest. [`Progress::cancelled`] is asked before each file; a cancelled
/// write leaves what it wrote, for the caller to throw away with its staging folder.
pub fn write<S: Store>(
    conversion: Conversion,
    source: &dyn Source,
    store: S,
    generator: &Generator,
    progress: &mut dyn Progress,
) -> Result<Saved, Error> {
    let Conversion {
        name,
        document,
        files,
        report: _,
    } = conversion;

    if let Some(reference) = file_outside_private(&store, "")? {
        return Err(Error::OutputNotEmpty(reference));
    }

    // Every check, and every state's bytes (states are small), before anything is written.
    let referenced: BTreeSet<String> = referenced_files(&document).into_iter().collect();
    let mut places = BTreeSet::new();
    let mut states: Vec<Option<Vec<u8>>> = Vec::with_capacity(files.len());
    for put in &files {
        let (destination, state) = match &put.place {
            Place::Audio { file } => {
                let destination = format!("audio/{file}");
                let kept = file
                    .rsplit_once('.')
                    .is_some_and(|(stem, _)| clean_name(stem, "Untitled") == stem);
                if !is_file_name(file) || !kept {
                    return Err(Error::placement(
                        &destination,
                        "a name the project would change: give it as the core cleans names",
                    ));
                }
                if !referenced.contains(&destination) {
                    return Err(Error::placement(
                        &destination,
                        "no audio piece of the document refers to it",
                    ));
                }
                (destination, None)
            }
            Place::DeviceFile { device, path } => {
                let destination = format!("plugins/files/{}/{path}", device.as_str());
                if !devices(&document).any(|d| d.id == *device) {
                    return Err(Error::placement(
                        &destination,
                        "the document has no such device",
                    ));
                }
                if let Err(reason) = check_device_file_path(path) {
                    return Err(Error::placement(&destination, reason));
                }
                (destination, None)
            }
            Place::State { kind } => {
                let bytes = match &put.contents {
                    Contents::Bytes(bytes) => bytes.clone(),
                    Contents::Copy { from } => source.read(from).map_err(Error::source(from))?,
                };
                let sha256 = sha256_hex(&bytes);
                let destination = format!("plugins/states/{sha256}.{kind}");
                let used = devices(&document).any(|d| {
                    d.state
                        .as_ref()
                        .is_some_and(|s| s.sha256 == sha256 && s.kind == *kind)
                });
                if !used {
                    return Err(Error::placement(
                        &destination,
                        "no device of the document refers to this state",
                    ));
                }
                (destination, Some(bytes))
            }
        };
        if !places.insert(destination.to_lowercase()) {
            return Err(Error::placement(&destination, "two files go to this place"));
        }
        states.push(state);
    }

    let document_name = format!("{}.{DOCUMENT_EXTENSION}", clean_name(&name, "Project"));
    let pieces: Vec<AudioPiece> = audio_pieces(&document).cloned().collect();
    store.make_folder("").map_err(Error::store(""))?;
    let mut project = Project::adopt(store, &document_name, document)?;

    let total = files.len() as u64 + 1;
    for (i, (put, state)) in files.into_iter().zip(states).enumerate() {
        if progress.cancelled() {
            return Err(Error::Cancelled);
        }
        match put.place {
            Place::Audio { file } => {
                let destination = format!("audio/{file}");
                progress.step(i as u64, total, &destination);
                let (stem, extension) = file.rsplit_once('.').expect("checked above");
                let mut incoming = project.new_audio(stem, extension)?;
                if incoming.reference() != destination {
                    return Err(Error::placement(
                        &destination,
                        format!("the project named it {}", incoming.reference()),
                    ));
                }
                match &put.contents {
                    Contents::Bytes(bytes) => {
                        incoming.write_all(bytes).map_err(|e| Error::Store {
                            reference: destination.clone(),
                            error: FolderError::Other(e.to_string()),
                        })?
                    }
                    Contents::Copy { from } => {
                        source
                            .copy_to(from, &mut incoming)
                            .map_err(Error::source(from))?;
                    }
                }
                let stored = incoming.finish()?;
                if let Some(piece) = pieces
                    .iter()
                    .find(|p| p.file == destination && p.sha256 != stored.sha256)
                {
                    return Err(Error::placement(
                        &destination,
                        format!(
                            "the document gives it the SHA-256 {}, its bytes have {}",
                            piece.sha256, stored.sha256
                        ),
                    ));
                }
            }
            Place::DeviceFile { device, path } => {
                let destination = format!("plugins/files/{}/{path}", device.as_str());
                progress.step(i as u64, total, &destination);
                let bytes = match &put.contents {
                    Contents::Bytes(bytes) => bytes.clone(),
                    Contents::Copy { from } => source.read(from).map_err(Error::source(from))?,
                };
                project.device_files(device.as_str())?.put(&path, &bytes)?;
            }
            Place::State { kind } => {
                let bytes = state.expect("read above");
                progress.step(i as u64, total, &format!("a {kind} state"));
                project.put_state(&kind, &bytes)?;
            }
        }
    }

    if progress.cancelled() {
        return Err(Error::Cancelled);
    }
    progress.step(total - 1, total, &document_name);
    let saved = project.save(generator)?;
    progress.step(total, total, &document_name);
    Ok(saved)
}

/// Writes an export's files into a store, each new (a file already there is refused, nothing
/// is overwritten), in the order given. Each path is a safe relative one, and no two go to one
/// place; both checked before anything is written. `project` gives the files a
/// [`Contents::Copy`] streams.
pub fn write_export<S: Store>(export: Export, project: &dyn Source, store: S) -> Result<(), Error> {
    let mut places = BTreeSet::new();
    for (path, _) in &export.files {
        if let Err(reason) = tutti::zip::check_entry_name(path) {
            return Err(Error::placement(path, reason));
        }
        if !places.insert(path.to_lowercase()) {
            return Err(Error::placement(path, "two files go to this place"));
        }
    }
    for (path, contents) in export.files {
        match contents {
            Contents::Bytes(bytes) => store
                .write_new(&path, &bytes)
                .map_err(Error::store(&path))?,
            Contents::Copy { from } => {
                let mut writer = store.open_new(&path).map_err(Error::store(&path))?;
                project
                    .copy_to(&from, &mut writer)
                    .map_err(Error::source(&from))?;
                store.close(writer).map_err(Error::store(&path))?;
            }
        }
    }
    Ok(())
}

/// The first file under `folder`, `private/` aside, walking folders; none in a store that is
/// not there yet.
fn file_outside_private<S: Store>(store: &S, folder: &str) -> Result<Option<String>, Error> {
    let entries = match Store::list(store, folder) {
        Ok(entries) => entries,
        Err(FolderError::Missing) => return Ok(None),
        Err(e) => return Err(Error::store(folder)(e)),
    };
    let mut entries = entries;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    for entry in entries {
        let reference = if folder.is_empty() {
            entry.name.clone()
        } else {
            format!("{folder}/{}", entry.name)
        };
        if folder.is_empty() && entry.name == PRIVATE && entry.kind == Kind::Folder {
            continue;
        }
        match entry.kind {
            Kind::Folder => {
                if let Some(found) = file_outside_private(store, &reference)? {
                    return Ok(Some(found));
                }
            }
            Kind::File | Kind::Other => return Ok(Some(reference)),
        }
    }
    Ok(None)
}

/// Every device of the document: the master's and every track's, folders walked.
fn devices(document: &Document) -> impl Iterator<Item = &Device> {
    document
        .master
        .devices
        .iter()
        .chain(tracks(&document.tracks).flat_map(|t| t.devices.iter()))
}

/// Every audio piece of the document's notes.
fn audio_pieces(document: &Document) -> impl Iterator<Item = &AudioPiece> {
    tracks(&document.tracks)
        .flat_map(|t| t.patterns.iter())
        .flat_map(|p| p.events.iter())
        .filter_map(|e| match e {
            Event::Note(n) => n.audio.as_ref(),
            _ => None,
        })
}

/// Every track, folders' members after their folder.
fn tracks(top: &[Track]) -> impl Iterator<Item = &Track> {
    let mut out = Vec::new();
    let mut stack: Vec<&Track> = top.iter().rev().collect();
    while let Some(track) = stack.pop() {
        out.push(track);
        stack.extend(track.children.iter().rev());
    }
    out.into_iter()
}
