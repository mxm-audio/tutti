//! The command over the examples: `check` accepts every example and refuses every hostile
//! fixture with its named error; `inspect`, `deps`, `archive` and `fmt` do what they say.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tutti(args: &[&str], folder: &Path) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_tutti"))
        .args(args)
        .arg(folder)
        .output()
        .expect("the command runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn folders_under(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    out.sort();
    out
}

#[test]
fn check_accepts_every_example_and_says_what_it_uses() {
    let examples = repository_root().join("examples");
    for folder in [
        examples.join("minimal"),
        examples.join("typical"),
        examples.join("every-feature"),
    ]
    .into_iter()
    .chain(folders_under(&examples.join("conformance/l0")))
    {
        let (code, out, err) = tutti(&["check"], &folder);
        assert_eq!(code, 0, "{}: {err}", folder.display());
        assert!(
            out.contains("valid (0 errors"),
            "{}: {out}",
            folder.display()
        );
    }
    let (_, out, _) = tutti(&["check"], &examples.join("every-feature"));
    assert!(out.contains("a reader needs Level 2"), "{out}");
    let (_, out, _) = tutti(&["check"], &examples.join("minimal"));
    assert!(out.contains("features: none"), "{out}");
}

/// The project as one file in a hostile fixture's folder, if it is one.
fn zip_in(folder: &Path) -> Option<PathBuf> {
    fs::read_dir(folder)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| {
            p.file_name()
                .is_some_and(|n| tutti::zip::is_zip_name(&n.to_string_lossy()))
        })
}

#[test]
fn check_refuses_every_hostile_fixture_with_its_named_error() {
    for folder in folders_under(&repository_root().join("examples/conformance/hostile")) {
        let expected: serde_json::Value =
            serde_json::from_slice(&fs::read(folder.join("expected.json")).unwrap()).unwrap();
        let want = expected["error"].as_str().unwrap();
        // A project as one file is refused by `unzip`, and nothing is unpacked.
        let (code, _, err) = match zip_in(&folder) {
            Some(zip) => {
                let result = tutti(&["unzip"], &zip);
                let name = tutti::zip::project_name(&zip.file_name().unwrap().to_string_lossy())
                    .unwrap()
                    .to_owned();
                assert!(
                    !folder.join(name).exists(),
                    "{}: unpacked",
                    folder.display()
                );
                result
            }
            None => tutti(&["check"], &folder),
        };
        assert_eq!(code, 1, "{}: exit {code}; {err}", folder.display());
        assert!(
            err.contains(want),
            "{}: expected {want:?} in {err:?}",
            folder.display()
        );
    }
}

#[test]
fn usage_is_exit_2() {
    let output = Command::new(env!("CARGO_BIN_EXE_tutti")).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let output = Command::new(env!("CARGO_BIN_EXE_tutti"))
        .arg("dance")
        .arg(".")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn inspect_and_deps_describe_the_typical_project() {
    let folder = repository_root().join("examples/typical");
    let (code, out, _) = tutti(&["inspect"], &folder);
    assert_eq!(code, 0);
    for expected in [
        "title: Typical",
        "ppq 960, tempo 87",
        "section Verse at 7680 for 15360",
        "Bass (trk_",
        "mxm-mono-01 [clap:dk.mxm.mxm-mono-01]",
        "1 send(s)",
    ] {
        assert!(out.contains(expected), "{expected:?} not in {out}");
    }
    let (code, out, _) = tutti(&["deps"], &folder);
    assert_eq!(code, 0);
    assert!(
        out.contains("mxm-mono-01 MXM 0.1.1 (clap:dk.mxm.mxm-mono-01)"),
        "{out}"
    );
    assert!(
        out.contains("source: https://github.com/mxm-audio/mxm-mono-01.git"),
        "{out}"
    );
    let (code, out, _) = tutti(&["deps"], &repository_root().join("examples/minimal"));
    assert_eq!(code, 0);
    assert!(out.contains("no dependencies"), "{out}");
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().filter_map(Result::ok) {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn zip_and_unzip_round_trip_a_project() {
    let temp = std::env::temp_dir().join(format!("tutti-cli-zip-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp);
    // A copy of the typical example, so the file lands in a folder of the test's.
    let source = temp.join("src").join("typical");
    copy_dir(&repository_root().join("examples/typical"), &source);
    let (code, out, err) = tutti(&["zip"], &source);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("entries"), "{out}");
    let zip = temp.join("src").join("typical.tutti.zip");
    assert!(zip.is_file());
    // Zipping again overwrites nothing.
    let (code, _, err) = tutti(&["zip"], &source);
    assert_eq!(code, 1);
    assert!(err.contains("is there already"), "{err}");

    fs::create_dir_all(temp.join("dst")).unwrap();
    let moved = temp.join("dst").join("typical.tutti.zip");
    fs::rename(&zip, &moved).unwrap();
    let (code, out, err) = tutti(&["unzip"], &moved);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("files"), "{out}");
    let unpacked = temp.join("dst").join("typical");
    let (code, out, err) = tutti(&["check"], &unpacked);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("valid (0 errors"), "{out}");
    for name in ["Typical.tutti", "README.md", "audio/Click-loop_95.wav"] {
        assert_eq!(
            fs::read(unpacked.join(name)).unwrap(),
            fs::read(source.join(name)).unwrap(),
            "{name}"
        );
    }
    // Unpacking again beside the file lands on the folder it made: refused.
    let (code, _, err) = tutti(&["unzip"], &moved);
    assert_eq!(code, 1);
    assert!(err.contains("holds a project already"), "{err}");
    let _ = fs::remove_dir_all(&temp);
}

#[test]
fn archive_and_fmt_work_on_a_copy() {
    let source = repository_root().join("examples/minimal");
    let temp = std::env::temp_dir().join(format!("tutti-cli-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).unwrap();
    for entry in fs::read_dir(&source).unwrap().filter_map(Result::ok) {
        if entry.path().is_file() {
            fs::copy(entry.path(), temp.join(entry.file_name())).unwrap();
        }
    }
    let (code, out, _) = tutti(&["archive"], &temp);
    assert_eq!(code, 0, "{out}");
    assert!(temp.join("spec/tutti.md").is_file() && temp.join("spec/tutti.schema.json").is_file());
    // A document written in any form comes back in the pretty form.
    let document = temp.join("Minimal.tutti");
    let original = fs::read_to_string(&document).unwrap();
    let scrambled: serde_json::Value = serde_json::from_str(&original).unwrap();
    fs::write(&document, serde_json::to_string(&scrambled).unwrap()).unwrap();
    let (code, _, err) = tutti(&["fmt"], &temp);
    assert_eq!(code, 0, "{err}");
    assert_eq!(fs::read_to_string(&document).unwrap(), original);
    let _ = fs::remove_dir_all(&temp);
}
