//! Real projects, read from a folder of them when `POLYEND_CORPUS` names one (a Tracker card's
//! `Projects/`, copied): never committed, as they are someone's songs. Run by hand:
//! `POLYEND_CORPUS=<folder> cargo test -p tutti-polyend -- --ignored`.

use tutti::folder::{Folder, Kind, MemoryStore};
use tutti::{Generator, Project};
use tutti_convert::{Error, Format, Quiet, Source, write};
use tutti_fs::DirStore;
use tutti_polyend::{PolyendTracker, model::samples, read, to_model};

fn projects(source: &dyn Source, at: &str, out: &mut Vec<String>) {
    let Ok(entries) = source.entries(at) else {
        return;
    };
    for e in entries {
        if e.kind != Kind::Folder {
            continue;
        }
        let child = if at.is_empty() {
            e.name.clone()
        } else {
            format!("{at}/{}", e.name)
        };
        if PolyendTracker
            .detect(source, &child, Kind::Folder)
            .unwrap()
            .is_some()
        {
            out.push(child);
        } else {
            projects(source, &child, out);
        }
    }
}

#[test]
#[ignore = "needs POLYEND_CORPUS, a folder of real projects"]
fn every_real_project_round_trips_and_converts() {
    let Ok(root) = std::env::var("POLYEND_CORPUS") else {
        return;
    };
    let source = DirStore::new(root);
    let mut found = Vec::new();
    projects(&source, "", &mut found);
    assert!(!found.is_empty(), "no projects under POLYEND_CORPUS");
    let (mut converted, mut refused) = (0, 0);
    for at in found {
        let f = PolyendTracker
            .detect(&source, &at, Kind::Folder)
            .unwrap()
            .unwrap();
        let (model, locators) = match read(&source, &f) {
            Ok(read) => read,
            Err(Error::Unreadable { reference, why }) => {
                println!("refused  {at}: {reference}: {why}");
                refused += 1;
                continue;
            }
            Err(e) => panic!("{at}: {e}"),
        };
        // Every file written back byte for byte.
        for (path, bytes) in model.files(&source, &locators).unwrap() {
            let original = source.read(&format!("{at}/{path}")).unwrap();
            assert!(original == bytes, "{at}/{path} written back differs");
        }
        // Converted, written, opened again and valid.
        let conversion = PolyendTracker.to_tutti(&source, &f, &mut Quiet).unwrap();
        let remarks = conversion.report.remarks.len();
        let out = MemoryStore::new();
        let generator = Generator::new("corpus", "0", "https://example.org");
        write(conversion, &source, out.clone(), &generator, &mut Quiet).unwrap();
        let opened = Project::open(out.clone()).unwrap();
        assert!(
            opened.report.is_valid(),
            "{at}: {:?}",
            opened.report.errors().collect::<Vec<_>>()
        );
        // Back to the model: equal, the samples' bytes too.
        let (back, back_locators) = to_model(opened.project.document(), &out).unwrap();
        assert!(back == model, "{at}: the model read back differs");
        for (index, locator) in &locators {
            assert_eq!(
                samples(&source, locator).unwrap(),
                samples(&out, &back_locators[index]).unwrap(),
                "{at}: instrument {index}'s samples"
            );
        }
        let notes: usize = opened
            .project
            .document()
            .tracks
            .iter()
            .flat_map(|t| std::iter::once(t).chain(t.children.iter()))
            .flat_map(|c| c.patterns.iter())
            .map(|p| p.events.len())
            .sum();
        println!("converted {at}: {notes} notes in patterns, {remarks} remarks");
        let song = tutti_polyend::song::song(&model);
        let seconds = song.end as f64 / tutti_polyend::song::PPQ as f64 * 60.0 / song.tempo[0].1;
        println!(
            "  tempo {:?}, {} slots ({:?}), about {seconds:.1} s",
            song.tempo,
            song.slots.len(),
            song.slots
                .iter()
                .map(|s| (s.pattern, s.steps))
                .collect::<Vec<_>>()
        );
        let report = PolyendTracker
            .to_tutti(&source, &f, &mut Quiet)
            .unwrap()
            .report;
        for r in report.remarks {
            println!("  - {:?}: {}", r.kind, r.text);
        }
        converted += 1;
    }
    println!("{converted} converted, {refused} refused");
    assert!(converted > 0);
}
