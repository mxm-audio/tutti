//! The tests that hold the schema, the examples, the docs and the types together, and the
//! generator of `docs/field-reference.md`. Nothing here ships in a program: the validator it
//! uses is a dev-dependency, so its toolchain floor never reaches a DAW. A project folder on disk
//! is the command's `DirFolder` (`tutti-cli`), which the tests take as a dev-dependency.

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

/// The repository's root, from this crate's manifest.
pub fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// This version's schema, as a value.
pub fn schema() -> Value {
    let path = repository_root()
        .join("schema")
        .join(tutti::VERSION)
        .join("tutti.schema.json");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).expect("the schema is JSON")
}

/// `docs/field-reference.md` from the schema: the document's keys, then every object under
/// `$defs`, each a table of key, type, required, default and meaning.
pub fn field_reference(schema: &Value) -> String {
    let mut out = String::new();
    out.push_str("# Field reference (tutti ");
    out.push_str(tutti::VERSION);
    out.push_str(")\n\n**Generated** from `schema/");
    out.push_str(tutti::VERSION);
    out.push_str("/tutti.schema.json` by `cargo run -p tutti-schema --bin fields`; a test fails when this file\ndiffers from the generator's output. Edit the schema, not this file. The normative text is\n`spec/");
    out.push_str(tutti::VERSION);
    out.push_str("/tutti.md`; the bounds here are its.\n\n");
    out.push_str("## The document\n\n");
    table(&mut out, schema);
    if let Some(Value::Object(defs)) = schema.get("$defs") {
        for (name, def) in defs {
            if def.get("properties").is_none() {
                continue;
            }
            out.push_str(&format!("## `{name}`\n\n"));
            if let Some(d) = def.get("description").and_then(Value::as_str) {
                out.push_str(d);
                out.push_str("\n\n");
            }
            table(&mut out, def);
        }
    }
    out
}

fn table(out: &mut String, object: &Value) {
    let required: Vec<&str> = object
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    out.push_str("| Key | Type | Required | Default | Meaning |\n|---|---|---|---|---|\n");
    if let Some(Value::Object(props)) = object.get("properties") {
        for (key, prop) in props {
            let ty = type_text(prop);
            let req = if required.contains(&key.as_str()) {
                "yes"
            } else {
                ""
            };
            let default = prop
                .get("default")
                .map(|d| format!("`{}`", tutti::canon::compact(d)))
                .unwrap_or_default();
            let meaning = prop
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .replace('|', "\\|");
            out.push_str(&format!(
                "| `{key}` | {ty} | {req} | {default} | {meaning} |\n"
            ));
        }
    }
    out.push('\n');
}

fn type_text(prop: &Value) -> String {
    if let Some(r) = prop.get("$ref").and_then(Value::as_str) {
        let name = r.rsplit('/').next().unwrap_or(r);
        return format!("`{name}`");
    }
    if let Some(c) = prop.get("const") {
        return format!("`{}`", tutti::canon::compact(c));
    }
    if let Some(one) = prop.get("oneOf").and_then(Value::as_array) {
        return one.iter().map(type_text).collect::<Vec<_>>().join(" or ");
    }
    if let Some(e) = prop.get("enum").and_then(Value::as_array) {
        return e
            .iter()
            .map(|v| format!("`{}`", tutti::canon::compact(v)))
            .collect::<Vec<_>>()
            .join(", ");
    }
    let ty = prop
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("any")
        .to_owned();
    let mut text = ty.clone();
    if ty == "array" {
        if let Some(items) = prop.get("items") {
            text = format!("array of {}", type_text(items));
        }
    }
    let mut bounds = Vec::new();
    if let Some(m) = prop.get("minimum") {
        bounds.push(format!("≥ {}", tutti::canon::compact(m)));
    }
    if let Some(m) = prop.get("exclusiveMinimum") {
        bounds.push(format!("> {}", tutti::canon::compact(m)));
    }
    if let Some(m) = prop.get("maximum") {
        bounds.push(format!("≤ {}", tutti::canon::compact(m)));
    }
    if let Some(p) = prop.get("pattern").and_then(Value::as_str) {
        bounds.push(format!("`{}`", p.replace('|', "\\|")));
    }
    if !bounds.is_empty() {
        text = format!("{text}, {}", bounds.join(", "));
    }
    text
}
