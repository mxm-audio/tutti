//! A project as one file, `<Name>.tutti.zip` (spec 2.5): what the core can say without IO, the
//! names a reader accepts and the limit it holds an entry to. Packing and unpacking the file is
//! `tutti-fs`'s (`tutti_fs::zip`).

/// The extension of a project as one file, without the dot.
pub const ZIP_EXTENSION: &str = "tutti.zip";

/// The most bytes an entry the files index does not cover may declare (the document, the README,
/// an analysis file, a plugin's file): a bomb declares more.
pub const UNINDEXED_LIMIT: u64 = 1 << 30;

/// Whether a file name is a project's as one file: `<Name>.tutti.zip`, in any case.
pub fn is_zip_name(name: &str) -> bool {
    project_name(name).is_some()
}

/// The project's name from its file's: `My-Project` from `My-Project.tutti.zip`.
pub fn project_name(file_name: &str) -> Option<&str> {
    let suffix = format!(".{ZIP_EXTENSION}");
    if file_name.len() <= suffix.len() {
        return None;
    }
    let (stem, end) = file_name.split_at(file_name.len() - suffix.len());
    end.eq_ignore_ascii_case(&suffix).then_some(stem)
}

/// Checks an entry's name (spec 2.5): a relative path of plain parts, `/` between them. The
/// reason when it is not, in the words a refusal uses.
pub fn check_entry_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("an empty name".to_owned());
    }
    if name.starts_with('/') {
        return Err("an absolute path".to_owned());
    }
    if name.contains('\\') {
        return Err("a backslash in the name".to_owned());
    }
    if name.chars().any(char::is_control) {
        return Err("a control character in the name".to_owned());
    }
    if name.as_bytes().get(1) == Some(&b':') {
        return Err("a drive letter".to_owned());
    }
    let folder_entry = name.ends_with('/');
    let parts: Vec<&str> = name.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            if folder_entry && i == parts.len() - 1 {
                continue;
            }
            return Err("an empty part in the path".to_owned());
        }
        if *part == "." || *part == ".." {
            return Err("a part that leaves the folder".to_owned());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_of_projects_as_one_file() {
        assert_eq!(project_name("My-Project.tutti.zip"), Some("My-Project"));
        assert_eq!(project_name("x.TUTTI.ZIP"), Some("x"));
        assert_eq!(project_name(".tutti.zip"), None);
        assert_eq!(project_name("My-Project.zip"), None);
        assert!(is_zip_name("A.tutti.zip") && !is_zip_name("A.tutti"));
    }

    #[test]
    fn entry_names_a_reader_refuses() {
        for ok in ["A.tutti", "audio/x.wav", "plugins/states/ab.clap", "audio/"] {
            assert_eq!(check_entry_name(ok), Ok(()), "{ok}");
        }
        for (bad, why) in [
            ("", "empty"),
            ("/etc/passwd", "absolute"),
            ("audio\\x.wav", "backslash"),
            ("audio/../x.wav", "leaves"),
            ("./x.wav", "leaves"),
            ("audio//x.wav", "empty part"),
            ("C:/x.wav", "drive"),
            ("audio/x\u{0}.wav", "control"),
        ] {
            let reason = check_entry_name(bad).unwrap_err();
            assert!(reason.contains(why), "{bad:?}: {reason}");
        }
    }
}
