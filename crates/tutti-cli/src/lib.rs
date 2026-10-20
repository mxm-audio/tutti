//! What the `tutti` command does with a folder, as a library so the tests and the generators can
//! use the same folder on disk: `DirFolder` (a project folder, references resolved inside it,
//! links refused), the one document in a folder, the standard plugin folders, and `archive`.

use std::fs;
use std::path::{Path, PathBuf};

use tutti::folder::{Folder, FolderError};

/// A project folder on disk: references resolved inside it, links refused (spec 9).
pub struct DirFolder {
    pub root: PathBuf,
}

impl DirFolder {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The path of a reference, each part checked not to be a link, `.`, `..` or empty.
    pub fn path_of(&self, reference: &str) -> Result<PathBuf, FolderError> {
        let mut path = self.root.clone();
        for part in reference.split('/') {
            if part.is_empty() || part == "." || part == ".." || part.contains('\\') {
                return Err(FolderError::Other(format!("{reference}: not a reference")));
            }
            path.push(part);
            let meta = fs::symlink_metadata(&path).map_err(|_| FolderError::Missing)?;
            if meta.file_type().is_symlink() {
                return Err(FolderError::Link);
            }
        }
        Ok(path)
    }
}

impl Folder for DirFolder {
    fn read(&self, reference: &str) -> Result<Vec<u8>, FolderError> {
        fs::read(self.path_of(reference)?).map_err(|e| FolderError::Other(e.to_string()))
    }

    fn size(&self, reference: &str) -> Result<u64, FolderError> {
        fs::metadata(self.path_of(reference)?)
            .map(|m| m.len())
            .map_err(|e| FolderError::Other(e.to_string()))
    }
}

/// The one `.tutti` document at the top of a folder: its path, or why not (spec 3.1).
pub fn document_in(folder: &Path) -> Result<PathBuf, String> {
    let mut found: Vec<PathBuf> = fs::read_dir(folder)
        .map_err(|e| format!("{}: {e}", folder.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "tutti") && p.is_file())
        .collect();
    found.sort();
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(format!("{}: no .tutti document", folder.display())),
        n => Err(format!(
            "{}: {n} documents ({})",
            folder.display(),
            found
                .iter()
                .filter_map(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

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
    ("spec/agents.md", include_str!("../../../docs/agents.md")),
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
