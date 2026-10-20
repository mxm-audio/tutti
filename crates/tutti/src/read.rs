//! Reading a document (spec 3.1, 8.3, 10): I-JSON checks a parser does not make by itself
//! (no byte-order mark, no duplicate keys), the format and version refusals, then the model, in
//! canonical order.

use std::collections::HashSet;
use std::fmt;

use serde::Deserialize;

use crate::model::{Document, version_parts};

/// Why bytes are not a document this crate opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// Not UTF-8.
    Encoding,
    /// Starts with a byte-order mark, which I-JSON forbids.
    ByteOrderMark,
    /// The same key twice in one object, at this JSON pointer.
    DuplicateKey(String),
    /// Not JSON, or not the shape of a document (the parser's message).
    Json(String),
    /// The `format` key is not `tutti`.
    NotTutti(String),
    /// A major version above this crate's (spec 8.3), refused by name.
    HigherMajor {
        version: String,
        spec: Option<String>,
    },
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::Encoding => write!(f, "the document is not UTF-8"),
            ReadError::ByteOrderMark => write!(f, "the document starts with a byte-order mark"),
            ReadError::DuplicateKey(pointer) => write!(f, "duplicate key at {pointer}"),
            ReadError::Json(msg) => write!(f, "not a tutti document: {msg}"),
            ReadError::NotTutti(format) => write!(f, "the format is {format:?}, not \"tutti\""),
            ReadError::HigherMajor { version, spec } => match spec {
                Some(url) => write!(
                    f,
                    "tutti {version} is newer than this program reads (its spec: {url})"
                ),
                None => write!(f, "tutti {version} is newer than this program reads"),
            },
        }
    }
}

impl std::error::Error for ReadError {}

/// The first keys alone, to refuse before parsing the rest.
#[derive(Deserialize)]
struct Peek {
    format: String,
    version: String,
    #[serde(default)]
    spec: Option<String>,
}

/// Reads a document: checks, refusals, parse, [`Document::normalise`].
pub fn read(bytes: &[u8]) -> Result<Document, ReadError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ReadError::Encoding)?;
    if text.starts_with('\u{feff}') {
        return Err(ReadError::ByteOrderMark);
    }
    if let Some(pointer) = duplicate_key(text) {
        return Err(ReadError::DuplicateKey(pointer));
    }
    let peek: Peek = serde_json::from_str(text).map_err(|e| ReadError::Json(e.to_string()))?;
    if peek.format != crate::FORMAT {
        return Err(ReadError::NotTutti(peek.format));
    }
    if version_parts(&peek.version).is_some_and(|(major, _)| major > crate::MAJOR) {
        return Err(ReadError::HigherMajor {
            version: peek.version,
            spec: peek.spec,
        });
    }
    let mut document: Document =
        serde_json::from_str(text).map_err(|e| ReadError::Json(e.to_string()))?;
    document.normalise();
    Ok(document)
}

/// Whether a document opens read-only on this crate (spec 8.3): its `min_version` is a later
/// minor of this major than this crate implements.
pub fn is_read_only(document: &Document) -> bool {
    let Some((major, minor)) = document.min_version.as_deref().and_then(version_parts) else {
        return false;
    };
    let (own_major, own_minor) = version_parts(crate::VERSION).expect("the crate's version");
    major == own_major && minor > own_minor
}

/// The JSON pointer of the first duplicate key, scanning the text once. Keys are compared after
/// unescaping, so `"a"` and `"a"` are the same key.
fn duplicate_key(text: &str) -> Option<String> {
    enum Frame {
        Object {
            seen: HashSet<String>,
            key: Option<String>,
            expect_key: bool,
        },
        Array {
            index: usize,
        },
    }
    fn pointer(stack: &[Frame]) -> String {
        let mut out = String::new();
        for frame in stack {
            match frame {
                Frame::Object { key: Some(k), .. } => {
                    out.push('/');
                    out.push_str(&k.replace('~', "~0").replace('/', "~1"));
                }
                Frame::Object { key: None, .. } => {}
                Frame::Array { index } => {
                    out.push('/');
                    out.push_str(&index.to_string());
                }
            }
        }
        out
    }
    let mut stack: Vec<Frame> = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                stack.push(Frame::Object {
                    seen: HashSet::new(),
                    key: None,
                    expect_key: true,
                });
                i += 1;
            }
            b'[' => {
                stack.push(Frame::Array { index: 0 });
                i += 1;
            }
            b'}' | b']' => {
                stack.pop();
                i += 1;
            }
            b',' => {
                match stack.last_mut() {
                    Some(Frame::Object { expect_key, .. }) => *expect_key = true,
                    Some(Frame::Array { index }) => *index += 1,
                    None => {}
                }
                i += 1;
            }
            b':' => {
                i += 1;
            }
            b'"' => {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
                let token = &text[start..i.min(bytes.len())];
                let mut duplicate = false;
                if let Some(Frame::Object {
                    seen,
                    key,
                    expect_key,
                }) = stack.last_mut()
                {
                    if *expect_key {
                        let unescaped: String =
                            serde_json::from_str(token).unwrap_or_else(|_| token.to_owned());
                        duplicate = !seen.insert(unescaped.clone());
                        *key = Some(unescaped);
                        *expect_key = false;
                    }
                }
                if duplicate {
                    return Some(pointer(&stack));
                }
            }
            _ => i += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"{"format": "tutti", "version": "1.0", "generator": {"name": "test"},
        "ppq": 960, "tempo": [{"at": 0, "bpm": 120}], "meter": [{"at": 0, "numerator": 4, "denominator": 4}]}"#;

    #[test]
    fn the_smallest_document_reads() {
        let doc = read(MINIMAL.as_bytes()).unwrap();
        assert_eq!(doc.ppq, 960);
        assert_eq!(doc.tempo[0].bpm, 120.0);
        assert_eq!(doc.generator.name, "test");
        assert!(doc.tracks.is_empty());
    }

    #[test]
    fn refusals() {
        assert_eq!(read(b"\xff\xfe").unwrap_err(), ReadError::Encoding);
        assert_eq!(
            read("\u{feff}{}".as_bytes()).unwrap_err(),
            ReadError::ByteOrderMark
        );
        assert_eq!(
            read(br#"{"format": "tutti", "a": 1, "a": 2}"#).unwrap_err(),
            ReadError::DuplicateKey("/a".to_owned())
        );
        assert_eq!(
            read(br#"{"format": "tutti", "tracks": [{"id": "x", "name": "n", "name": "m"}]}"#)
                .unwrap_err(),
            ReadError::DuplicateKey("/tracks/0/name".to_owned())
        );
        assert_eq!(
            read(br#"{"format": "tutti", "o": {"a": 1, "a": 2}}"#).unwrap_err(),
            ReadError::DuplicateKey("/o/a".to_owned())
        );
        assert!(matches!(
            read(br#"{"format": "dawproject", "version": "1.0"}"#).unwrap_err(),
            ReadError::NotTutti(_)
        ));
        assert!(matches!(
            read(br#"{"format": "tutti", "version": "2.0", "spec": "https://x"}"#).unwrap_err(),
            ReadError::HigherMajor { .. }
        ));
        assert!(matches!(read(b"not json").unwrap_err(), ReadError::Json(_)));
    }

    #[test]
    fn a_later_minor_within_the_major_opens_but_a_later_min_version_is_read_only() {
        let later = MINIMAL.replace(
            r#""version": "1.0""#,
            r#""version": "1.7", "min_version": "1.7""#,
        );
        let doc = read(later.as_bytes()).unwrap();
        assert!(is_read_only(&doc));
        let doc = read(MINIMAL.as_bytes()).unwrap();
        assert!(!is_read_only(&doc));
    }
}
