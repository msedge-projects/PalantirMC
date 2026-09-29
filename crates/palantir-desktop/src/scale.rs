//! What the interface costs at size, measured rather than guessed.
//!
//! Nothing in this tree knew what a page costs when its list is long. The plan
//! asked for three numbers -- an instance with thousands of files in its `mods/`,
//! a Discover page with a hundred hits, and the interaction clock with every
//! control on a page in flight -- and this module is where they come from. It is
//! `#[cfg(test)]` like [`crate::reference_tokens`]: it exists to be run and
//! recorded, not to ship.
//!
//! **Why the numbers matter here and not somewhere else.** `pages/instance.rs`
//! says of itself that its tab bodies read the filesystem when they are *drawn*,
//! "which is fine for the tens of files an instance has and would not be for" a
//! folder of thousands. That is a claim, and a claim about cost is the one kind
//! this repository can check: a mods folder is a `read_dir` plus one card per
//! entry, rebuilt from scratch every frame, so the per-frame cost of the page is a
//! function of the file count and grows with it. The same shape is behind the
//! clock: every control on a page takes the clock's lock and does a hash lookup
//! *per frame* (see [`crate::ui::interaction`]), which is free for a card's worth
//! of controls and is not obviously free for a thousand.
//!
//! **How the measurement is taken.** Real directories, real views, real clock.
//! Nothing here is a model of the cost: the fixture is `N` empty files under a
//! scratch instance's `mods/`, the view is the page's own [`crate::pages::instance::view`],
//! and the number reported is the **median** of a small number of repetitions
//! rather than the mean or the best, because the machine this runs on is shared
//! with another agent's compiles and a mean would carry their spike.
//!
//! The tests print a table (`cargo test -p palantir-desktop --locked scale --
//! --nocapture`) and *assert an envelope*, so a regression that makes a frame
//! quadratic fails here rather than being read about in a gate. The envelope is
//! set from the measurement with room for a slow runner; the numbers in
//! `GATES.md` are the measurement itself.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::widget::column;
use iced::Length;

use crate::icons_gen::Glyph;
use crate::page::Load;
use crate::pages::discover::{self, Hit};
use crate::route::{InstanceTab, ProjectType};
use crate::store::{self, Store};
use crate::theme_gen::Theme as Gen;

/// The file counts an instance is measured at.
///
/// 0 is the arm that has to stay free -- an instance with nothing in `mods/` is
/// the common case and must not pay for the rare one -- and 5,000 is the size the
/// plan named. 100 and 1,000 are the two points between them, because a cost that
/// is linear looks the same at 100 and 10,000 and a cost that is quadratic does
/// not.
const FILE_COUNTS: [usize; 4] = [0, 100, 1_000, 5_000];

/// Discover's two: the reference asks for 20 results a page and draws a hundred
/// cards without complaint, and 1,000 is where a linear cost becomes visible.
const HIT_COUNTS: [usize; 3] = [20, 100, 1_000];

/// The clock's counts: a page's controls, then ten pages of them.
const CONTROL_COUNTS: [usize; 4] = [0, 100, 1_000, 5_000];

/// A scratch root, emptied first so a rerun does not double-count files.
fn root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join("palantirmc-scale").join(name);
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("a scratch root");
    path
}

/// An instance holding `files` jars in its `mods/` and `files` loose files in
/// its root, and the store that reads it.
///
/// A store is loaded rather than hand-built so the instance is a real one: the
/// page reads through `store.instance_dir`, so a fixture that skipped the scan
/// would measure a page reading a directory nothing else knows about.
///
/// **Two placements, because the two tabs read two directories.** The Content tab
/// lists `mods/`; the Files tab lists the instance root. The first version of this
/// fixture put everything in `mods/` and then measured the Files tab at five
/// thousand -- which read a root holding exactly one directory, and reported a
/// flat 0.68 ms that said nothing about the tab. Both placements are written here
/// rather than one, so `n` means the same thing on every row.
fn instance(name: &str, files: usize) -> (Store, String) {
    let store = Store::load(&palantir_core::paths::PalantirPaths::at(root(name)));
    let id = "measured".to_string();
    let instance_dir = store.instance_dir(&id);
    let mods = instance_dir.join("mods");
    std::fs::create_dir_all(&mods).expect("a mods directory");
    for index in 0..files {
        // A name shaped like the ones `mods::split_mod_name` sees: a version and
        // a jar, and every hundredth one disabled, which is the state the toggle
        // writes into the file name rather than into the file.
        let enabled = !index.to_string().ends_with('0');
        let suffix = if enabled { "jar" } else { "jar.disabled" };
        std::fs::write(mods.join(format!("mod-{index:05}.1.0.{suffix}")), b"")
            .expect("a fixture mod");
        // And the same count in front of it, for the Files tab: a root with a few
        // hundred config files and folders in it is what an installed pack looks
        // like on disk.
        std::fs::write(instance_dir.join(format!("config-{index:05}.toml")), b"")
            .expect("a fixture file");
    }
    (store, id)
}

/// `count` hits, shaped like the API's: the strings a card draws.
fn hits(count: usize) -> Vec<Hit> {
    (0..count)
        .map(|index| Hit {
            id: format!("AABB{index:04}"),
            title: format!("Project {index}"),
            author: "Author".to_string(),
            summary: "A summary sentence of the length a result card carries.".to_string(),
            downloads: 4_200_000 + index as u64,
            follows: 12_000,
            game_versions: vec!["1.21.1".to_string(), "1.20.6".to_string()],
            loaders: vec!["fabric".to_string()],
        })
        .collect()
}

/// The median of `reps` runs of `run`, in milliseconds.
///
/// Median rather than mean because this machine is shared: a mean would carry the
/// other agent's compile spike into a number that is supposed to describe the
/// interface, and the plan's own gates record medians for the same reason.
fn median_ms(reps: usize, mut run: impl FnMut()) -> f64 {
    let mut samples: Vec<f64> = Vec::with_capacity(reps);
    for _ in 0..reps {
        let start = Instant::now();
        run();
        samples.push(start.elapsed().as_secs_f64() * 1_000.0);
    }
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

/// A row of the table these tests print: `surface` at size `n`, `ms` per call.
fn row(surface: &str, n: usize, ms: f64) {
    println!("{surface:<34} n={n:<6} {ms:>9.3} ms");
}

#[test]
fn the_instance_pages_tabs_cost_this_much_a_frame() {
    println!();
    println!("== instance page, per frame ==");
    let theme = Gen::ALL[0];
    let mut worst = 0.0_f64;
    for n in FILE_COUNTS {
        let (store, id) = instance(&format!("mods-{n}"), n);
        let directory = store.instance_dir(&id);
        // The read the page makes when it draws, on its own: this is the part
        // that is I/O rather than widget building.
        let read = median_ms(5, || {
            std::hint::black_box(store::content(&directory));
        });
        row("mods/ read (store::content)", n, read);

        // What the Content tab spends its time on, in the two pieces the view is
        // made of. `ui::scoped` interns a control's name in a process-wide table
        // behind a mutex, and the Content tab names every row's toggle from the
        // file it acts on -- so this is one lock, one `format!` and one hash
        // lookup *per row per frame*, which is the part a fix would remove.
        let mods = store::content(&directory);
        let intern = median_ms(5, || {
            for entry in &mods {
                std::hint::black_box(crate::ui::scoped("instance:content:toggle", &entry.file_name));
            }
        });
        row("  of which: row key interning", n, intern);

        let state = crate::pages::instance::State::new(id.clone(), InstanceTab::Content);
        let content = median_ms(if n >= 1_000 { 5 } else { 20 }, || {
            std::hint::black_box(crate::pages::instance::view(theme, &state, &store));
        });
        row("Content tab view", n, content);

        let files_state = crate::pages::instance::State::new(id.clone(), InstanceTab::Files);
        // The Files tab's own listing is the root's `n` config files plus the
        // `mods` directory above them, which is the one entry the count does not
        // account for and the reason this row is `n` and not `n + 1`.
        let files = median_ms(if n >= 1_000 { 5 } else { 20 }, || {
            std::hint::black_box(crate::pages::instance::view(theme, &files_state, &store));
        });
        row("Files tab view", n, files);

        // The Files tab at five thousand entries is a second a frame, and the
        // Content tab at the same count is a tenth of that. The two differ in one
        // thing: Content builds a card *per row* and Files builds one card around a
        // column of every row. These two rows measure the halves apart, so the
        // finding names the culprit instead of the tab.
        let entries = store::files(&directory);
        let read = median_ms(5, || {
            std::hint::black_box(store::files(&directory));
        });
        row("  of which: files/ read", n, read);
        let built = median_ms(if n >= 1_000 { 5 } else { 20 }, || {
            // The message type is named because nothing in this closure tells the
            // compiler which page's `Element` these rows are for -- the real view
            // gets it from its own signature.
            let mut list: iced::widget::Column<'_, crate::pages::instance::Message> =
                column![].spacing(4.0).width(Length::Fill);
            for entry in &entries {
                list = list.push(crate::ui::icon_label(theme, Glyph::File, &entry.name));
            }
            std::hint::black_box(crate::ui::card(theme, list));
        });
        row("  of which: N rows in one column", n, built);

        worst = worst.max(content).max(files);
    }
    println!();
    // A frame at 60Hz is 16.7 ms and the measured worst case above is 110 ms at
    // 5,000 mods -- this page does not fit in a frame at that size, and the gate
    // says so with the number rather than a ceiling that hides it. What this
    // assertion guards is the *regression*: 250 ms is more than twice the
    // measurement, because the runner is shared and this test runs beside 533
    // others, and a page that had gone quadratic (a scan inside the row loop, say)
    // would be an order of magnitude past it rather than a few percent.
    assert!(
        worst < 250.0,
        "the instance page's tabs are linear in the entry count; worst was {worst:.3} ms"
    );
}

#[test]
fn discover_costs_this_much_a_frame() {
    println!();
    println!("== discover, per frame ==");
    let theme = Gen::ALL[0];
    let store = Store::load(&palantir_core::paths::PalantirPaths::at(root("discover")));
    let mut worst = 0.0_f64;
    for n in HIT_COUNTS {
        let mut state = discover::State::new(ProjectType::Mod);
        state.results = Load::Ready(hits(n));
        let ms = median_ms(if n >= 1_000 { 5 } else { 20 }, || {
            std::hint::black_box(discover::view(theme, &state, &store));
        });
        row("results view", n, ms);
        worst = worst.max(ms);
    }
    println!();
    // 48.2 ms measured at 1,000 hits, which the API never returns in one page:
    // it asks for 20 at a time and the reference draws a hundred. The guard is for
    // a regression, and sits at four times the measurement for the runner's sake.
    assert!(
        worst < 200.0,
        "a hundred result cards is the documented case; worst was {worst:.3} ms"
    );
}

#[test]
fn the_interaction_clock_costs_this_much_a_frame() {
    println!();
    println!("== interaction clock, per frame ==");
    let mut worst = 0.0_f64;
    for n in CONTROL_COUNTS {
        // One key per control, leaked: `Interactions` is keyed by `&'static str`,
        // which is what lets the stylesheets hold a name across a rebuild.
        let keys: Vec<&'static str> = (0..n)
            .map(|index| {
                Box::leak(format!("scale:control:{index}").into_boxed_str()) as &'static str
            })
            .collect();
        let mut clock = crate::anim::Interactions::default();
        let start = Instant::now();
        for key in &keys {
            clock.set(key, true, false, start);
        }

        // A frame with every one of them in flight: the tick, then the read each
        // control makes in its own style.
        let tick = median_ms(20, || {
            clock.tick(start + Duration::from_millis(75));
        });
        row("clock tick, all in flight", n, tick);

        let read = median_ms(20, || {
            for key in &keys {
                std::hint::black_box(clock.drawn(key));
            }
        });
        row("N reads (one per control)", n, read);

        // The clock the shell actually reaches, which is behind a mutex.
        let locked = median_ms(20, || {
            for key in &keys {
                std::hint::black_box(crate::ui::interaction(key));
            }
        });
        row("N locked reads (ui::interaction)", n, locked);

        worst = worst.max(tick).max(read).max(locked);
    }
    println!();
    // 5.4 ms measured at 5,000 controls with every one of them in flight, and
    // 0.4 ms for the tick itself: the clock is not where this interface's cost
    // is. Ten times the measurement is the guard.
    assert!(
        worst < 50.0,
        "the clock is read once per control per frame; worst was {worst:.3} ms"
    );
}
