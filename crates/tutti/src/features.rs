//! What a project uses, and so the lowest conformance level a reader needs to play it in full
//! (`conformance.md` 3): what `tutti check` reports about a project.

use crate::model::{Document, Event};

/// A feature a project uses, with the level that first covers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Feature {
    pub name: &'static str,
    pub level: u8,
}

const fn feature(name: &'static str, level: u8) -> Feature {
    Feature { name, level }
}

/// The features a document uses, each once, in level order.
pub fn features(doc: &Document) -> Vec<Feature> {
    let mut out: Vec<Feature> = Vec::new();
    let mut add = |f: Feature| {
        if !out.contains(&f) {
            out.push(f);
        }
    };
    if doc.tempo.len() > 1 {
        add(feature("tempo changes", 0));
    }
    if doc.meter.len() > 1 {
        add(feature("meter changes", 1));
    }
    if !doc.key.is_empty() {
        add(feature("key map", 1));
    }
    if !doc.markers.is_empty() {
        add(feature("markers", 1));
    }
    if !doc.scenes.is_empty() {
        add(feature("scenes", 2));
    }
    if !doc.extensions_required.is_empty() {
        add(feature("required extensions", 2));
    }
    if !doc.master.devices.is_empty() {
        add(feature("plugins", 1));
    }
    if doc
        .files
        .iter()
        .any(|f| crate::validate::device_file(&f.path).is_some())
    {
        add(feature("plugin files", 1));
    }
    for t in doc.all_tracks() {
        if !t.children.is_empty() {
            add(feature("folders", 0));
        }
        if !t.channel.sends.is_empty() {
            add(feature("sends", 1));
        }
        if !t.devices.is_empty() {
            add(feature("plugins", 1));
        }
        if !t.automation.is_empty() {
            add(feature("automation", 1));
        }
        if !t.renders.is_empty() || t.patterns.iter().any(|p| !p.renders.is_empty()) {
            add(feature("renders", 1));
        }
        if !t.slots.is_empty() {
            add(feature("scenes", 2));
        }
        for p in &t.patterns {
            for e in &p.events {
                match e {
                    Event::Note(n) => {
                        add(feature("notes", 0));
                        if n.expressions.is_some() {
                            add(feature("expressions", 1));
                        }
                        if let Some(a) = &n.audio {
                            add(feature("audio", 0));
                            if a.map.len() > 1 {
                                add(feature("warps", 1));
                            }
                        }
                    }
                    Event::Chord(_) => add(feature("chords", 0)),
                    Event::Param(_) => add(feature("parameter events", 1)),
                    Event::Unknown(_) => add(feature("unknown event types", 0)),
                }
            }
        }
    }
    out.sort();
    out
}

/// The lowest level a reader needs to play the document in full.
pub fn level_needed(doc: &Document) -> u8 {
    features(doc).iter().map(|f| f.level).max().unwrap_or(0)
}
