//! What plays, worked out from the document alone (spec 6.6, 6.12, 7.1): a track's notes from its
//! parts, the notes an instrument receives from the folders above it through their members'
//! zones, and the chokes among a folder's members. Every reader gets the same, whatever order it
//! walks the tracks in; a conformance fixture's expected notes are made with it.
//!
//! Note effects are plugins, so this leaves them out: it is what reaches an instrument when the
//! folders on its path have none, as a conformance fixture's do.

use std::collections::HashMap;

use crate::id::Id;
use crate::model::{Document, Event, Pattern, Role, Ticks, Track};

/// A note as it plays on a track, in absolute ticks.
#[derive(Clone, Debug, PartialEq)]
pub struct Sounding {
    /// The track it plays on.
    pub track: Id,
    /// The track whose pattern holds the event: the track itself, or a folder above it.
    pub from: Id,
    /// The note's (or chord's) id.
    pub event: Id,
    pub at: Ticks,
    /// Where it stops sounding: its length's end, or its part's end when that comes first (6.6).
    pub end: Ticks,
    pub pitch: u8,
    pub velocity: f64,
}

/// A note choked: its voice stops at `at`, without its release (6.12).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choke {
    /// The track the choked note plays on.
    pub track: Id,
    /// The choked note's (or chord's) id.
    pub event: Id,
    pub pitch: u8,
    /// Where the choked note started.
    pub start: Ticks,
    pub at: Ticks,
}

/// The notes of a track's own parts (6.6), those for an instrument: notes without audio, and a
/// chord's voicing. Sorted by start, then event id, then pitch.
pub fn own(track: &Track) -> Vec<Sounding> {
    let patterns: HashMap<&str, &Pattern> =
        track.patterns.iter().map(|p| (p.id.as_str(), p)).collect();
    let mut out = Vec::new();
    for part in track.parts.iter().filter(|p| !p.mute && p.length > 0) {
        let Some(pattern) = patterns.get(part.pattern.as_str()) else {
            continue;
        };
        let part_end = part.at + part.length;
        // The pattern's spans as they play: from `offset`, and on reaching a loop's end, from its
        // start again, for the part's length.
        let mut spans: Vec<(Ticks, Ticks, Ticks)> = Vec::new(); // (pattern from, pattern to, track at)
        match &part.r#loop {
            Some(l) if l.start < l.end && part.offset <= l.end => {
                let mut at = part.at;
                let mut from = part.offset;
                while at < part_end {
                    let to = (l.end).min(from + (part_end - at));
                    spans.push((from, to, at));
                    at += to - from;
                    from = l.start;
                }
            }
            _ => spans.push((part.offset, part.offset + part.length, part.at)),
        }
        for (from, to, at) in spans {
            for event in &pattern.events {
                let (id, start, length, pitches, velocity) = match event {
                    Event::Note(n) if n.audio.is_none() => {
                        (&n.id, n.at, n.length, vec![n.pitch], n.velocity)
                    }
                    Event::Chord(c) => (&c.id, c.at, c.length, c.notes.clone(), c.velocity),
                    _ => continue,
                };
                if start < from || start >= to {
                    continue;
                }
                let begins = at + (start - from);
                let ends = (begins + length).min(part_end);
                if ends <= begins {
                    continue;
                }
                for pitch in pitches {
                    out.push(Sounding {
                        track: track.id.clone(),
                        from: track.id.clone(),
                        event: id.clone(),
                        at: begins,
                        end: ends,
                        pitch,
                        velocity,
                    });
                }
            }
        }
    }
    sort(&mut out);
    out
}

fn sort(notes: &mut [Sounding]) {
    notes.sort_by(|a, b| (a.at, a.event.as_str(), a.pitch).cmp(&(b.at, b.event.as_str(), b.pitch)));
}

/// The path from the top to the track with `id`, that track last; empty when it isn't held.
pub(crate) fn path<'a>(doc: &'a Document, id: &Id) -> Vec<&'a Track> {
    fn walk<'a>(tracks: &'a [Track], id: &Id, path: &mut Vec<&'a Track>) -> bool {
        for t in tracks {
            path.push(t);
            if &t.id == id || walk(&t.children, id, path) {
                return true;
            }
            path.pop();
        }
        false
    }
    let mut out = Vec::new();
    walk(&doc.tracks, id, &mut out);
    out
}

/// The notes that reach the instrument on `track` (7.1, 6.12): its own, then each folder's above
/// it, each passing the zones of the members between that folder and the track, their transposes
/// adding up. Sorted by start, then the folders' order from the top (the track's own last), then
/// event id.
pub fn reaching(doc: &Document, track: &Track) -> Vec<Sounding> {
    let line = path(doc, &track.id);
    let mut out = own(track);
    if line.len() >= 2 {
        for (k, folder) in line[..line.len() - 1].iter().enumerate() {
            'note: for mut n in own(folder) {
                for member in &line[k + 1..] {
                    match member.zone.as_ref() {
                        None => {}
                        Some(z) => match z.pass(n.pitch, n.velocity) {
                            Some(p) => n.pitch = p,
                            None => continue 'note,
                        },
                    }
                }
                n.track = track.id.clone();
                out.push(n);
            }
        }
    }
    let order: HashMap<&str, usize> = line
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), i))
        .collect();
    out.sort_by(|a, b| {
        let oa = order.get(a.from.as_str()).copied().unwrap_or(usize::MAX);
        let ob = order.get(b.from.as_str()).copied().unwrap_or(usize::MAX);
        (a.at, oa, a.event.as_str(), a.pitch).cmp(&(b.at, ob, b.event.as_str(), b.pitch))
    });
    out
}

fn has_instrument(track: &Track) -> bool {
    track
        .devices
        .iter()
        .any(|d| d.enabled && d.effective_role() == Role::Instrument)
}

/// The tracks with an instrument at or inside `track`, in document order.
fn instruments<'a>(track: &'a Track, out: &mut Vec<&'a Track>) {
    if has_instrument(track) {
        out.push(track);
    }
    for c in &track.children {
        instruments(c, out);
    }
}

/// Every track, in document order (depth first), with its place.
fn places(doc: &Document) -> HashMap<String, usize> {
    fn walk(tracks: &[Track], out: &mut HashMap<String, usize>) {
        for t in tracks {
            let n = out.len();
            out.insert(t.id.as_str().to_owned(), n);
            walk(&t.children, out);
        }
    }
    let mut out = HashMap::new();
    walk(&doc.tracks, &mut out);
    out
}

/// Every choke in the document (6.12), in the order a player sends them: by tick, then the
/// choked note's start, then its track's place in the document, then its id and pitch. Each note
/// sounding is a voice of its own, so two parts playing one event over each other give two. A
/// voice is choked once, at the first start that chokes it; a voice choked is over.
pub fn chokes(doc: &Document) -> Vec<Choke> {
    fn all<'a>(tracks: &'a [Track], out: &mut Vec<&'a Track>) {
        for t in tracks {
            out.push(t);
            all(&t.children, out);
        }
    }
    let mut every = Vec::new();
    all(&doc.tracks, &mut every);
    // The voices reaching each instrument, worked out once, each known by its track and its place
    // in that track's list.
    let reached: HashMap<&str, Vec<Sounding>> = every
        .iter()
        .filter(|t| has_instrument(t))
        .map(|t| (t.id.as_str(), reaching(doc, t)))
        .collect();
    // A member's voices: every one playing on an instrument at or inside it.
    let member_voices = |member: &Track| -> Vec<(&str, usize, &Sounding)> {
        let mut inside = Vec::new();
        instruments(member, &mut inside);
        inside
            .iter()
            .filter_map(|t| reached.get_key_value(t.id.as_str()))
            .flat_map(|(&track, voices)| {
                voices
                    .iter()
                    .enumerate()
                    .map(move |(place, voice)| (track, place, voice))
            })
            .collect()
    };
    // The first tick each voice is choked at.
    let mut first: HashMap<(&str, usize), Ticks> = HashMap::new();
    for folder in every.iter().filter(|t| !t.children.is_empty()) {
        let members: Vec<(&Track, u8)> = folder
            .children
            .iter()
            .map(|m| (m, m.zone.as_ref().map_or(0, |z| z.choke)))
            .collect();
        for (i, &(member, group)) in members.iter().enumerate() {
            if group == 0 {
                continue;
            }
            let starts: Vec<Ticks> = member_voices(member)
                .iter()
                .map(|(_, _, voice)| voice.at)
                .collect();
            for (j, &(other, other_group)) in members.iter().enumerate() {
                if i == j || other_group != group {
                    continue;
                }
                for (track, place, voice) in member_voices(other) {
                    // Sounding at t, and started before it: at < t < end.
                    if let Some(&t) = starts
                        .iter()
                        .filter(|&&t| voice.at < t && t < voice.end)
                        .min()
                    {
                        first
                            .entry((track, place))
                            .and_modify(|e| *e = (*e).min(t))
                            .or_insert(t);
                    }
                }
            }
        }
    }
    let place = places(doc);
    let mut out: Vec<Choke> = first
        .into_iter()
        .map(|((track, at_place), at)| {
            let voice = &reached[track][at_place];
            Choke {
                track: voice.track.clone(),
                event: voice.event.clone(),
                pitch: voice.pitch,
                start: voice.at,
                at,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        let pa = place.get(a.track.as_str()).copied().unwrap_or(usize::MAX);
        let pb = place.get(b.track.as_str()).copied().unwrap_or(usize::MAX);
        (a.at, a.start, pa, a.event.as_str(), a.pitch).cmp(&(
            b.at,
            b.start,
            pb,
            b.event.as_str(),
            b.pitch,
        ))
    });
    out
}

/// The members whose notes can choke a track's (6.12): at each folder on its path whose member on
/// that path is in a choke group, the folder's other members in the group; the outermost folder's
/// first, each folder's in its order.
pub fn choking<'a>(doc: &'a Document, track: &Track) -> Vec<&'a Track> {
    let line = path(doc, &track.id);
    let mut out = Vec::new();
    for pair in line.windows(2) {
        let (folder, member) = (pair[0], pair[1]);
        let group = member.zone.as_ref().map_or(0, |z| z.choke);
        if group == 0 {
            continue;
        }
        out.extend(folder.children.iter().filter(|other| {
            other.id != member.id && other.zone.as_ref().is_some_and(|z| z.choke == group)
        }));
    }
    out
}

/// The chokes on one track's notes (6.12), in [`chokes`]' order: what the other members of a
/// choke group it's in do to its sound. None, and nothing worked out, when no member can choke it.
pub fn chokes_on(doc: &Document, track: &Track) -> Vec<Choke> {
    if choking(doc, track).is_empty() {
        return Vec::new();
    }
    chokes(doc)
        .into_iter()
        .filter(|choke| choke.track == track.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Device, Generator, LoopRange, Note, Part, Zone};

    fn id(s: &str) -> Id {
        Id::new(s).unwrap()
    }

    fn note(n: &str, at: Ticks, length: Ticks, pitch: u8, velocity: f64) -> Event {
        Event::Note(Note::new(id(n), at, length, pitch, velocity))
    }

    /// A track holding one pattern of `events`, `length` long, in one part at 0 of `part` ticks.
    fn holding(name: &str, events: Vec<Event>, length: Ticks, part: Ticks) -> Track {
        let mut t = Track::new(id(&format!("trk_{name}")), name);
        let mut p = Pattern::new(id(&format!("pat_{name}")), length);
        p.events = events;
        t.patterns.push(p);
        t.parts.push(Part::new(
            id(&format!("prt_{name}")),
            id(&format!("pat_{name}")),
            0,
            part,
        ));
        t
    }

    fn instrument(name: &str, zone: Option<Zone>) -> Track {
        let mut t = Track::new(id(&format!("trk_{name}")), name);
        t.devices.push(Device::new(
            id(&format!("dev_{name}")),
            "instrument",
            "clap:org.example.x",
        ));
        t.zone = zone;
        t
    }

    fn zone(keys: [u8; 2], transpose: i32, choke: u8) -> Option<Zone> {
        Some(Zone {
            keys,
            transpose,
            choke,
            ..Zone::default()
        })
    }

    fn doc(tracks: Vec<Track>) -> Document {
        let mut d = Document::new(Generator::new("t", "1", "https://example.org"));
        d.tracks = tracks;
        d
    }

    fn pitches(doc: &Document, name: &str) -> Vec<(Ticks, u8)> {
        let line = path(doc, &id(&format!("trk_{name}")));
        reaching(doc, line.last().unwrap())
            .iter()
            .map(|n| (n.at, n.pitch))
            .collect()
    }

    #[test]
    fn a_part_plays_from_its_offset_loops_and_cuts_at_its_end() {
        // A pattern 960 long: notes at 0 and 480, each 400 long. A part of 1 200 from offset 480,
        // looping 0 to 960: the note at 480 at 0, the note at 0 at 480, the note at 480 at 960,
        // cut at 1 200.
        let mut t = holding(
            "a",
            vec![
                note("evt_0", 0, 400, 60, 1.0),
                note("evt_1", 480, 400, 62, 1.0),
            ],
            960,
            1200,
        );
        t.parts[0].offset = 480;
        t.parts[0].r#loop = Some(LoopRange {
            start: 0,
            end: 960,
            unknown: Default::default(),
        });
        let got: Vec<(Ticks, Ticks, u8)> = own(&t).iter().map(|n| (n.at, n.end, n.pitch)).collect();
        assert_eq!(got, [(0, 400, 62), (480, 880, 60), (960, 1200, 62)]);
        t.parts[0].mute = true;
        assert!(own(&t).is_empty(), "a muted part plays nothing");
    }

    /// A part starting at its loop's end reaches it at once and goes on from its start (6.6).
    #[test]
    fn a_part_starting_at_its_loops_end_plays_from_its_start() {
        let mut t = holding(
            "a",
            vec![
                note("evt_0", 0, 100, 60, 1.0),
                note("evt_1", 480, 100, 62, 1.0),
            ],
            960,
            960,
        );
        t.parts[0].offset = 480;
        t.parts[0].r#loop = Some(LoopRange {
            start: 0,
            end: 480,
            unknown: Default::default(),
        });
        let got: Vec<(Ticks, u8)> = own(&t).iter().map(|n| (n.at, n.pitch)).collect();
        assert_eq!(
            got,
            [(0, 60), (480, 60)],
            "the loop 0 to 480, twice; 480 never heard"
        );
    }

    #[test]
    fn a_zone_takes_its_keys_and_velocities_both_ends_in_and_drops_what_its_transpose_takes_past_the_ends()
     {
        let mut folder = holding(
            "kit",
            vec![
                note("evt_a", 0, 10, 35, 1.0),
                note("evt_b", 10, 10, 36, 1.0),
                note("evt_c", 20, 10, 40, 0.5),
                note("evt_d", 30, 10, 41, 1.0),
                note("evt_e", 40, 10, 38, 0.4),
                note("evt_f", 50, 10, 127, 1.0),
                note("evt_g", 60, 10, 0, 1.0),
            ],
            100,
            100,
        );
        let mut range = Zone {
            keys: [36, 40],
            velocities: [0.5, 1.0],
            transpose: 2,
            ..Zone::default()
        };
        folder.children.push(instrument("in", Some(range.clone())));
        range.keys = [0, 127];
        range.velocities = [0.0, 1.0];
        range.transpose = 1;
        folder.children.push(instrument("up", Some(range.clone())));
        range.transpose = -1;
        folder.children.push(instrument("down", Some(range)));
        let d = doc(vec![folder]);
        // 36 and 40 (velocity 0.5) in, 35, 41 and the 0.4 out; each two up.
        assert_eq!(pitches(&d, "in"), [(10, 38), (20, 42)]);
        // 127 up one is dropped, 0 down one is dropped.
        assert!(!pitches(&d, "up").iter().any(|&(at, _)| at == 50));
        assert_eq!(pitches(&d, "up").len(), 6);
        assert_eq!(pitches(&d, "down").len(), 6);
        assert!(!pitches(&d, "down").iter().any(|&(at, _)| at == 60));
    }

    #[test]
    fn zones_down_a_tree_add_up_and_a_members_own_notes_pass_none() {
        // Outer's note 36 reaches Inner (+12) then Leaf (+12, keys 48 only): 60.
        let mut outer = holding("outer", vec![note("evt_o", 0, 10, 36, 1.0)], 100, 100);
        let mut inner = Track::new(id("trk_inner"), "inner");
        inner.zone = zone([0, 127], 12, 0);
        let mut leaf = instrument("leaf", zone([48, 48], 12, 0));
        let mut p = Pattern::new(id("pat_leaf"), 100);
        p.events = vec![note("evt_own", 5, 10, 90, 1.0)];
        leaf.patterns.push(p);
        leaf.parts
            .push(Part::new(id("prt_leaf"), id("pat_leaf"), 0, 100));
        inner.children.push(leaf);
        outer.children.push(inner);
        let d = doc(vec![outer]);
        assert_eq!(pitches(&d, "leaf"), [(0, 60), (5, 90)]);
    }

    #[test]
    fn a_choke_stops_what_sounds_in_its_group_and_nothing_else() {
        // Kit: a note at 0 (36, 100 long) for Open, at 50 (38) for Closed, at 0 (40) for Ride,
        // at 100 (38) again, and at 50 (42) for Crash; Open, Closed and Crash choke group 1,
        // Ride group 2.
        let folder_notes = vec![
            note("evt_1", 0, 100, 36, 1.0),
            note("evt_2", 50, 30, 38, 1.0),
            note("evt_3", 0, 200, 40, 1.0),
            note("evt_4", 100, 30, 38, 1.0),
            note("evt_5", 50, 30, 42, 1.0),
        ];
        let mut kit = holding("kit", folder_notes, 400, 400);
        kit.children.push(instrument("open", zone([36, 36], 0, 1)));
        kit.children
            .push(instrument("closed", zone([38, 38], 0, 1)));
        kit.children.push(instrument("ride", zone([40, 40], 0, 2)));
        kit.children.push(instrument("crash", zone([42, 42], 0, 1)));
        let d = doc(vec![kit]);
        let got: Vec<(String, Ticks)> = chokes(&d)
            .iter()
            .map(|c| (c.track.as_str().to_owned(), c.at))
            .collect();
        // Open, sounding from 0, is choked at 50; Closed and Crash, struck together at 50, both
        // sound; the Ride (group 2) is never choked; Closed's first note ends at 80, before the
        // start at 100, so it isn't choked then.
        assert_eq!(got, [("trk_open".to_owned(), 50)]);
    }

    /// Two parts playing one pattern over each other sound its note twice: each voice is choked.
    /// Choked at one tick, the voice that started first comes first, before the track's place.
    #[test]
    fn each_voice_is_choked_and_chokes_at_one_tick_go_by_their_notes_starts() {
        // Kit: a note at 0 (36) for Open, one at 20 (37) for Low, both in Pair, a member folder in
        // choke group 1 with Snare, struck at 50 (38); Low comes before Open in the document.
        let mut kit = holding(
            "kit",
            vec![
                note("evt_1", 0, 100, 36, 1.0),
                note("evt_2", 20, 100, 37, 1.0),
                note("evt_3", 50, 10, 38, 1.0),
            ],
            200,
            200,
        );
        let mut pair = Track::new(id("trk_pair"), "pair");
        pair.zone = zone([36, 37], 0, 1);
        pair.children.push(instrument("low", zone([37, 37], 0, 0)));
        pair.children.push(instrument("open", zone([36, 36], 0, 0)));
        kit.children.push(pair);
        kit.children.push(instrument("snare", zone([38, 38], 0, 1)));
        // A second part playing the kit's pattern from 0 again: the notes twice.
        kit.parts
            .push(Part::new(id("prt_kit_2"), id("pat_kit"), 0, 200));
        let d = doc(vec![kit]);
        let got: Vec<(String, Ticks, Ticks)> = chokes(&d)
            .iter()
            .map(|c| (c.track.as_str().to_owned(), c.start, c.at))
            .collect();
        assert_eq!(
            got,
            [
                ("trk_open".to_owned(), 0, 50),
                ("trk_open".to_owned(), 0, 50),
                ("trk_low".to_owned(), 20, 50),
                ("trk_low".to_owned(), 20, 50),
            ],
            "two voices each, Open's first: it started first"
        );
    }

    #[test]
    fn a_member_folder_in_a_choke_group_is_choked_whole() {
        let mut kit = holding(
            "kit",
            vec![
                note("evt_1", 0, 100, 36, 1.0),
                note("evt_2", 40, 10, 38, 1.0),
            ],
            100,
            100,
        );
        let mut pair = Track::new(id("trk_pair"), "pair");
        pair.zone = zone([36, 36], 0, 1);
        pair.children.push(instrument("left", None));
        pair.children.push(instrument("right", None));
        kit.children.push(pair);
        kit.children.push(instrument("snare", zone([38, 38], 0, 1)));
        let d = doc(vec![kit]);
        let got: Vec<(String, Ticks)> = chokes(&d)
            .iter()
            .map(|c| (c.track.as_str().to_owned(), c.at))
            .collect();
        assert_eq!(
            got,
            [("trk_left".to_owned(), 40), ("trk_right".to_owned(), 40)]
        );
    }
}
