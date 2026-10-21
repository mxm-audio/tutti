//! The `tutti` command, over `tutti-fs` for the project folder. Arguments are read by hand, as
//! newDAWn's command line does: no argument-parsing dependency.
//!
//! Exit codes: 0 valid, 1 invalid (the reasons on stderr, one per line, with a JSON pointer
//! where one applies), 2 usage.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use tutti::features::{features, level_needed};
use tutti::validate::Severity;
use tutti::{Document, Event, Opened};
use tutti_cli::{Installed, archive, find_installed};
use tutti_fs::DirStore;

const USAGE: &str = "\
tutti: an open audio project format (version 1.0)

    tutti check <folder>      validate a project folder: the document, its files, the
                              features it uses and the level a reader needs
    tutti inspect <folder>    list its tracks, devices, patterns, markers and extensions
    tutti deps <folder>       list what it depends on, and whether each plugin is installed
    tutti archive <folder>    copy the spec, schema and agent guide into the folder's spec/
    tutti fmt <folder>        rewrite the document in the pretty form (keys in order,
                              defaults left out, ids and unknown data kept)
    tutti zip <folder>        pack the project into <Name>.tutti.zip beside the folder
    tutti unzip <file>        unpack <Name>.tutti.zip into the folder <Name> beside it
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, folder) = match (args.first().map(String::as_str), args.get(1)) {
        (Some(c), Some(f)) if args.len() == 2 => (c, PathBuf::from(f)),
        _ => {
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let result = match command {
        "check" => check(&folder),
        "inspect" => inspect(&folder),
        "deps" => deps(&folder),
        "archive" => archive_command(&folder),
        "fmt" => fmt(&folder),
        "zip" => zip_command(&folder),
        "unzip" => unzip_command(&folder),
        _ => {
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

/// The project in a folder, opened: refusals are errors with their reason.
fn open_project(folder: &Path) -> Result<Opened<DirStore>, String> {
    tutti_fs::open(folder).map_err(|e| format!("{}: {e}", folder.display()))
}

/// The document of a folder, read, and its path.
fn open(folder: &Path) -> Result<(PathBuf, Document), String> {
    let opened = open_project(folder)?;
    Ok((
        folder.join(opened.project.document_name()),
        opened.project.document().clone(),
    ))
}

fn check(folder: &Path) -> Result<bool, String> {
    let opened = open_project(folder)?;
    let path = folder.join(opened.project.document_name());
    let doc = opened.project.document();
    let issues = opened.project.check().issues;
    let errors = issues
        .iter()
        .filter(|i| i.severity == Severity::Error)
        .count();
    let warnings = issues
        .iter()
        .filter(|i| i.severity == Severity::Warning)
        .count();
    for issue in &issues {
        eprintln!("{issue}");
    }
    if opened.read_only {
        println!(
            "read-only on this version: min_version {}",
            doc.min_version.as_deref().unwrap_or("?")
        );
    }
    let used = features(doc);
    let level = level_needed(doc);
    println!(
        "{}: {} ({} error{}, {} warning{})",
        path.display(),
        if errors == 0 { "valid" } else { "invalid" },
        errors,
        if errors == 1 { "" } else { "s" },
        warnings,
        if warnings == 1 { "" } else { "s" }
    );
    if used.is_empty() {
        println!("features: none; any reader plays it");
    } else {
        let names: Vec<String> = used
            .iter()
            .map(|f| format!("{} (L{})", f.name, f.level))
            .collect();
        println!("features: {}", names.join(", "));
        println!("a reader needs Level {level} to play it in full");
    }
    Ok(errors == 0)
}

fn inspect(folder: &Path) -> Result<bool, String> {
    let (path, doc) = open(folder)?;
    println!("{}", path.display());
    println!(
        "tutti {} written by {}{}",
        doc.version,
        doc.generator.name,
        doc.generator
            .version
            .as_ref()
            .map(|v| format!(" {v}"))
            .unwrap_or_default()
    );
    if let Some(title) = &doc.metadata.title {
        println!("title: {title}");
    }
    println!(
        "ppq {}, tempo {}{}, meter {}/{}{}",
        doc.ppq,
        doc.tempo.first().map(|t| t.bpm).unwrap_or(0.0),
        if doc.tempo.len() > 1 {
            format!(" and {} more point(s)", doc.tempo.len() - 1)
        } else {
            String::new()
        },
        doc.meter.first().map(|m| m.numerator).unwrap_or(0),
        doc.meter.first().map(|m| m.denominator).unwrap_or(0),
        if doc.meter.len() > 1 {
            format!(" and {} more point(s)", doc.meter.len() - 1)
        } else {
            String::new()
        },
    );
    if !doc.markers.is_empty() {
        println!("markers:");
        for m in &doc.markers {
            match m.length {
                Some(len) => println!("  {} {} at {} for {}", m.kind, m.name, m.at, len),
                None => println!("  {} {} at {}", m.kind, m.name, m.at),
            }
        }
    }
    if !doc.master.devices.is_empty() {
        println!(
            "master: {}",
            doc.master
                .devices
                .iter()
                .map(|d| d.name.clone())
                .collect::<Vec<_>>()
                .join(" > ")
        );
    }
    println!("tracks:");
    for t in &doc.tracks {
        print_track(t, 1);
    }
    if !doc.scenes.is_empty() {
        println!(
            "scenes: {}",
            doc.scenes
                .iter()
                .map(|s| s.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !doc.extensions_used.is_empty() {
        println!("extensions used: {}", doc.extensions_used.join(", "));
    }
    if !doc.extensions_required.is_empty() {
        println!(
            "extensions required: {}",
            doc.extensions_required.join(", ")
        );
    }
    Ok(true)
}

fn print_track(t: &tutti::Track, depth: usize) {
    let indent = "  ".repeat(depth);
    let notes: usize = t
        .patterns
        .iter()
        .map(|p| {
            p.events
                .iter()
                .filter(|e| matches!(e, Event::Note(_)))
                .count()
        })
        .sum();
    let audio: usize = t
        .patterns
        .iter()
        .map(|p| {
            p.events
                .iter()
                .filter(|e| matches!(e, Event::Note(n) if n.audio.is_some()))
                .count()
        })
        .sum();
    let mut what = Vec::new();
    if !t.devices.is_empty() {
        what.push(
            t.devices
                .iter()
                .map(|d| format!("{} [{}]", d.name, d.plugin))
                .collect::<Vec<_>>()
                .join(" > "),
        );
    }
    what.push(format!(
        "{} pattern(s), {} part(s), {} note(s), {} of them audio",
        t.patterns.len(),
        t.parts.len(),
        notes,
        audio
    ));
    if !t.channel.sends.is_empty() {
        what.push(format!("{} send(s)", t.channel.sends.len()));
    }
    if !t.automation.is_empty() {
        what.push(format!("{} automation lane(s)", t.automation.len()));
    }
    if !t.renders.is_empty() {
        what.push(format!("{} render(s)", t.renders.len()));
    }
    println!("{indent}{} ({}): {}", t.name, t.id, what.join("; "));
    for child in &t.children {
        print_track(child, depth + 1);
    }
}

fn deps(folder: &Path) -> Result<bool, String> {
    let (_, doc) = open(folder)?;
    if doc.dependencies.is_empty() {
        println!("no dependencies: the project needs nothing beyond its folder");
        return Ok(true);
    }
    for d in &doc.dependencies {
        let found = find_installed(&d.id);
        let status = match &found {
            Installed::Exact(p) => format!("installed: {}", p.display()),
            Installed::Likely(p) => format!("likely installed: {}", p.display()),
            Installed::NotFound => "not found in the standard plugin folders".to_owned(),
        };
        println!(
            "{} {}{} ({}): {}",
            d.name,
            d.vendor.as_deref().unwrap_or(""),
            d.version
                .as_ref()
                .map(|v| format!(" {v}"))
                .unwrap_or_default(),
            d.id,
            status
        );
        if let Some(url) = &d.url {
            println!("  from: {url}");
        }
        if let Some(repo) = &d.repository {
            println!(
                "  source: {repo}{}",
                d.commit
                    .as_ref()
                    .map(|c| format!(" at {c}"))
                    .unwrap_or_default()
            );
        }
        if let Some(license) = &d.license {
            println!("  licence: {license}");
        }
    }
    Ok(true)
}

fn archive_command(folder: &Path) -> Result<bool, String> {
    tutti_fs::document_in(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let written = archive(folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    for path in written {
        println!("{}", path.display());
    }
    Ok(true)
}

fn zip_command(folder: &Path) -> Result<bool, String> {
    let name = folder
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{}: no folder name", folder.display()))?;
    let file = folder
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
        .join(format!("{name}.{}", tutti::zip::ZIP_EXTENSION));
    let entries =
        tutti_fs::zip::pack(folder, &file).map_err(|e| format!("{}: {e}", folder.display()))?;
    println!("{}: {entries} entries", file.display());
    Ok(true)
}

fn unzip_command(file: &Path) -> Result<bool, String> {
    let name = file
        .file_name()
        .and_then(|n| tutti::zip::project_name(&n.to_string_lossy()).map(str::to_owned))
        .ok_or_else(|| {
            format!(
                "{}: not a <Name>.{} file",
                file.display(),
                tutti::zip::ZIP_EXTENSION
            )
        })?;
    let into = file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
        .join(name);
    let files =
        tutti_fs::zip::unpack(file, &into).map_err(|e| format!("{}: {e}", file.display()))?;
    println!("{}: {files} files", into.display());
    Ok(true)
}

fn fmt(folder: &Path) -> Result<bool, String> {
    let mut project = open_project(folder)?.project;
    let path = folder.join(project.document_name());
    project
        .rewrite()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    println!("{}: the pretty form", path.display());
    Ok(true)
}
