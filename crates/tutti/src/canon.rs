//! The two canonical forms (spec 3.1): JCS, RFC 8785, which equality and fingerprints are over;
//! and the pretty form a writer puts on disk, which shares JCS's number and string rendering.

use std::cmp::Ordering;
use std::fmt::Write as _;

use serde_json::{Map, Number, Value};

/// The arrays whose elements are objects with arrays of their own, written multi-line; every
/// other array element is written on one line (spec 3.1).
pub const MULTILINE_ELEMENTS: &[&str] =
    &["tracks", "children", "patterns", "devices", "automation"];

/// Keys compare by UTF-16 code units, as JCS sorts them.
pub fn utf16_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// A number as ECMAScript's Number-to-string renders it, which JCS prescribes: integers as
/// digits, other values with the shortest digits that round-trip, a plain decimal between
/// 1e-6 and 1e21, an exponent outside that.
pub fn number(n: &Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    es_number(n.as_f64().unwrap_or(0.0))
}

/// ECMAScript's Number-to-string (ECMA-262 §6.1.6.1.20) for a finite double. Rust's `{:e}`
/// gives the shortest digits that round-trip; this lays them out as ECMAScript does.
pub fn es_number(x: f64) -> String {
    if x == 0.0 || !x.is_finite() {
        return "0".to_owned();
    }
    let negative = x < 0.0;
    let sci = format!("{:e}", x.abs());
    let (mantissa, exponent) = sci.split_once('e').expect("{:e} has an exponent");
    let exponent: i32 = exponent.parse().expect("a decimal exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exponent + 1;
    let mut out = String::with_capacity(24);
    if negative {
        out.push('-');
    }
    if k <= n && n <= 21 {
        out.push_str(&digits);
        for _ in 0..(n - k) {
            out.push('0');
        }
    } else if 0 < n && n <= 21 {
        out.push_str(&digits[..n as usize]);
        out.push('.');
        out.push_str(&digits[n as usize..]);
    } else if -6 < n && n <= 0 {
        out.push_str("0.");
        for _ in 0..(-n) {
            out.push('0');
        }
        out.push_str(&digits);
    } else {
        out.push_str(&digits[..1]);
        if k > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if n >= 1 { '+' } else { '-' });
        write!(out, "{}", (n - 1).abs()).expect("a String never fails");
    }
    out
}

/// A string as JCS writes it: `"` and `\` escaped, the five short escapes for backspace,
/// form feed, newline, return and tab, `\u00xx` (lower case) for the other control characters,
/// everything else literal.
pub fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                write!(out, "\\u{:04x}", c as u32).expect("a String never fails")
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// The JCS bytes of a value: keys sorted by UTF-16 code units, no whitespace.
pub fn jcs(value: &Value) -> String {
    let mut out = String::new();
    jcs_into(&mut out, value);
    out
}

fn jcs_into(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&number(n)),
        Value::String(s) => string(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                jcs_into(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| utf16_cmp(a, b));
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                string(out, key);
                out.push(':');
                jcs_into(out, &map[*key]);
            }
            out.push('}');
        }
    }
}

/// The pretty form of a value whose objects are already in the spec's key order (a `Document`
/// serialised to a `Value` is): two-space indentation, LF, one final LF, scalars' arrays on one
/// line, an array's objects one per line and compact, but the elements of
/// [`MULTILINE_ELEMENTS`], which are written multi-line.
pub fn pretty(value: &Value) -> String {
    let mut out = String::new();
    pretty_into(&mut out, value, 0, None);
    out.push('\n');
    out
}

fn pretty_into(out: &mut String, value: &Value, indent: usize, key: Option<&str>) {
    match value {
        Value::Object(map) if map.is_empty() => out.push_str("{}"),
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Object(map) => {
            out.push_str("{\n");
            let last = map.len() - 1;
            for (i, (k, v)) in map.iter().enumerate() {
                push_indent(out, indent + 2);
                string(out, k);
                out.push_str(": ");
                pretty_into(out, v, indent + 2, Some(k));
                out.push_str(if i < last { ",\n" } else { "\n" });
            }
            push_indent(out, indent);
            out.push('}');
        }
        Value::Array(items) => {
            if items
                .iter()
                .all(|item| !matches!(item, Value::Array(_) | Value::Object(_)))
            {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    compact_into(out, item);
                }
                out.push(']');
                return;
            }
            let multiline = key.is_some_and(|k| MULTILINE_ELEMENTS.contains(&k));
            out.push_str("[\n");
            let last = items.len() - 1;
            for (i, item) in items.iter().enumerate() {
                push_indent(out, indent + 2);
                if multiline {
                    pretty_into(out, item, indent + 2, None);
                } else {
                    compact_into(out, item);
                }
                out.push_str(if i < last { ",\n" } else { "\n" });
            }
            push_indent(out, indent);
            out.push(']');
        }
        scalar => compact_into(out, scalar),
    }
}

/// One line, one space after each `:` and `,`, keys in the map's order.
pub fn compact(value: &Value) -> String {
    let mut out = String::new();
    compact_into(&mut out, value);
    out
}

fn compact_into(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&number(n)),
        Value::String(s) => string(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                compact_into(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            for (i, (k, v)) in map.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                string(out, k);
                out.push_str(": ");
                compact_into(out, v);
            }
            out.push('}');
        }
    }
}

fn push_indent(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push(' ');
    }
}

/// Sorts an object's keys, and every object's below it, by UTF-16 code units, in place.
pub fn sort_keys(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = std::mem::take(map).into_iter().collect();
            entries.sort_by(|(a, _), (b, _)| utf16_cmp(a, b));
            let mut sorted = Map::new();
            for (k, mut v) in entries {
                sort_keys(&mut v);
                sorted.insert(k, v);
            }
            *map = sorted;
        }
        Value::Array(items) => items.iter_mut().for_each(sort_keys),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_render_as_ecmascript_does() {
        for (x, want) in [
            (1.0, "1"),
            (0.5, "0.5"),
            (218.4253, "218.4253"),
            (-12.0, "-12"),
            (0.0, "0"),
            (-0.0, "0"),
            (1e21, "1e+21"),
            (1e-7, "1e-7"),
            (0.000001, "0.000001"),
            (123456789012345680000.0, "123456789012345680000"),
            (1.5e300, "1.5e+300"),
            (5e-324, "5e-324"),
            (0.1 + 0.2, "0.30000000000000004"),
        ] {
            assert_eq!(es_number(x), want, "{x}");
        }
        assert_eq!(number(&Number::from(960)), "960");
        assert_eq!(number(&Number::from_f64(87.0).unwrap()), "87");
    }

    #[test]
    fn strings_escape_as_jcs_does() {
        let mut out = String::new();
        string(&mut out, "a\"b\\c\n\t\u{1}\u{7f}é/");
        assert_eq!(out, "\"a\\\"b\\\\c\\n\\t\\u0001\u{7f}é/\"");
    }

    #[test]
    fn jcs_sorts_keys_by_utf16_units_and_drops_whitespace() {
        let v =
            json!({"b": [1, 2.5, "x"], "a": {"z": null, "y": true}, "\u{1F600}": 0, "\u{FF5E}": 1});
        // U+FF5E is one code unit (FF5E); U+1F600 is a surrogate pair starting D83D, which sorts first.
        assert_eq!(
            jcs(&v),
            r#"{"a":{"y":true,"z":null},"b":[1,2.5,"x"],"😀":0,"～":1}"#
        );
    }

    #[test]
    fn the_pretty_form() {
        let v = json!({
            "format": "tutti",
            "tempo": [{"at": 0, "bpm": 87.0}],
            "extensions_used": ["dk.mxm.newdawn"],
            "tracks": [{"id": "trk_1", "parts": [{"id": "prt_1", "at": 0}], "children": []}],
            "extensions": {}
        });
        let want = "{\n  \"format\": \"tutti\",\n  \"tempo\": [\n    {\"at\": 0, \"bpm\": 87}\n  ],\n  \
                    \"extensions_used\": [\"dk.mxm.newdawn\"],\n  \"tracks\": [\n    {\n      \"id\": \"trk_1\",\n      \
                    \"parts\": [\n        {\"id\": \"prt_1\", \"at\": 0}\n      ],\n      \"children\": []\n    }\n  ],\n  \
                    \"extensions\": {}\n}\n";
        assert_eq!(pretty(&v), want);
    }
}
