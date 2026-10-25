//! The tests that hold the spec, the schema, the examples, the docs and the code together.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};
use tutti::folder::{Folder, MemoryStore, check_files};
use tutti::render::render_fingerprint;
use tutti::validate::{self, Severity};
use tutti::{Device, Generator, Override};
use tutti_fs::{DirStore, document_in};
use tutti_schema::{field_reference, ids_of, repository_root, schema, shared_patterns};

/// The conformance levels' folders, each with its level.
const LEVELS: [(u8, &str); 3] = [(0, "l0"), (1, "l1"), (2, "l2")];

/// The cases of one conformance level (`l0`, `l1`, `l2`, `hostile`), sorted.
fn cases_of(level: &str) -> Vec<PathBuf> {
    let dir = repository_root().join("examples/conformance").join(level);
    let mut cases: Vec<PathBuf> = match fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => Vec::new(),
    };
    cases.sort();
    cases
}

/// Every example project folder: the three examples and the conformance fixtures.
fn example_folders() -> Vec<PathBuf> {
    let examples = repository_root().join("examples");
    let mut out = vec![
        examples.join("minimal"),
        examples.join("typical"),
        examples.join("every-feature"),
    ];
    for (_, level) in LEVELS {
        out.extend(cases_of(level));
    }
    out
}

fn expected_of(folder: &Path) -> Value {
    serde_json::from_slice(&fs::read(folder.join("expected.json")).unwrap()).unwrap()
}

fn document_of(folder: &Path) -> tutti::Document {
    tutti::read(&fs::read(document_in(folder).unwrap()).unwrap()).unwrap()
}

/// A scratch folder of this test binary's, under the build folder.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
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

/// Makes `link` a link to the folder `target`, relative to the link's own folder: a symbolic
/// link where the system gives one, else, on Windows, a junction, which needs no privilege and
/// which a reader refuses alike (spec 9).
fn make_folder_link(link: &Path, target: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        if std::os::windows::fs::symlink_dir(target, link).is_ok() {
            return Ok(());
        }
        let absolute = link.parent().expect("a link in a folder").join(target);
        let status = Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(link)
            .arg(&absolute)
            .stdout(std::process::Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("mklink /J failed"))
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (link, target);
        Err(std::io::Error::other("no links on this system"))
    }
}

/// A hostile case that needs a link (its `expected.json`'s `link`), as a runner makes it: the
/// folder copied into a scratch folder, the link made in the copy.
fn linked_copy(folder: &Path, link: &Value, scratch: &Path) -> PathBuf {
    let into = scratch.join(folder.file_name().unwrap());
    copy_dir(folder, &into);
    let path = into.join(link["path"].as_str().unwrap());
    make_folder_link(&path, link["target"].as_str().unwrap())
        .unwrap_or_else(|e| panic!("{}: the link: {e}", path.display()));
    into
}

/// Every file directly in a folder, with its bytes.
fn files_in(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out: Vec<(PathBuf, Vec<u8>)> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .map(|p| {
            let bytes = fs::read(&p).unwrap();
            (p, bytes)
        })
        .collect();
    out.sort();
    out
}

fn schema_errors(validator: &jsonschema::Validator, instance: &Value) -> Vec<String> {
    validator
        .iter_errors(instance)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect()
}

#[test]
fn every_example_validates_reads_writes_back_byte_identical_and_checks_out() {
    let validator = jsonschema::validator_for(&schema()).expect("the schema compiles");
    for folder in example_folders() {
        let path = document_in(&folder).unwrap();
        let bytes = fs::read(&path).unwrap();
        let instance: Value = serde_json::from_slice(&bytes).unwrap();
        let errors = schema_errors(&validator, &instance);
        assert!(errors.is_empty(), "{}: {errors:#?}", path.display());
        let doc = tutti::read(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let written = tutti::write(&doc);
        assert_eq!(
            written.as_bytes(),
            bytes.as_slice(),
            "{}: not the pretty form",
            path.display()
        );
        let report = validate::validate(&doc);
        let errors: Vec<String> = report.errors().map(|i| i.to_string()).collect();
        assert!(errors.is_empty(), "{}: {errors:#?}", path.display());
        let file_issues = check_files(&doc, &DirStore::new(&folder));
        let file_errors: Vec<String> = file_issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .map(|i| i.to_string())
            .collect();
        assert!(
            file_errors.is_empty(),
            "{}: {file_errors:#?}",
            path.display()
        );
        // README.md is the template filled in.
        let readme = fs::read_to_string(folder.join("README.md")).unwrap();
        assert!(
            readme.contains(tutti::SPEC_URL) && !readme.contains("{title}"),
            "{}: README",
            folder.display()
        );
    }
}

#[test]
fn every_hostile_fixture_is_refused_as_its_expectation_says() {
    let cases = cases_of("hostile");
    assert!(!cases.is_empty());
    let links = scratch("hostile-links");
    for folder in cases {
        let expected = expected_of(&folder);
        let want = expected["error"].as_str().unwrap();
        // A case with a link is checked where the link is made; what the link reaches stays
        // as it was.
        let (at, reached) = match expected.get("link") {
            Some(link) => {
                let at = linked_copy(&folder, link, &links);
                let target = at.join(link["target"].as_str().unwrap());
                let before = files_in(&target);
                (at, Some((target, before)))
            }
            None => (folder.clone(), None),
        };
        let actual = refusal(&at);
        assert!(
            actual.contains(want),
            "{}: expected {want:?}, got {actual:?}",
            folder.display()
        );
        if let Some((target, before)) = reached {
            assert_eq!(files_in(&target), before, "{}", target.display());
        }
    }
}

#[test]
fn every_fixture_needs_the_level_it_is_filed_under() {
    for (level, dir) in LEVELS {
        for folder in cases_of(dir) {
            let needed = tutti::features::level_needed(&document_of(&folder));
            // Level 2 is about writing, so its cases may play at a lower level.
            if level < 2 {
                assert_eq!(needed, level, "{}", folder.display());
            } else {
                assert!(needed <= level, "{}: Level {needed}", folder.display());
            }
        }
    }
}

/// Every object in a table that gives a tick and its seconds.
fn timed_rows<'a>(value: &'a Value, out: &mut Vec<&'a Value>) {
    match value {
        Value::Object(map) => {
            if map.get("tick").is_some_and(Value::is_number)
                && map.get("seconds").is_some_and(Value::is_number)
            {
                out.push(value);
            }
            map.values().for_each(|v| timed_rows(v, out));
        }
        Value::Array(items) => items.iter().for_each(|v| timed_rows(v, out)),
        _ => {}
    }
}

#[test]
fn the_expected_tables_times_follow_the_documents_tempo_map() {
    for (_, dir) in LEVELS {
        for folder in cases_of(dir) {
            let doc = document_of(&folder);
            let map = tutti::time::TempoMap::new(doc.ppq, &doc.tempo);
            let expected = expected_of(&folder);
            let mut rows = Vec::new();
            timed_rows(&expected, &mut rows);
            for row in rows {
                let tick = row["tick"].as_f64().unwrap();
                let seconds = row["seconds"].as_f64().unwrap();
                assert!(
                    (map.seconds_at(tick) - seconds).abs() < 1e-9,
                    "{}: tick {tick} is {} s, not {seconds}",
                    folder.display(),
                    map.seconds_at(tick)
                );
                if let Some(sample) = row.get("sample_48000") {
                    assert_eq!(
                        sample.as_i64(),
                        Some(map.samples_at(tick, 48_000)),
                        "{}: tick {tick}",
                        folder.display()
                    );
                }
            }
        }
    }
}

fn level_case(path: &str) -> PathBuf {
    repository_root().join("examples/conformance").join(path)
}

fn another_writer() -> Generator {
    Generator::new(
        "another-writer",
        "2.0",
        "https://example.org/another-writer",
    )
}

#[test]
fn level_1_missing_plugin_keeps_its_device_and_takes_an_edit_into_overrides_only() {
    let folder = level_case("l1/missing-plugin");
    let expected = expected_of(&folder);
    let opened = tutti_fs::open(&folder).unwrap();
    let doc = opened.project.document().clone();
    let device_id = expected["edit"]["device"].as_str().unwrap();
    let track = doc
        .all_tracks()
        .into_iter()
        .find(|t| t.devices.iter().any(|d| d.id.as_str() == device_id))
        .expect("the device's track")
        .clone();
    // The render is current: its fingerprint is the track's (spec 7.7).
    let render = track.renders[0].clone();
    assert_eq!(
        render_fingerprint(&doc, &track, render.from, render.to),
        render.fingerprint
    );
    // What the reader names the missing plugin by: its dependencies entry (7.6).
    let dependency = doc
        .dependencies
        .iter()
        .find(|d| d.id == expected["plugin"].as_str().unwrap())
        .expect("the plugin's entry");
    assert_eq!(
        json!({"name": dependency.name, "vendor": dependency.vendor, "version": dependency.version, "url": dependency.url}),
        expected["reports"]
    );
    // The device as the table keeps it.
    let kept: Device = serde_json::from_value(expected["keeps"]["device"].clone()).unwrap();
    assert_eq!(track.devices[0], kept);
    // The edit, as a reader without the plugin makes it: into overrides only; then written by
    // another program.
    let state_file = expected["keeps"]["state_file"].as_str().unwrap();
    let mut copy = opened.project.copy_into(MemoryStore::new()).unwrap();
    let state = copy.read(state_file).unwrap();
    copy.document_mut().track_mut(&track.id).unwrap().devices[0]
        .overrides
        .push(Override {
            param: expected["edit"]["param"].as_str().unwrap().to_owned(),
            value: expected["edit"]["value"].as_f64().unwrap(),
            normalized: None,
            unknown: tutti::Unknown::new(),
        });
    copy.save(&another_writer()).unwrap();
    let again = tutti::read(&copy.store().read(copy.document_name()).unwrap()).unwrap();
    let edited = again.track(&track.id).unwrap();
    let mut want = kept;
    want.overrides = serde_json::from_value(expected["after_edit"]["overrides"].clone()).unwrap();
    assert_eq!(edited.devices[0], want);
    assert!(again.files.iter().any(|f| f.path == state_file));
    assert_eq!(copy.read(state_file).unwrap(), state);
    // The fingerprint covers the device, so the render is stale now.
    assert_eq!(expected["after_edit"]["render_stale"], true);
    assert_ne!(
        render_fingerprint(&again, edited, render.from, render.to),
        edited.renders[0].fingerprint
    );
}

#[test]
fn level_2_everything_round_trips_through_another_writer() {
    let folder = level_case("l2/everything");
    let expected = expected_of(&folder);
    let opened = tutti_fs::open(&folder).unwrap();
    assert!(opened.report.is_valid());
    let original = opened.project.document().clone();
    let mut copy = opened.project.copy_into(MemoryStore::new()).unwrap();
    copy.save(&another_writer()).unwrap();
    let again = tutti::read(&copy.store().read(copy.document_name()).unwrap()).unwrap();
    assert_eq!(again.generator.name, "another-writer");
    assert_eq!(expected["equal"], true);
    assert!(
        tutti::equal(&original, &again),
        "not equal once written back"
    );
    assert_eq!(json!(ids_of(&again)), expected["ids"]);
    let shared: Vec<Value> = shared_patterns(&again)
        .into_iter()
        .map(|(pattern, parts)| json!({"pattern": pattern, "parts": parts}))
        .collect();
    assert_eq!(Value::from(shared), expected["shared_patterns"]);
    let files: Vec<&str> = again.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(json!(files), expected["files"]);
    let file_errors: Vec<String> = check_files(&again, copy.store())
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .map(|i| i.to_string())
        .collect();
    assert!(file_errors.is_empty(), "{file_errors:#?}");
    assert_eq!(
        json!(again.extensions_required),
        expected["extensions_required"]
    );
    let readme = copy
        .store()
        .read(expected["readme"].as_str().unwrap())
        .unwrap();
    assert!(
        String::from_utf8(readme)
            .unwrap()
            .contains("another-writer")
    );
}

/// The object with this id, anywhere in a document as JSON.
fn find_by_id<'a>(value: &'a mut Value, id: &str) -> Option<&'a mut Value> {
    if value.get("id").and_then(Value::as_str) == Some(id) {
        return Some(value);
    }
    match value {
        Value::Object(map) => map.values_mut().find_map(|v| find_by_id(v, id)),
        Value::Array(items) => items.iter_mut().find_map(|v| find_by_id(v, id)),
        _ => None,
    }
}

/// One edit of a merge case's table, made to a document as JSON: `set` a value at a path of
/// keys in the object with the id (checking what was there), or `add` one to an array there.
fn apply(doc: &mut Value, edit: &Value) {
    let id = edit["id"].as_str().unwrap();
    let object = find_by_id(doc, id).unwrap_or_else(|| panic!("no object {id}"));
    let keys: Vec<&str> = edit["key"].as_str().unwrap().split('/').collect();
    let (last, path) = keys.split_last().unwrap();
    let mut at = object;
    for key in path {
        at = at
            .as_object_mut()
            .unwrap()
            .entry(*key)
            .or_insert_with(|| json!({}));
    }
    match edit["op"].as_str().unwrap() {
        "set" => {
            if let Some(was) = at.get(*last) {
                assert_eq!(was.as_f64(), edit["from"].as_f64(), "{id}: {last}");
            }
            at[*last] = edit["value"].clone();
        }
        "add" => at
            .as_object_mut()
            .unwrap()
            .entry(*last)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .unwrap()
            .push(edit["value"].clone()),
        other => panic!("{other:?} is not an edit"),
    }
}

/// A document as JSON, read and written in the pretty form, as a writer would.
fn rewritten(value: &Value) -> String {
    tutti::write(&tutti::read(&serde_json::to_vec(value).unwrap()).unwrap())
}

#[test]
fn level_2_two_branches_on_different_tracks_merge_in_git() {
    let folder = level_case("l2/merge-different-tracks");
    let expected = expected_of(&folder);
    let base_name = expected["base"].as_str().unwrap();
    let base: Value = serde_json::from_slice(&fs::read(folder.join(base_name)).unwrap()).unwrap();
    let mut both = base.clone();
    let mut documents = Vec::new();
    for branch in expected["branches"].as_array().unwrap() {
        // Each branch's edits, made to the base, give its document in the pretty form.
        let document = branch["document"].as_str().unwrap();
        let mut value = base.clone();
        for edit in branch["edits"].as_array().unwrap() {
            apply(&mut value, edit);
            apply(&mut both, edit);
        }
        assert_eq!(
            rewritten(&value),
            fs::read_to_string(folder.join(document)).unwrap(),
            "{document}"
        );
        documents.push(document);
    }
    // Both branches' edits on the base: the merged document, valid.
    let merged = fs::read_to_string(folder.join(expected["merged"].as_str().unwrap())).unwrap();
    assert_eq!(rewritten(&both), merged);
    let validator = jsonschema::validator_for(&schema()).expect("the schema compiles");
    let errors = schema_errors(&validator, &serde_json::from_str(&merged).unwrap());
    assert!(errors.is_empty(), "{errors:#?}");
    let doc = tutti::read(merged.as_bytes()).unwrap();
    assert!(validate::validate(&doc).is_valid());
    assert!(
        check_files(&doc, &DirStore::new(&folder))
            .iter()
            .all(|i| i.severity != Severity::Error)
    );
    // And git merges the two branches into it without a conflict.
    let output = Command::new("git")
        .current_dir(&folder)
        .args(["merge-file", "-p", documents[0], base_name, documents[1]])
        .output()
        .expect("git, which this test runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "conflicts:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), merged);
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

/// What a reader says about a hostile folder: the first refusal, in order of the checks a reader
/// makes (one document, I-JSON, the version, validation, the files). A project as one file is
/// unpacked first, into a folder of the test's, and refused there or as a folder.
fn refusal(folder: &Path) -> String {
    if let Some(zip) = zip_in(folder) {
        let into = std::env::temp_dir().join(format!(
            "tutti-hostile-{}-{}",
            std::process::id(),
            folder.file_name().unwrap().to_string_lossy()
        ));
        let _ = fs::remove_dir_all(&into);
        return match tutti_fs::zip::unpack(&zip, &into) {
            Err(e) => e.to_string(),
            Ok(_) => refusal(&into),
        };
    }
    let path = match document_in(folder) {
        Ok(p) => p,
        Err(e) => return e.to_string(),
    };
    let bytes = fs::read(&path).unwrap();
    let doc = match tutti::read(&bytes) {
        Ok(d) => d,
        Err(e) => return e.to_string(),
    };
    let report = validate::validate(&doc);
    if let Some(first) = report.errors().next() {
        return first.to_string();
    }
    let issues = check_files(&doc, &DirStore::new(folder));
    if let Some(first) = issues.iter().find(|i| i.severity == Severity::Error) {
        return first.to_string();
    }
    "accepted".to_owned()
}

fn keys_of(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                out.insert(k.clone());
                keys_of(v, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|v| keys_of(v, out)),
        _ => {}
    }
}

/// Every key named under a `properties` anywhere in the schema (the alternatives of a `oneOf`
/// included).
fn schema_property_names(schema: &Value) -> BTreeSet<String> {
    fn walk(value: &Value, out: &mut BTreeSet<String>) {
        match value {
            Value::Object(map) => {
                if let Some(Value::Object(props)) = map.get("properties") {
                    out.extend(props.keys().cloned());
                }
                map.values().for_each(|v| walk(v, out));
            }
            Value::Array(items) => items.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }
    let mut out = BTreeSet::new();
    walk(schema, &mut out);
    out
}

#[test]
fn the_every_feature_example_and_the_schema_cover_each_other() {
    let schema = schema();
    let names = schema_property_names(&schema);
    let folder = repository_root().join("examples/every-feature");
    let doc: Value =
        serde_json::from_slice(&fs::read(document_in(&folder).unwrap()).unwrap()).unwrap();
    let mut used = BTreeSet::new();
    keys_of(&doc, &mut used);
    // Keys under `extensions` are vendor data, not the schema's.
    let vendor: BTreeSet<String> = [
        "dk.mxm.newdawn",
        "org.example.lyrics",
        "armed",
        "language",
        "lines",
        "text",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let unknown: Vec<&String> = used
        .iter()
        .filter(|k| !names.contains(*k) && !vendor.contains(*k))
        .collect();
    assert!(
        unknown.is_empty(),
        "keys the schema does not name: {unknown:?}"
    );
    // `min_version` is left out at its default, and a higher value would make the example open
    // read-only, so it is the one key the example does not carry.
    let unused: Vec<&String> = names
        .iter()
        .filter(|n| !used.contains(*n) && *n != "min_version")
        .collect();
    assert!(
        unused.is_empty(),
        "schema properties every-feature never uses: {unused:?}"
    );
}

#[test]
fn the_spec_the_schema_and_the_crate_agree_on_the_version() {
    let spec = fs::read_to_string(
        repository_root()
            .join("spec")
            .join(tutti::VERSION)
            .join("tutti.md"),
    )
    .unwrap();
    assert!(
        spec.starts_with(&format!("# tutti {}", tutti::VERSION)),
        "the spec's heading"
    );
    let schema = schema();
    assert_eq!(schema["title"], format!("tutti {}", tutti::VERSION));
    assert!(
        schema["$id"]
            .as_str()
            .unwrap()
            .contains(&format!("/schema/{}/", tutti::VERSION))
    );
    assert!(tutti::SCHEMA_URL.contains(&format!("/schema/{}/", tutti::VERSION)));
    assert!(tutti::SPEC_URL.contains(&format!("/spec/{}/", tutti::VERSION)));
    assert!(tutti::ABOUT.contains(&format!("tutti {}", tutti::VERSION)));
}

#[test]
fn the_schemas_bounds_are_the_crates_constants() {
    let schema = schema();
    let defs = &schema["$defs"];
    assert_eq!(schema["properties"]["ppq"]["maximum"], validate::PPQ_MAX);
    assert_eq!(defs["ticks"]["maximum"], tutti::time::TICK_MAX);
    assert_eq!(defs["signed_ticks"]["minimum"], -tutti::time::TICK_MAX);
    assert_eq!(defs["bpm"]["maximum"], validate::BPM_MAX);
    assert_eq!(defs["gain"]["maximum"], validate::GAIN_MAX);
    assert_eq!(
        defs["meter_point"]["properties"]["numerator"]["maximum"],
        validate::NUMERATOR_MAX
    );
    assert_eq!(
        defs["meter_point"]["properties"]["denominator"]["enum"]
            .as_array()
            .unwrap()
            .last()
            .unwrap(),
        &Value::from(validate::DENOMINATOR_MAX)
    );
    assert_eq!(
        defs["audio_piece"]["properties"]["sample_rate"]["maximum"],
        validate::SAMPLE_RATE_MAX
    );
    assert_eq!(
        defs["audio_piece"]["properties"]["channels"]["maximum"],
        validate::CHANNELS_MAX
    );
    assert_eq!(
        defs["id"]["pattern"],
        format!("^[A-Za-z0-9_-]{{1,{}}}$", tutti::id::MAX_LEN)
    );
    assert_eq!(schema["properties"]["format"]["const"], tutti::FORMAT);
}

#[test]
fn the_committed_field_reference_is_the_generators_output() {
    let committed =
        fs::read_to_string(repository_root().join("docs/field-reference.md")).unwrap_or_default();
    let generated = field_reference(&schema());
    assert_eq!(
        committed, generated,
        "run: cargo run -p tutti-schema --bin fields > docs/field-reference.md"
    );
}

#[test]
fn the_readme_template_has_only_the_placeholders_the_crate_fills() {
    let template = tutti::README_TEMPLATE;
    let filled = tutti::readme(
        "Test",
        &tutti::Generator::new("t", "1", "https://x"),
        "Test.tutti",
    );
    for placeholder in [
        "{title}",
        "{version}",
        "{generator}",
        "{document}",
        "{spec}",
        "{schema}",
        "{agents}",
        "{repository}",
    ] {
        assert!(
            template.contains(placeholder),
            "the template lost {placeholder}"
        );
        assert!(!filled.contains(placeholder), "{placeholder} not filled");
    }
    assert!(template.len() < 3000, "about 2 kB (spec 2.4)");
}
