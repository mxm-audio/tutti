//! What the `tutti` command does besides a project: the standard plugin folders and whether a
//! plugin is installed (`deps`), and `archive`. The project itself is `tutti-fs`'s.

use std::fs;
use std::path::{Path, PathBuf};

/// The folders a host scans for plugins of a format (`clap` or `vst3`): the environment's
/// `CLAP_PATH` or `VST3_PATH` first, then the platform's standard folders.
pub fn plugin_folders(format: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let var = match format {
        "clap" => "CLAP_PATH",
        "vst3" => "VST3_PATH",
        _ => return out,
    };
    if let Ok(paths) = std::env::var(var) {
        let separator = if cfg!(windows) { ';' } else { ':' };
        out.extend(
            paths
                .split(separator)
                .filter(|p| !p.is_empty())
                .map(PathBuf::from),
        );
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    if cfg!(windows) {
        let common = std::env::var_os("COMMONPROGRAMFILES")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\Common Files"));
        out.push(common.join(if format == "clap" { "CLAP" } else { "VST3" }));
        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            out.push(
                local
                    .join("Programs")
                    .join("Common")
                    .join(if format == "clap" { "CLAP" } else { "VST3" }),
            );
        }
    } else if cfg!(target_os = "macos") {
        out.push(
            PathBuf::from("/Library/Audio/Plug-Ins").join(if format == "clap" {
                "CLAP"
            } else {
                "VST3"
            }),
        );
        if let Some(h) = &home {
            out.push(h.join("Library/Audio/Plug-Ins").join(if format == "clap" {
                "CLAP"
            } else {
                "VST3"
            }));
        }
    } else {
        if let Some(h) = &home {
            out.push(h.join(if format == "clap" { ".clap" } else { ".vst3" }));
        }
        out.push(PathBuf::from(if format == "clap" {
            "/usr/lib/clap"
        } else {
            "/usr/lib/vst3"
        }));
        out.push(PathBuf::from(if format == "clap" {
            "/usr/local/lib/clap"
        } else {
            "/usr/local/lib/vst3"
        }));
    }
    out
}

/// Where a plugin id seems to be installed, and how sure that is. A CLAP id is inside the
/// binary, so a `.clap` whose name is the id's last segment is *likely*; a VST3 class id is
/// *exact* when a bundle's `moduleinfo.json` lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Installed {
    Exact(PathBuf),
    Likely(PathBuf),
    NotFound,
}

/// Looks for a plugin id (`clap:<id>`, `vst3:<class id>`) in the standard folders.
pub fn find_installed(plugin: &str) -> Installed {
    if let Some(id) = plugin.strip_prefix("clap:") {
        let last = id.rsplit('.').next().unwrap_or(id).to_ascii_lowercase();
        for folder in plugin_folders("clap") {
            if let Some(found) = find_bundle(&folder, "clap", &last) {
                return Installed::Likely(found);
            }
        }
    } else if let Some(class) = plugin.strip_prefix("vst3:") {
        let class = class.to_ascii_uppercase();
        for folder in plugin_folders("vst3") {
            let Ok(entries) = fs::read_dir(&folder) else {
                continue;
            };
            for entry in entries.filter_map(Result::ok) {
                let bundle = entry.path();
                let info = bundle.join("Contents").join("moduleinfo.json");
                if let Ok(text) = fs::read_to_string(&info) {
                    if text.to_ascii_uppercase().contains(&class) {
                        return Installed::Exact(bundle);
                    }
                }
            }
            let last = class.to_ascii_lowercase();
            if let Some(found) = find_bundle(&folder, "vst3", &last) {
                return Installed::Likely(found);
            }
        }
    }
    Installed::NotFound
}

fn find_bundle(folder: &Path, extension: &str, stem: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(folder).ok()?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let is_bundle = path
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case(extension));
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if is_bundle && name == stem {
            return Some(path);
        }
    }
    None
}

/// This version's spec, schema and agent guide, as the `archive` command copies them into a
/// project folder (spec 2.4).
pub const ARCHIVE_FILES: &[(&str, &str)] = &[
    ("spec/tutti.md", include_str!("../../../spec/1.0/tutti.md")),
    (
        "spec/conformance.md",
        include_str!("../../../spec/1.0/conformance.md"),
    ),
    (
        "spec/extensions.md",
        include_str!("../../../spec/1.0/extensions.md"),
    ),
    (
        "spec/README.template.md",
        include_str!("../../../spec/1.0/README.template.md"),
    ),
    (
        "spec/tutti.schema.json",
        include_str!("../../../schema/1.0/tutti.schema.json"),
    ),
    (
        "spec/agent-guide.md",
        include_str!("../../../docs/agent-guide.md"),
    ),
];

/// Copies the spec, the schema and the agent guide into `<folder>/spec/`.
pub fn archive(folder: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for (name, text) in ARCHIVE_FILES {
        let path = folder.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, text.as_bytes())?;
        written.push(path);
    }
    Ok(written)
}
