//! The tests that hold the spec, the schema, the examples, the docs and the code together.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use tutti::folder::check_files;
use tutti::validate::{self, Severity};
use tutti_fs::{DirStore, document_in};
use tutti_schema::{field_reference, repository_root, schema};

/// Every example project folder: the three examples and the conformance fixtures.
fn example_folders() -> Vec<PathBuf> {
    let examples = repository_root().join("examples");
    let mut out = vec![
        examples.join("minimal"),
        examples.join("typical"),
        examples.join("every-feature"),
    ];
    for level in ["l0", "l1", "l2"] {
        let dir = examples.join("conformance").join(level);
        if let Ok(entries) = fs::read_dir(&dir) {
            let mut cases: Vec<PathBuf> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_dir())
                .collect();
            cases.sort();
            out.extend(cases);
        }
    }
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
    let hostile = repository_root().join("examples/conformance/hostile");
    let mut cases: Vec<PathBuf> = fs::read_dir(&hostile)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    cases.sort();
    assert!(!cases.is_empty());
    for folder in cases {
        let expected: Value =
            serde_json::from_slice(&fs::read(folder.join("expected.json")).unwrap()).unwrap();
        let want = expected["error"].as_str().unwrap();
        let actual = refusal(&folder);
        assert!(
            actual.contains(want),
            "{}: expected {want:?}, got {actual:?}",
            folder.display()
        );
    }
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
