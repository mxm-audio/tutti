//! A render's fingerprint (spec 7.7): the SHA-256 of the JCS bytes of one descriptor object.

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::canon;
use crate::model::{Document, Track};

/// SHA-256 of bytes as 64 lower-case hexadecimal digits.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// The descriptor a render's fingerprint is over: the track without `renders`, `channel` and
/// `children`; the render's `from` and `to`; the document's `ppq`, `tempo`, `meter` and `key`.
pub fn render_descriptor(doc: &Document, track: &Track, from: i64, to: i64) -> Value {
    let mut track_value = serde_json::to_value(track).expect("a track always serialises");
    if let Value::Object(map) = &mut track_value {
        map.remove("renders");
        map.remove("channel");
        map.remove("children");
    }
    let doc_value = serde_json::to_value(doc).expect("a document always serialises");
    let field = |name: &str| {
        doc_value
            .get(name)
            .cloned()
            .unwrap_or(Value::Array(Vec::new()))
    };
    let mut descriptor = Map::new();
    descriptor.insert("track".to_owned(), track_value);
    descriptor.insert("from".to_owned(), json!(from));
    descriptor.insert("to".to_owned(), json!(to));
    descriptor.insert("ppq".to_owned(), json!(doc.ppq));
    descriptor.insert("tempo".to_owned(), field("tempo"));
    descriptor.insert("meter".to_owned(), field("meter"));
    descriptor.insert("key".to_owned(), field("key"));
    Value::Object(descriptor)
}

/// The fingerprint of a render of `track` from `from` to `to` ticks (spec 7.7).
pub fn render_fingerprint(doc: &Document, track: &Track, from: i64, to: i64) -> String {
    sha256_hex(canon::jcs(&render_descriptor(doc, track, from, to)).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read;

    #[test]
    fn the_fingerprint_follows_the_chain_and_the_maps_but_not_the_channel() {
        let text = r#"{"format": "tutti", "version": "1.0", "generator": {"name": "t"}, "ppq": 960,
            "tempo": [{"at": 0, "bpm": 120}], "meter": [{"at": 0, "numerator": 4, "denominator": 4}],
            "tracks": [{"id": "t", "channel": {"gain": 0.5}, "patterns": [{"id": "p", "length": 960}]}]}"#;
        let doc = read(text.as_bytes()).unwrap();
        let a = render_fingerprint(&doc, &doc.tracks[0], 0, 960);
        assert_eq!(a.len(), 64);
        let mut quieter = doc.clone();
        quieter.tracks[0].channel.gain = 0.25;
        assert_eq!(
            render_fingerprint(&quieter, &quieter.tracks[0], 0, 960),
            a,
            "the channel is not in it"
        );
        let mut faster = doc.clone();
        faster.tempo[0].bpm = 121.0;
        assert_ne!(
            render_fingerprint(&faster, &faster.tracks[0], 0, 960),
            a,
            "the tempo map is"
        );
        assert_ne!(
            render_fingerprint(&doc, &doc.tracks[0], 0, 1920),
            a,
            "the range is"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
