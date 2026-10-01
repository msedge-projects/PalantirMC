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

/// The tab body a windowed listing is measured in, in pixels.
///
/// A 700px window with the bar, the instance header and the tab strip above the
/// body leaves about this much, which is the number the page is handed by its own
/// scroll region (`Scrollable::on_scroll`) and the number the window
/// ([`crate::scroll::window`]) is computed from.
const VIEW: f32 = 600.0;

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
            // A URL per hit, shaped like the API's, because a card's icon slot is a
            // box whether a picture arrived for it or not: the cost this file is
            // about is the box, and the fetch is the store's.
            icon_url: format!("https://cdn.modrinth.com/{index:04}.png"),
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

/// The scroll region's report for a body of [`VIEW`] pixels, scrolled to
/// `offset`.
///
/// A plain value rather than a real `Viewport`, because a real one is what a
/// window that has been scrolled hands the page -- and these tests run without
/// one. The page is handed the same thing either way
/// ([`crate::scroll::Geometry::of`] is the only conversion).
fn scroll_geometry(offset: f32) -> crate::scroll::Geometry {
    crate::scroll::Geometry { offset, view_height: VIEW }
}

#[test]
fn the_instance_pages_tabs_cost_this_much_a_frame() {
    println!();
    println!("== instance page, per frame ==");
    let theme = Gen::ALL[0];
    let mut worst = 0.0_f64;
    // The frame at a hundred entries, which every larger count has to match: the
    // window is the same size there as here.
    let mut base = (0.0_f64, 0.0_f64);
    // And the worst of the two fallback frames, which are compared with a ceiling
    // rather than with the hundred-row frame: their window is a fixed height of
    // rows, and at this row height that is more rows than a hundred rows are.
    let mut opened = 0.0_f64;
    for n in FILE_COUNTS {
        let (store, id) = instance(&format!("mods-{n}"), n);
        let directory = store.instance_dir(&id);
        // The read the page makes when it draws, on its own: this is the part
        // that is I/O rather than widget building.
        let read = median_ms(5, || {
            std::hint::black_box(store::content(&directory));
        });
        row("mods/ read (store::content)", n, read);

        // Where the Content tab's cost sits now, in the two places it is paid.
        // The read and the row names are both charged when the tab is *entered*:
        // `store::listing` is the load, and `ui::scoped` -- a process-wide table
        // behind a mutex, formatting a name per row -- is called as the answer
        // arrives rather than on every frame the rows are drawn. The frame pays
        // for the rows alone, which is the row below the two above it.
        let read = median_ms(5, || {
            // `let _ =` because the answer is a `Result` and this is the one place
            // it is deliberately dropped: the row is about the time the read takes,
            // and the answer itself is put through the page below.
            let _ = std::hint::black_box(store::listing(&directory, &InstanceTab::Content));
        });
        row("Content: the read (once a tab)", n, read);

        let mods = store::content(&directory);
        let intern = median_ms(5, || {
            for entry in &mods {
                std::hint::black_box(crate::ui::scoped(
                    "instance:content:toggle",
                    &entry.file_name,
                ));
            }
        });
        row("Content: the row names (once a tab)", n, intern);

        // Drawn as the shell draws it: the listing was read when the tab was
        // entered, and the page's body is handed nothing but the state. What these
        // rows measure is the frame's whole cost, which is the number the budget
        // is about -- in the two states a frame is drawn in. *Reported* is a tab
        // the reader has scrolled, whose region has said how tall it is and where
        // it is; *no report yet* is a tab that has just been opened, where the page
        // draws the window a window-sized guess gives
        // ([`crate::scroll::INITIAL_VIEW`]) because a region nobody has touched has
        // never published a viewport.
        let mut state = crate::pages::instance::State::new(id.clone(), InstanceTab::Content);
        let _ = state.update(
            crate::pages::instance::Message::Listed {
                round: 0,
                listing: store::listing(&directory, &InstanceTab::Content),
            },
            &store,
        );
        let content_open = median_ms(if n >= 1_000 { 5 } else { 20 }, || {
            std::hint::black_box(crate::pages::instance::view(theme, &state, &store));
        });
        row("Content view (no report yet)", n, content_open);

        let mut scrolled = state.clone();
        let _ = scrolled.update(
            crate::pages::instance::Message::Scrolled(scroll_geometry(0.0)),
            &store,
        );
        let content = median_ms(if n >= 1_000 { 5 } else { 20 }, || {
            std::hint::black_box(crate::pages::instance::view(theme, &scrolled, &store));
        });
        row("Content view (region reported)", n, content);

        let mut files_state = crate::pages::instance::State::new(id.clone(), InstanceTab::Files);
        let _ = files_state.update(
            crate::pages::instance::Message::Listed {
                round: 0,
                listing: store::listing(&directory, &InstanceTab::Files),
            },
            &store,
        );
        let files_open = median_ms(if n >= 1_000 { 5 } else { 20 }, || {
            std::hint::black_box(crate::pages::instance::view(theme, &files_state, &store));
        });
        row("Files view (no report yet)", n, files_open);

        // The Files tab's own listing is the root's `n` config files plus the
        // `mods` directory above them, which is the one entry the count does not
        // account for and the reason this row is `n` and not `n + 1`.
        let mut files_scrolled = files_state.clone();
        let _ = files_scrolled.update(
            crate::pages::instance::Message::Scrolled(scroll_geometry(0.0)),
            &store,
        );
        let files = median_ms(if n >= 1_000 { 5 } else { 20 }, || {
            std::hint::black_box(crate::pages::instance::view(theme, &files_scrolled, &store));
        });
        row("Files view (region reported)", n, files);

        // What each window holds at each size, which is the property rather than
        // the cost. Reported, it is one number for every size the test names;
        // with no report yet it is the fallback window, bounded by a height
        // ([`crate::scroll::INITIAL_VIEW`]) rather than by the listing -- and the
        // Files tab is where that shows, because its rows are the short ones.
        let drawn = crate::scroll::window(n, crate::pages::instance::CONTENT_ROW, scroll_geometry(0.0));
        println!(
            "{:<34} n={:<6} {:>9} rows of {n}",
            "  of which: rows drawn, reported",
            n,
            drawn.len()
        );
        let open = crate::scroll::window(
            n,
            crate::pages::instance::CONTENT_ROW,
            crate::scroll::Geometry::default(),
        );
        let open_files = crate::scroll::window(
            n,
            crate::pages::instance::PLAIN_ROW,
            crate::scroll::Geometry::default(),
        );
        println!(
            "{:<34} n={:<6} {:>4} / {:>4} rows of {n}",
            "  of which: rows drawn, no report",
            n,
            open.len(),
            open_files.len()
        );

        // The read this tab pays once when it is entered, and the rows it draws --
        // measured apart because they are where a fix has to choose between. The
        // tab that was a second a frame at five thousand entries was paying
        // almost all of it here, in the first row.
        let entries = store::files(&directory);
        let read = median_ms(5, || {
            let _ = std::hint::black_box(store::listing(&directory, &InstanceTab::Files));
        });
        row("Files: the read (once a tab)", n, read);
        // The contrast row: every row built, which is what the tab did before
        // there was a window, and the cost G116 left behind. It is not part of
        // `worst` -- it is a measurement of the alternative, not of the page.
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
        row("  of which: every row built (no window)", n, built);

        match n {
            0 | 100 => {
                base = (content, files);
            }
            _ => {
                // The property this slice is about, asserted rather than
                // described: a hundred rows and five thousand draw the same frame,
                // because they draw the same window. The margins are for the
                // shared runner -- the measurement is flat to within a tenth of a
                // millisecond -- and a page that had gone back to drawing the whole
                // listing would be 60 times past them at this size.
                assert!(
                    content < base.0 * 3.0 + 0.5,
                    "{n} mods drew {content:.3} ms against {:.3} ms at 100",
                    base.0
                );
                assert!(
                    files < base.1 * 3.0 + 0.5,
                    "{n} entries drew {files:.3} ms against {:.3} ms at 100",
                    base.1
                );
            }
        }

        worst = worst.max(content).max(files).max(content_open).max(files_open);
        opened = opened.max(content_open).max(files_open);
    }
    println!();
    // The tab with no report yet is the only frame whose cost is not flat, and it
    // is still bounded: the fallback draws a window's worth of rows whatever the
    // listing holds -- 177 rows for the Files tab's 24px ones, five times what a
    // 600px body needs -- so 4 ms is a ceiling on a frame that was 12.0 ms when the
    // tab drew its whole listing and is 0.18 ms once the reader scrolls. A page
    // that had stopped windowing the reported case would be a hundred times past
    // this at the size below.
    assert!(
        opened < 4.0,
        "a tab nobody has scrolled draws a bounded window; worst was {opened:.3} ms"
    );
    // What this guards is the *window*: a frame at 60Hz is 16.7 ms, the frame at
    // 5,000 mods was 32.0 ms when every row was built, and it is now the same frame
    // as at a hundred. 8 ms is a long way above the measurement -- a shared runner
    // is the reason -- and a page that had stopped windowing would be four times
    // past it at the size this test names.
    assert!(
        worst < 8.0,
        "a windowed tab draws the window and not the listing; worst was {worst:.3} ms"
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
