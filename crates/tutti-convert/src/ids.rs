use sha2::{Digest, Sha256};
use tutti::render::sha256_hex;
use tutti::{Document, Id};

use crate::source::hex;

/// Hex digits of the hash in an id that could not be spelled from its parts.
const HASHED_DIGITS: usize = 32;

/// An id from the source's own identities (a track's number, a pattern's), the same at every
/// conversion of the same source, so converting an unchanged project again writes the same
/// document.
///
/// `prefix_a_b` when the prefix and every part are made of the characters an id may hold
/// (spec 5.1) and hold no `_` (so `["a_b"]` and `["a", "b"]` never meet) and the whole fits;
/// otherwise `prefix_<hash>`, the hash over the prefix and the parts (`id_<hash>` when the
/// prefix itself can't stand). Either way a well-formed id; distinct inputs give distinct ids
/// but by a hash collision.
pub fn derived_id(prefix: &str, parts: &[&str]) -> Id {
    let plain = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if plain(prefix) && parts.iter().all(|p| plain(p)) {
        let mut text = prefix.to_owned();
        for part in parts {
            text.push('_');
            text.push_str(part);
        }
        if let Ok(id) = Id::new(&text) {
            return id;
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    for part in parts {
        hasher.update([0x1f]);
        hasher.update(part.as_bytes());
    }
    let digest = hex(&hasher.finalize());
    let head = if plain(prefix) && prefix.len() + 1 + HASHED_DIGITS <= tutti::id::MAX_LEN {
        prefix
    } else {
        "id"
    };
    Id::new(&format!("{head}_{}", &digest[..HASHED_DIGITS]))
        .expect("a plain head and hex digits make a well-formed id")
}

/// The document's fingerprint: the SHA-256 of its JCS with `generator` left out, the bytes
/// `tutti::equal` compares, so two documents have the same fingerprint exactly when they are
/// equal as the spec means it (3.1). What a watcher records of an output's document.
pub fn fingerprint(document: &Document) -> String {
    let mut value = tutti::to_value(document);
    if let Some(map) = value.as_object_mut() {
        map.remove("generator");
    }
    sha256_hex(tutti::canon::jcs(&value).as_bytes())
}
