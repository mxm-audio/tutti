//! The names a writer gives files: `A–Z a–z 0–9 - _` and one `.` (spec 2.1), so a name works on
//! every file system and in a URL. Names inside the document, of tracks and patterns, stay as
//! typed; only file names are cleaned. From newDAWn's store, relicensed MIT by its author.

use std::collections::BTreeSet;

/// The most characters a name takes from what was typed, so a project's own paths stay short.
pub const TYPED_MAX: usize = 60;

/// Names Windows keeps for devices, whatever their case or extension.
const RESERVED: [&str; 24] = [
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
    "COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// `typed` as a name's stem: a letter with a usual spelling in ASCII is spelled so (`é` → `e`,
/// `ø` → `oe`, `å` → `aa`, `ß` → `ss`), any other run of characters becomes one `-`, and `-` at
/// either end goes. At most [`TYPED_MAX`] characters; `fallback` when nothing is left, and a name
/// Windows keeps for a device gains a `_`.
pub fn clean_name(typed: &str, fallback: &str) -> String {
    let mut spelled = String::with_capacity(typed.len());
    let mut before = None;
    for c in typed.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
            spelled.push(c);
        } else if let Some(ascii) = ascii_spelling(c) {
            spelled.push_str(ascii);
        } else if ('\u{300}'..='\u{36f}').contains(&c) {
            // A mark on the letter before it, as macOS spells a name: dropped, as the letter it
            // makes is spelled, but for the two that are spelled with a letter of their own.
            match (c, before) {
                ('\u{308}', Some('a' | 'o' | 'u' | 'A' | 'O' | 'U')) => spelled.push('e'),
                ('\u{30a}', Some('a' | 'A')) => spelled.push('a'),
                _ => {}
            }
        } else {
            spelled.push('-');
        }
        before = Some(c);
    }
    let mut name = String::with_capacity(spelled.len());
    for c in spelled.chars() {
        if c != '-' || !(name.is_empty() || name.ends_with('-')) {
            name.push(c);
        }
    }
    // ASCII by now, so a character is a byte.
    name.truncate(TYPED_MAX);
    let mut name = name.trim_end_matches('-').to_owned();
    if name.is_empty() {
        name = fallback.to_owned();
    }
    if RESERVED.contains(&name.to_ascii_uppercase().as_str()) {
        name.push('_');
    }
    name
}

/// `stem.extension`, or the first of `stem_2.extension`, `stem_3.extension` and on that none of
/// `taken` is, ignoring case, as Windows and macOS do. No extension when `extension` is empty.
pub fn free_name<'a>(
    taken: impl IntoIterator<Item = &'a str>,
    stem: &str,
    extension: &str,
) -> String {
    let taken: BTreeSet<String> = taken.into_iter().map(str::to_lowercase).collect();
    let named = |stem: String| {
        if extension.is_empty() {
            stem
        } else {
            format!("{stem}.{extension}")
        }
    };
    std::iter::once(stem.to_owned())
        .chain((2..).map(|n| format!("{stem}_{n}")))
        .map(named)
        .find(|name| !taken.contains(&name.to_lowercase()))
        .expect("some number is free")
}

/// How a Latin letter outside ASCII is usually spelled in it.
fn ascii_spelling(c: char) -> Option<&'static str> {
    Some(match c {
        'À'..='Ã' => "A",
        'Ä' | 'Æ' => "Ae",
        'Å' => "Aa",
        'Ç' => "C",
        'È'..='Ë' => "E",
        'Ì'..='Ï' => "I",
        'Ð' => "D",
        'Ñ' => "N",
        'Ò'..='Õ' => "O",
        'Ö' | 'Ø' | 'Œ' => "Oe",
        'Ù'..='Û' => "U",
        'Ü' => "Ue",
        'Ý' | 'Ÿ' | 'Ŷ' => "Y",
        'Þ' => "Th",
        'ß' => "ss",
        'ẞ' => "SS",
        'à'..='ã' => "a",
        'ä' | 'æ' => "ae",
        'å' => "aa",
        'ç' => "c",
        'è'..='ë' => "e",
        'ì'..='ï' => "i",
        'ð' => "d",
        'ñ' => "n",
        'ò'..='õ' => "o",
        'ö' | 'ø' | 'œ' => "oe",
        'ù'..='û' => "u",
        'ü' => "ue",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'þ' => "th",
        'Ā' | 'Ă' | 'Ą' => "A",
        'ā' | 'ă' | 'ą' => "a",
        'Ć' | 'Ĉ' | 'Ċ' | 'Č' => "C",
        'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'Ď' | 'Đ' => "D",
        'ď' | 'đ' => "d",
        'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => "E",
        'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'Ĝ' | 'Ğ' | 'Ġ' | 'Ģ' => "G",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'Ĥ' | 'Ħ' => "H",
        'ĥ' | 'ħ' => "h",
        'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => "I",
        'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'Ĳ' => "IJ",
        'ĳ' => "ij",
        'Ĵ' => "J",
        'ĵ' => "j",
        'Ķ' => "K",
        'ķ' | 'ĸ' => "k",
        'Ĺ' | 'Ļ' | 'Ľ' | 'Ŀ' | 'Ł' => "L",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'Ń' | 'Ņ' | 'Ň' | 'Ŋ' => "N",
        'ń' | 'ņ' | 'ň' | 'ŉ' | 'ŋ' => "n",
        'Ō' | 'Ŏ' | 'Ő' => "O",
        'ō' | 'ŏ' | 'ő' => "o",
        'Ŕ' | 'Ŗ' | 'Ř' => "R",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'Ś' | 'Ŝ' | 'Ş' | 'Š' => "S",
        'ś' | 'ŝ' | 'ş' | 'š' | 'ſ' => "s",
        'Ţ' | 'Ť' | 'Ŧ' => "T",
        'ţ' | 'ť' | 'ŧ' => "t",
        'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => "U",
        'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'Ŵ' => "W",
        'ŵ' => "w",
        'Ź' | 'Ż' | 'Ž' => "Z",
        'ź' | 'ż' | 'ž' => "z",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_keeps_to_the_strict_characters() {
        let cases = [
            ("Vox: take 1/2?", "Vox-take-1-2"),
            ("Kør", "Koer"),
            ("Min sang", "Min-sang"),
            ("Café crème", "Cafe-creme"),
            ("Ærø Århus straße", "Aeroe-Aarhus-strasse"),
            ("  --a  --  b--  ", "a-b"),
            ("Lead_vocals-2", "Lead_vocals-2"),
            ("日本語の歌", "Untitled"),
            ("Song ♪ 日本 two", "Song-two"),
            ("../..", "Untitled"),
            ("", "Untitled"),
        ];
        for (typed, clean) in cases {
            assert_eq!(clean_name(typed, "Untitled"), clean, "{typed:?}");
        }
    }

    #[test]
    fn a_name_spelled_as_macos_spells_it_cleans_as_one_spelled_whole() {
        let decomposed = "Ka\u{30a}re Mu\u{308}ller cafe\u{301}";
        assert_eq!(clean_name(decomposed, "Untitled"), "Kaare-Mueller-cafe");
        assert_eq!(
            clean_name("Kåre Müller café", "Untitled"),
            "Kaare-Mueller-cafe"
        );
    }

    #[test]
    fn a_device_name_gains_an_underscore_and_a_long_one_is_cut() {
        assert_eq!(clean_name("con", "Untitled"), "con_");
        assert_eq!(clean_name("COM1", "Untitled"), "COM1_");
        assert_eq!(clean_name("Console", "Untitled"), "Console");
        let long = clean_name(&"ab ".repeat(40), "Untitled");
        assert!(long.len() <= TYPED_MAX, "{long}");
        assert!(!long.ends_with('-'), "{long}");
    }

    #[test]
    fn a_free_name_is_unique_ignoring_case() {
        let taken = ["untitled.tutti", "UNTITLED_2.tutti", "other"];
        assert_eq!(free_name(taken, "Untitled", "tutti"), "Untitled_3.tutti");
        assert_eq!(free_name(taken, "Song", "tutti"), "Song.tutti");
        assert_eq!(free_name(taken, "Other", ""), "Other_2");
    }
}
