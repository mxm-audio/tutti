//! The `tutti` command: the one place in the repository with file IO. Arguments are read by
//! hand, as newDAWn's command line does: no argument-parsing dependency.

use std::process::ExitCode;

const USAGE: &str = "\
tutti: an open audio project format

    tutti check <folder>      validate a project folder and say which features it uses
    tutti inspect <folder>    list its tracks, patterns, devices and extensions
    tutti deps <folder>       list what it depends on, and whether each plugin is installed
    tutti archive <folder>    copy the spec, schema and agent guide into the folder
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check" | "inspect" | "deps" | "archive") => {
            eprintln!(
                "tutti {}: not built yet (the format's version is {})",
                args[0],
                tutti::VERSION
            );
            ExitCode::from(2)
        }
        _ => {
            eprint!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
