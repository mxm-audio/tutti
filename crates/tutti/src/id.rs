//! Ids (spec 5.1): opaque strings, unique across a document, never renumbered.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The characters an id may hold, and its length: `^[A-Za-z0-9_-]{1,64}$`.
pub const MAX_LEN: usize = 64;

/// An id as read. Its form is only checked by `validate`, so a document with a malformed id
/// still loads and reports it; `Id::new` checks on construction for a program's own ids.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(pub String);

impl Id {
    /// An id of a program's own making, checked against the form.
    pub fn new(text: &str) -> Result<Self, IdError> {
        check(text)?;
        Ok(Self(text.to_owned()))
    }

    /// Whether this id has the form the spec requires.
    pub fn is_well_formed(&self) -> bool {
        check(&self.0).is_ok()
    }

    /// The recommended form: a kind prefix and a ULID, from a Unix time in milliseconds and 80
    /// bits of randomness the caller supplies (the crate has no clock and no random source).
    /// `prefix` is `trk`, `pat`, `prt`, `evt`, `dev`, `aut`, `mrk` or `scn`, or a program's own.
    pub fn ulid(prefix: &str, unix_millis: u64, random: u128) -> Self {
        const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        let value: u128 =
            ((unix_millis as u128 & ((1u128 << 48) - 1)) << 80) | (random & ((1u128 << 80) - 1));
        let mut chars = [b'0'; 26];
        let mut v = value;
        for slot in chars.iter_mut().rev() {
            *slot = ALPHABET[(v & 31) as usize];
            v >>= 5;
        }
        Self(format!(
            "{prefix}_{}",
            std::str::from_utf8(&chars).expect("ASCII")
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Id {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// Why a text is not an id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdError {
    Empty,
    TooLong,
    /// The character that is not `A-Z a-z 0-9 - _`.
    Character(char),
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdError::Empty => write!(f, "an id is empty"),
            IdError::TooLong => write!(f, "an id is longer than {MAX_LEN} characters"),
            IdError::Character(c) => {
                write!(f, "an id holds {c:?}; only A-Z a-z 0-9 - _ are allowed")
            }
        }
    }
}

impl std::error::Error for IdError {}

/// Whether `text` matches `^[A-Za-z0-9_-]{1,64}$`.
pub fn check(text: &str) -> Result<(), IdError> {
    if text.is_empty() {
        return Err(IdError::Empty);
    }
    if text.len() > MAX_LEN {
        return Err(IdError::TooLong);
    }
    match text
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '-' || *c == '_'))
    {
        Some(c) => Err(IdError::Character(c)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_form() {
        assert!(Id::new("trk_01JB2K7Q9M3F5A7Y9R2E4W6N").is_ok());
        assert!(Id::new("master").is_ok());
        assert_eq!(Id::new(""), Err(IdError::Empty));
        assert_eq!(Id::new("a b"), Err(IdError::Character(' ')));
        assert_eq!(Id::new(&"x".repeat(65)), Err(IdError::TooLong));
        assert!(Id::new(&"x".repeat(64)).is_ok());
    }

    #[test]
    fn a_ulid_is_26_crockford_characters_after_the_prefix() {
        let id = Id::ulid("evt", 1_700_000_000_000, 0x1234_5678_9abc_def0_1234);
        let (prefix, ulid) = id.as_str().split_once('_').unwrap();
        assert_eq!(prefix, "evt");
        assert_eq!(ulid.len(), 26);
        assert!(id.is_well_formed());
        // The time part sorts first: a later millisecond is a greater id.
        let later = Id::ulid("evt", 1_700_000_000_001, 0);
        assert!(later > id);
    }
}
