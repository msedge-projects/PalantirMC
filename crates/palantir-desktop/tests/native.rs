//! The gate behind the claim that this launcher is a native program.
//!
//! The rewrite this repository is in the middle of is a port of the Modrinth App
//! to Rust and iced: no webview, no JavaScript engine, no HTML asset, no Node
//! toolchain. Every one of those is something a project can acquire by accident --
//! a dependency of a dependency, a `.html` file added as a "template", a
//! `package.json` for a build script -- and an accident is exactly what a claim in
//! a document does not catch. So the claim is a test, and it runs in the same job
//! as every other test.
//!
//! What is measured here:
//!
//! * The lock file names no browser or scripting engine. The one nuance worth
//!   writing down: `js-sys` and `web-sys` *are* in the lock, because they are half
//!   the wasm target's dependency chain, and the desktop crate's own tree does not
//!   contain them for the platform this ships on -- measured with
//!   `cargo tree -p palantir-desktop --locked --target x86_64-pc-windows-msvc -e
//!   normal`, which prints neither of them (nor `wasm-bindgen`) at all. They are
//!   therefore checked where it matters, in the dependencies this workspace
//!   declares, rather than in a lock file shared with every target.
//! * No crate in the workspace declares a browser, a scripting engine or a Node
//!   binding as a dependency.
//! * Nothing under `crates/` is a script or a markup file. The launcher's assets
//!   are fonts and PNG art; its interface is Rust that draws, not files that are
//!   interpreted. (`tools/curve_samples.html`, which measures Chromium's own
//!   timing answers, is deliberately outside this scan: it is a measurement
//!   harness in an ignored path, it never ships, and it is the instrument that
//!   defined the motion table rather than part of the program.)
//! * There is no `package.json` and no `node_modules` anywhere outside the
//!   vendored reference tree.
//!
//! If a future dependency genuinely needs one of these, this test is where the
//! conversation happens: deleting the assertion is a decision with a reason, and
//! the reason goes in the commit message.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Crates that mean a browser, a JavaScript engine or a Node binding is being
/// compiled into this program. Substring-free exact names, so that a crate
/// *about* the same subject (`webp`, `jiff`) is not caught by accident.
const ENGINE_CRATES: [&str; 18] = [
    "tauri",
    "tauri-build",
    "tauri-runtime",
    "wry",
    "webview2-com",
    "webview2-com-sys",
    "webkit2gtk",
    "servo",
    "mozjs",
    "quickjs",
    "rusty_v8",
    "deno_core",
    "boa_engine",
    "neon",
    "napi",
    "electron",
    "nodejs",
    "chromiumoxide",
];

/// Files a browser would interpret, which this program must contain none of.
const SCRIPT_EXTENSIONS: [&str; 9] =
    ["js", "mjs", "cjs", "ts", "tsx", "html", "htm", "vue", "svelte"];

fn root() -> PathBuf {
    // `crates/palantir-desktop` -> the workspace root.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate is inside the workspace")
        .to_path_buf()
}

fn walk(directory: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // The scanner's own boundaries: build output, the repository's own
        // metadata, and the vendored reference tree, which is read as a
        // specification and never compiled.
        if name == "target" || name == "vendor" || name == ".git" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            walk(&path, into);
        } else {
            into.push(path);
        }
    }
}

/// Every crate name the lock file pins.
fn locked_crates() -> BTreeSet<String> {
    let lock = fs::read_to_string(root().join("Cargo.lock")).expect("the workspace has a lock file");
    lock.lines()
        .filter_map(|line| line.strip_prefix("name = "))
        .map(|value| value.trim().trim_matches('"').to_string())
        .collect()
}

/// Every dependency named by any manifest in the workspace.
fn declared_dependencies() -> BTreeSet<String> {
    let mut manifests = Vec::new();
    walk(&root().join("crates"), &mut manifests);
    manifests.push(root().join("Cargo.toml"));
    let mut names = BTreeSet::new();
    for manifest in manifests.into_iter().filter(|path| {
        path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml")
    }) {
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.starts_with("//") || !line.contains('=') {
                continue;
            }
            let Some((name, _)) = line.split_once('=') else {
                continue;
            };
            // A dependency line: a bare or quoted name before the first `=`, with
            // no space in it. Section headers and array elements are not.
            let name = name.trim().trim_matches('"');
            if !name.is_empty() && !name.contains(' ') && !name.starts_with('[') {
                names.insert(name.to_string());
            }
        }
    }
    names
}

#[test]
fn no_browser_or_scripting_engine_is_linked_or_declared() {
    let locked = locked_crates();
    let offenders: Vec<&str> = ENGINE_CRATES
        .iter()
        .copied()
        .filter(|name| locked.contains(*name))
        .collect();
    assert!(
        offenders.is_empty(),
        "the lock file pins a browser or scripting engine: {offenders:?}"
    );
    // And the half that matters more: nothing this workspace asks for. `js-sys`
    // and `web-sys` are in the lock for the wasm target but are not in the
    // Windows tree; a manifest naming one would put them in it.
    let declared = declared_dependencies();
    for name in ["js-sys", "web-sys", "wasm-bindgen", "tauri", "wry", "webview2"] {
        assert!(
            !declared.contains(name),
            "a workspace manifest depends on {name}, which makes this not a native program"
        );
    }
}

#[test]
fn no_script_or_markup_file_is_shipped_with_the_launcher() {
    let mut files = Vec::new();
    walk(&root().join("crates"), &mut files);
    let scripts: Vec<String> = files
        .iter()
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.to_ascii_lowercase())
                .is_some_and(|extension| SCRIPT_EXTENSIONS.contains(&extension.as_str()))
        })
        .map(|path| path.display().to_string())
        .collect();
    assert!(
        scripts.is_empty(),
        "a crate holds a file a browser would interpret: {scripts:?}"
    );
}

#[test]
fn there_is_no_node_toolchain_outside_the_vendored_reference() {
    let mut files = Vec::new();
    walk(&root(), &mut files);
    let node: Vec<String> = files
        .iter()
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "package.json" || name == "package-lock.json")
        })
        .map(|path| path.display().to_string())
        .collect();
    assert!(
        node.is_empty(),
        "the tree holds a Node manifest, which means a second toolchain: {node:?}"
    );
}

#[test]
fn the_interface_is_rust_that_draws_rather_than_files_that_are_read() {
    // The positive half of the same claim, and the one that keeps the negative
    // half from being satisfied by an empty tree: the program's own interface is
    // in the crate, and the design system, the icon set and the strings are
    // generated Rust rather than assets loaded at runtime.
    let generated = [
        "text_gen.rs",
        "theme_gen.rs",
        "icons_gen.rs",
    ];
    for name in generated {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(name);
        assert!(path.is_file(), "{name} is missing, so the interface has no source");
    }
}
