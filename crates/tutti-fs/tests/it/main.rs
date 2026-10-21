//! The disk: a project created, filled, saved and reopened; links refused; a failure at any
//! step of a save leaving the old document or the new one, never a torn one.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tutti::folder::{Entry, Folder, FolderError, Store};
use tutti::{
    AudioPiece, Event, Generator, Id, MapPoint, Note, Pattern, Project, ProjectError, Track,
    Unknown, equal,
};
use tutti_fs::DirStore;

fn generator() -> Generator {
    Generator::new(
        "tutti-fs-tests",
        "0.1",
        "https://github.com/mxm-audio/tutti",
    )
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tutti-fs-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn track_with_piece(reference: &str, sha256: &str) -> Track {
    let mut track = Track::new(Id::new("trk_1").unwrap(), "Vox");
    let mut pattern = Pattern {
        id: Id::new("pat_1").unwrap(),
        name: String::new(),
        length: 960,
        key: None,
        events: Vec::new(),
        extensions: Default::default(),
        unknown: Unknown::new(),
    };
    pattern.events.push(Event::Note(Note {
        id: Id::new("evt_1").unwrap(),
        at: 0,
        length: 960,
        pitch: 60,
        velocity: 0.8,
        release: 0.5,
        expressions: None,
        audio: Some(AudioPiece {
            file: reference.to_owned(),
            sha256: sha256.to_owned(),
            sample_rate: 48_000,
            channels: 1,
            frames: 4,
            name: None,
            from: 0,
            to: 4,
            map: vec![MapPoint {
                frame: 0,
                tick: 0,
                unknown: Unknown::new(),
            }],
            bpm: None,
            gain: 1.0,
            polarity: false,
            channel_mode: "as_recorded".to_owned(),
            fade_in: None,
            fade_out: None,
            reverse: false,
            extensions: Default::default(),
            unknown: Unknown::new(),
        }),
        extensions: Default::default(),
        unknown: Unknown::new(),
    }));
    track.patterns.push(pattern);
    track
}

#[test]
fn a_new_id_is_the_formats_and_never_repeats() {
    let ids: Vec<tutti::Id> = (0..1000).map(|_| tutti_fs::ulid("evt")).collect();
    for id in &ids {
        assert!(tutti::Id::new(id.as_str()).is_ok(), "{id:?}");
        assert!(id.as_str().starts_with("evt_"));
    }
    let mut unique: Vec<&str> = ids.iter().map(tutti::Id::as_str).collect();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), ids.len());
}

#[test]
fn a_project_on_disk_is_created_filled_saved_and_reopened() {
    let dir = temp("round-trip");
    let mut project = tutti_fs::create(&dir, "Min sang", &generator()).unwrap();
    assert!(dir.join("Min-sang.tutti").is_file());
    assert!(dir.join("README.md").is_file());
    assert!(dir.join("audio").is_dir() && dir.join("plugins/states").is_dir());

    // A recording streams through a handle that owns a clone of the store: on another thread.
    let mut take = project.new_audio("Take 1", "wav").unwrap();
    let writer = std::thread::spawn(move || {
        take.write_all(b"RI").unwrap();
        take.write_all(b"FF").unwrap();
        take.finish().unwrap()
    });
    let stored = writer.join().unwrap();
    assert_eq!(stored.reference, "audio/Take-1.wav");
    assert_eq!(fs::read(dir.join("audio/Take-1.wav")).unwrap(), b"RIFF");
    let state = project.put_state("clap", b"state bytes").unwrap();
    assert!(dir.join(state.path()).is_file());
    project
        .document_mut()
        .tracks
        .push(track_with_piece(&stored.reference, &stored.sha256));
    let saved = project.save(&generator()).unwrap();
    assert!(saved.warnings.is_empty(), "{:?}", saved.warnings);
    assert!(!dir.join("Min-sang.tutti.tmp").exists());

    let opened = tutti_fs::open(&dir).unwrap();
    assert!(opened.report.is_valid() && !opened.read_only);
    assert!(equal(opened.project.document(), project.document()));
    assert!(opened.project.check().is_valid());
    assert_eq!(opened.project.document().files.len(), 1);
    assert_eq!(
        tutti_fs::document_in(&dir).unwrap(),
        dir.join("Min-sang.tutti")
    );

    // A copy on disk is complete.
    let copy_dir = temp("round-trip-copy");
    let copy = project.copy_into(DirStore::new(&copy_dir)).unwrap();
    assert!(copy.check().is_valid());
    assert_eq!(
        fs::read(copy_dir.join("audio/Take-1.wav")).unwrap(),
        b"RIFF"
    );
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&copy_dir);
}

#[test]
fn a_link_in_place_of_a_file_is_refused() {
    let dir = temp("link");
    let project = tutti_fs::create(&dir, "Linked", &generator()).unwrap();
    let outside = dir.join("outside.wav");
    fs::write(&outside, b"RIFF").unwrap();
    let link = dir.join("audio/link.wav");
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file(&outside, &link).is_ok();
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(&outside, &link).is_ok();
    if !made {
        // Making a link needs a privilege this machine may not give a test: nothing to check.
        let _ = fs::remove_dir_all(&dir);
        return;
    }
    let store = project.store();
    assert_eq!(store.exists("audio/link.wav"), Err(FolderError::Link));
    assert_eq!(store.read("audio/link.wav"), Err(FolderError::Link));
    assert!(matches!(
        project.read("audio/link.wav"),
        Err(ProjectError::Store {
            error: FolderError::Link,
            ..
        })
    ));
    assert!(matches!(
        store.path_of("../outside.wav"),
        Err(FolderError::Other(_))
    ));
    let _ = fs::remove_dir_all(&dir);
}

/// A store that fails, as a crash would, at the n-th operation that changes something.
#[derive(Clone)]
struct Failing {
    inner: DirStore,
    fail_at: usize,
    count: Arc<AtomicUsize>,
}

impl Failing {
    fn step(&self) -> Result<(), FolderError> {
        if self.count.fetch_add(1, Ordering::SeqCst) == self.fail_at {
            Err(FolderError::Other("crash".to_owned()))
        } else {
            Ok(())
        }
    }
}

impl Folder for Failing {
    fn read(&self, reference: &str) -> Result<Vec<u8>, FolderError> {
        self.inner.read(reference)
    }

    fn size(&self, reference: &str) -> Result<u64, FolderError> {
        self.inner.size(reference)
    }

    fn sha256(&self, reference: &str) -> Result<(String, u64), FolderError> {
        self.inner.sha256(reference)
    }

    fn modified(&self, reference: &str) -> Result<Option<u64>, FolderError> {
        self.inner.modified(reference)
    }
}

impl Store for Failing {
    type Writer = <DirStore as Store>::Writer;

    fn list(&self, folder: &str) -> Result<Vec<Entry>, FolderError> {
        self.inner.list(folder)
    }

    fn exists(&self, reference: &str) -> Result<bool, FolderError> {
        self.inner.exists(reference)
    }

    fn make_folder(&self, folder: &str) -> Result<(), FolderError> {
        self.step()?;
        self.inner.make_folder(folder)
    }

    fn write_new(&self, reference: &str, bytes: &[u8]) -> Result<(), FolderError> {
        self.step()?;
        self.inner.write_new(reference, bytes)
    }

    fn open_new(&self, reference: &str) -> Result<Self::Writer, FolderError> {
        self.step()?;
        self.inner.open_new(reference)
    }

    fn close(&self, writer: Self::Writer) -> Result<(), FolderError> {
        self.step()?;
        self.inner.close(writer)
    }

    fn rename_over(&self, from: &str, to: &str) -> Result<(), FolderError> {
        self.step()?;
        self.inner.rename_over(from, to)
    }

    fn sync_folder(&self, folder: &str) -> Result<(), FolderError> {
        self.step()?;
        self.inner.sync_folder(folder)
    }

    fn remove(&self, reference: &str) -> Result<(), FolderError> {
        self.step()?;
        self.inner.remove(reference)
    }
}

fn tempo_of(folder: &Path) -> f64 {
    tutti_fs::open(folder).unwrap().project.document().tempo[0].bpm
}

#[test]
fn a_failure_at_any_step_of_a_save_leaves_the_old_document_or_the_new() {
    let dir = temp("crash");
    let mut project = tutti_fs::create(&dir, "Crash", &generator()).unwrap();
    let stored = project.put_audio("Loop", "wav", b"RIFF").unwrap();
    project
        .document_mut()
        .tracks
        .push(track_with_piece(&stored.reference, &stored.sha256));
    project.save(&generator()).unwrap();
    drop(stored);
    assert_eq!(tempo_of(&dir), 120.0);

    let mut steps = 0;
    for fail_at in 0..32 {
        let count = Arc::new(AtomicUsize::new(0));
        let store = Failing {
            inner: DirStore::new(&dir),
            fail_at,
            count: count.clone(),
        };
        let mut project = Project::open(store).unwrap().project;
        project.document_mut().tempo[0].bpm = 100.0 + fail_at as f64;
        let result = project.save(&generator());
        let after = tempo_of(&dir);
        let reopened = tutti_fs::open(&dir).unwrap();
        assert!(
            reopened.project.check().is_valid(),
            "after failing at {fail_at}"
        );
        match result {
            Ok(_) => {
                assert_eq!(after, 100.0 + fail_at as f64);
                steps = count.load(Ordering::SeqCst);
                break;
            }
            Err(e) => {
                assert!(
                    after == 120.0 || after == 100.0 + fail_at as f64,
                    "failing at {fail_at} ({e}) left tempo {after}"
                );
                // A failure after the commit point says so, and the hash is the new one.
                let committed = after == 100.0 + fail_at as f64;
                assert_eq!(
                    matches!(e, ProjectError::Unconfirmed(_)),
                    committed,
                    "failing at {fail_at}: {e}"
                );
                // The next save starts from whatever is there: the new one or the old.
                let mut again = tutti_fs::open(&dir).unwrap().project;
                again.document_mut().tempo[0].bpm = 120.0;
                again.save(&generator()).unwrap();
                assert_eq!(tempo_of(&dir), 120.0);
            }
        }
    }
    assert!(
        steps > 0 && steps < 32,
        "the save succeeded after {steps} steps"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn two_documents_and_a_missing_folder_are_refused_by_name() {
    let dir = temp("two");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("A.tutti"), b"{}").unwrap();
    fs::write(dir.join("B.tutti"), b"{}").unwrap();
    let error = tutti_fs::open(&dir).unwrap_err().to_string();
    assert!(error.contains("2 documents (A.tutti, B.tutti)"), "{error}");
    let missing = dir.join("nowhere");
    let error = tutti_fs::open(&missing).unwrap_err().to_string();
    assert!(error.contains("the folder: missing"), "{error}");
    let _ = fs::remove_dir_all(&dir);
}
