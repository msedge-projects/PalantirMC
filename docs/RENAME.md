# Rename inventory: PalantirMC → Sift Launcher

Every place the old name appears, grouped by surface, with the file, the line, the
exact text today and the exact text after `tools/rename_sift.py`. The script is
the executable half of this file; where the two disagree, the script is wrong and
this file is the spec.

**How the counts were taken.** `Select-String -CaseSensitive:$false` over the
working tree with `.git/`, `vendor/`, `target/`, `dist/`, `.scratch/` excluded
(`vendor/` is 1,889 files of somebody else's tree — see §11), plus
`python tools/rename_sift.py --verbose`, which reports every substitution it
would make per file. "Occurrences" counts every match, so a line carrying two of
them (a `palantir-core = { path = "../palantir-core" }` entry, say) counts twice
here and once in a `Select-String` line count.

**Total, as the script measures it:** 378 files scanned, **123 files changed**,
**1,897 occurrences replaced**, 90 files with no old name, 165 non-UTF-8 files
(binary assets), 5 files excluded by name.

**This is a snapshot.** It was measured on 2026-10-25 at about 19:25, while
another session was editing the same tree (`brand.rs`, `build.rs`, `README.md`
and a new `tools/gen_icons_egui.py` all changed between two runs an hour apart).
Line numbers drift first; the substitutions do not, because the script matches
*text*, not positions. Treat a line number here as a hint and
`python tools/rename_sift.py --verbose` as the authority for the tree you are
actually holding.

---

## 0. Counts per surface

Rows overlap where one occurrence serves two surfaces — a `[dependencies]` line is
both a manifest line and a crate path. The per-group totals underneath are
disjoint and are what the script reports. "Rows listed" is how many substitution
rows this document's own tables carry for that surface.

| § | Surface | Files | Rows listed | Occurrences | Replaced | Kept on purpose |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | Cargo manifests: package names, members, path deps | 6 | 28 | 34 | 33 | 1 (`authors`) |
| 2 | Rust crate paths `palantir_core::` / `palantir_core` | 43 | 487 | 487 | 487 | 0 |
| 3 | Crate names in prose, doc links, paths, `-p` flags | — | — | 860 | 860 | 0 |
| 4 | Binary target and Windows file properties | 6 | 15 | 15 | 13 | 2 (`COPYRIGHT`, `CompanyName`) |
| 5 | Product name, window title, single instance | 7 | 20 | 20 | 20 | 0 |
| 6 | Data directory, prefs file, lock file, temp dirs | 23 | 34 | 53 | 53 | 0 |
| 7 | Environment variables and their read sites | 11 | 5 | 27 | 16 | 11 (history docs) |
| 8 | CI workflows, artifacts, `-p` flags, DLL gate | 4 | 26 | 30 | 30 | 0 |
| 9 | Installer | 1 | 6 | 6 | 5 | 1 (`PUBLISHER`) |
| 10 | `Palantir Studios` and other non-product uses | 6 | 9 | 9 | 0 | 9 |

Row 3 is the remainder of group `1-crates` after §2's 487 (`1,347 − 487 = 860`):
the hyphen spelling in Rust comments, documents, tool defaults, CI flags and the
installer. It overlaps §1, §8 and §9, which list some of those same lines again
because a manifest or a workflow is easier to review as its own table. The
sections below cover the code, manifest, CI and installer surfaces; the rest of
the 1,891 is the recorded history discussed in §11.

Script groups, disjoint:

| Group | What it is | Occurrences |
| --- | --- | ---: |
| `1-crates` | the eleven crate-name pairs | 1,347 |
| `2-identifier-forms` | `.exe`, User-Agent, multipart boundary, bin targets, `PalantirPaths` | 266 |
| `3-product-name` | `PalantirMC` → `Sift Launcher` / `SiftLauncher` | 160 |
| `4-paths-and-env` | `PALANTIRMC_`, `palantirmc`, `palantir` | 124 |
| | **total** | **1,897** |

The five crate directories are **not** renamed by the script (§13).

---

## 1. Crate package names, workspace members, path dependencies

Five crates: `palantir-core` → `sift-core`, `palantir-net` → `sift-net`,
`palantir-loader` → `sift-loader`, `palantir-desktop` → `sift-desktop`,
`palantir-egui` → **`sift-ui`**.

There are **no** `[workspace.dependencies]` entries for these crates: the
workspace shares `serde`, `serde_json`, `thiserror` and `uuid` only, and the five
are path dependencies declared in each member's own `[dependencies]`. Every such
entry has *two* occurrences on one line — the key and the `path` — and both
change.

| File | Line | Exact text today | Exact replacement |
| --- | ---: | --- | --- |
| `Cargo.toml` | 4 | `    "crates/palantir-core",` | `    "crates/sift-core",` |
| `Cargo.toml` | 5 | `    "crates/palantir-net",` | `    "crates/sift-net",` |
| `Cargo.toml` | 6 | `    "crates/palantir-loader",` | `    "crates/sift-loader",` |
| `Cargo.toml` | 7 | `    "crates/palantir-desktop",` | `    "crates/sift-desktop",` |
| `Cargo.toml` | 8 | ``    # `crates/palantir-egui` is **not** a member, and cannot be: …`` | ``    # `crates/sift-ui` is **not** a member, and cannot be: …`` |
| `Cargo.toml` | 47 | ``# rasteriser (see `crates/palantir-desktop/src/gpu.rs`) the frame cost is the`` | ``# rasteriser (see `crates/sift-desktop/src/gpu.rs`) the frame cost is the`` |
| `crates/palantir-core/Cargo.toml` | 2 | `name = "palantir-core"` | `name = "sift-core"` |
| `crates/palantir-core/Cargo.toml` | 3 | `description = "PalantirMC's launcher core: config, instance management, version resolution"` | `description = "Sift Launcher's launcher core: …"` — mechanical; **rewrite by hand** to `"Sift Launcher core: config, instance management, version resolution"` |
| `crates/palantir-net/Cargo.toml` | 2 | `name = "palantir-net"` | `name = "sift-net"` |
| `crates/palantir-net/Cargo.toml` | 3 | `description = "PalantirMC networking: auth, downloads, API clients (phase 2)"` | `description = "Sift Launcher networking: auth, downloads, API clients (phase 2)"` |
| `crates/palantir-net/Cargo.toml` | 10 | `palantir-core = { path = "../palantir-core" }` | `sift-core = { path = "../sift-core" }` |
| `crates/palantir-net/Cargo.toml` | 27 | ``# a zip. Same version `palantir-loader` already uses, so it adds no package to`` | ``# a zip. Same version `sift-loader` already uses, so it adds no package to`` |
| `crates/palantir-loader/Cargo.toml` | 2 | `name = "palantir-loader"` | `name = "sift-loader"` |
| `crates/palantir-loader/Cargo.toml` | 10 | `palantir-core = { path = "../palantir-core" }` | `sift-core = { path = "../sift-core" }` |
| `crates/palantir-desktop/Cargo.toml` | 2 | `name = "palantir-desktop"` | `name = "sift-desktop"` |
| `crates/palantir-desktop/Cargo.toml` | 3 | `description = "PalantirMC — Minecraft launcher (iced)"` | `description = "Sift Launcher — Minecraft launcher (iced)"` |
| `crates/palantir-desktop/Cargo.toml` | 9 | ``# The package is `palantir-desktop` -- the workspace member, the `-p` selector`` | ``# The package is `sift-desktop` -- the workspace member, the `-p` selector`` |
| `crates/palantir-desktop/Cargo.toml` | 10 | ``# and every `palantir_desktop::` path depend on it -- while the *executable* is`` | ``# and every `sift_desktop::` path depend on it -- while the *executable* is`` |
| `crates/palantir-desktop/Cargo.toml` | 12 | ``# --release` and CI emit `PalantirMC.exe` directly, instead of every packaging`` | ``# --release` and CI emit `SiftLauncher.exe` directly, instead of every packaging`` |
| `crates/palantir-desktop/Cargo.toml` | 13 | ``# step having to copy `palantir-desktop.exe` to a new name.`` | ``# step having to copy `sift-desktop.exe` to a new name.`` |
| `crates/palantir-desktop/Cargo.toml` | 41 | `palantir-core = { path = "../palantir-core" }` | `sift-core = { path = "../sift-core" }` |
| `crates/palantir-desktop/Cargo.toml` | 42 | `palantir-net = { path = "../palantir-net" }` | `sift-net = { path = "../sift-net" }` |
| `crates/palantir-desktop/Cargo.toml` | 43 | `palantir-loader = { path = "../palantir-loader" }` | `sift-loader = { path = "../sift-loader" }` |
| `crates/palantir-desktop/Cargo.toml` | 48 | ``# was `browse.rs`'s pack installer, and that goes over `palantir-net`'s engine`` | ``# was `browse.rs`'s pack installer, and that goes over `sift-net`'s engine`` |
| `crates/palantir-desktop/Cargo.toml` | 73 | ``# writes, which needs a zip *writer*. Same version `palantir-loader` already pulls`` | ``# writes, which needs a zip *writer*. Same version `sift-loader` already pulls`` |
| `crates/palantir-egui/Cargo.toml` | 12 | `name = "palantir-egui"` | `name = "sift-ui"` |
| `crates/palantir-egui/Cargo.toml` | 27 | `name = "PalantirEgui"` | `name = "SiftUi"` |
| `Cargo.toml` | 25 | `authors = ["Palantir Studios"]` | **unchanged** — the publisher, not the product (§10) |

`Cargo.lock` also carries 9 `name = "palantir-…"` / `"PalantirMC"` entries. It is
not edited by hand: rename the manifests and let `cargo` rewrite it.

---

## 2. Rust crate paths (`palantir_core::`, `use palantir_net::…`)

Uniform substring replacements, 487 occurrences in 43 files:

`palantir_core` → `sift_core` · `palantir_net` → `sift_net` ·
`palantir_loader` → `sift_loader` · `palantir_desktop` → `sift_desktop` ·
`palantir_egui` → **`sift_ui`**.

Lines per file (`Select-String -Pattern 'palantir_'`), for checking the diff:

```
crates/palantir-core/tests/compat.rs                 7,8,9,10,11,12,13,14,100,108,310,313,335
crates/palantir-desktop/build.rs                     30
crates/palantir-desktop/src/accounts.rs              416,601
crates/palantir-desktop/src/brand.rs                 37,40,332
crates/palantir-desktop/src/catalog.rs               32,33,239,241
crates/palantir-desktop/src/install.rs               11,25,46,47,48,49,781,1037,1125,1129,1131,1160,
                                                     1169,1217,1218,1249,1261,1268,1361,1487,1515,
                                                     1518,1543,1573,1576,1577,1578,1580,1581,1583,
                                                     1606,1607,1876,1882,1888,1935,1953,2032,2110,
                                                     2686,2735,2779,2812,2830,2901,3168
crates/palantir-desktop/src/instances.rs             16,17,18,19,552,555,558,698
crates/palantir-desktop/src/java_runtime.rs          11,27,28,35,194,348,350
crates/palantir-desktop/src/launch.rs                16,36,52,53,202,203,399,423,478,504,1200,1251,
                                                     1290,1306,1407,1521,1614,2361,2362,2836,2865,
                                                     2879,2884,2889,2894,2899,2940,2951
crates/palantir-desktop/src/loader_install.rs        22,23,24,205,206,207,392
crates/palantir-desktop/src/main.rs                  113,387,427
crates/palantir-desktop/src/meta.rs                  11,39,40,41,42,43,44,45,48,228,229,230,231,232,420
crates/palantir-desktop/src/model.rs                 7,15,33,35,36,199,208
crates/palantir-desktop/src/prefs.rs                 28,35,39,44,49,51,92,268,417,462,533,605,619,
                                                     948,958,973,978,1017,1019
crates/palantir-desktop/src/saved_skins.rs           53,207
crates/palantir-desktop/src/scale.rs                 93,392
crates/palantir-desktop/src/shell.rs                 82,83,93,970,2115,3781,4304,4313,4352,4378,
                                                     10310,10311,11760,11765,12102,12139,12489,
                                                     12507,13065
crates/palantir-desktop/src/single.rs                89
crates/palantir-desktop/src/skin.rs                  36,262
crates/palantir-desktop/src/store.rs                 43,44,45,46,47,48,52,55,218,817,977,1015,1166,
                                                     1174,1175,1180,1295,1937,2000,2349,2350,2351,
                                                     2354,2939,2990,3209,3249,3352,3374,3392,3393,
                                                     3398,3412,3413,3431,3432,3460,3493,3494,3517,4004
crates/palantir-desktop/src/update.rs                149,163,317,333,351,364
crates/palantir-desktop/src/wire.rs                  27,249,349,354
crates/palantir-desktop/src/pages/discover.rs        54,55,56,117,184,230,505,685,1872,2021,2349,2466,
                                                     3661,3662,3667,3695,3777,3779,3784,3789,3795,
                                                     3870,3871,3876,3895,3897,3902,3907,3913,4023,
                                                     4025,4029,4033,4038,4043,4773,4774,4779,4919,
                                                     4921,4926,4932,4964,4965,5123,5125,5129,5133,
                                                     5347,5402,5403,5406,5413,5418
crates/palantir-desktop/src/pages/home.rs            1369
crates/palantir-desktop/src/pages/instance.rs        539,951
crates/palantir-desktop/src/pages/mod.rs             309,702,1083
crates/palantir-desktop/src/pages/project.rs         33,755,760
crates/palantir-desktop/src/pages/screenshots.rs     261
crates/palantir-desktop/src/pages/skins.rs           38,1201,1939,2269,2270,2271,2276,2283,2292,2348,
                                                     2359,2595,2715,2908,2914,2929,2967,2968,2974,
                                                     3164,3197,3218,3249,3250,3543
crates/palantir-desktop/src/pages/user.rs            74,75,77,78
crates/palantir-loader/src/install.rs                5,21,57,58,69,92,93,95,97,108,109,129,142,153,166,177
crates/palantir-loader/src/lib.rs                    7,8,10
crates/palantir-loader/src/modpack.rs                68,69,266,273,275,311,342,633,695,726,924,950,986
crates/palantir-net/src/auth.rs                      136,140,141,153,154,214,227,238,239,1828
crates/palantir-net/src/java.rs                      30
crates/palantir-net/src/lib.rs                       8,9,33,95,250,256,258,259,260,262,264,272,280,351,
                                                     353,362,364,371,373,380,383,393,396,406,407,409
crates/palantir-net/src/meta.rs                      3,5,12,20,21,22,216,291,297,307,344,348,360,363,
                                                     367,370,380,388,401,404,406,413,652,718
crates/palantir-net/src/engine/cache.rs              413,414
crates/palantir-net/src/engine/content.rs            468,469
crates/palantir-net/src/engine/forge.rs              72,599,601,614,1002,1012,1771
crates/palantir-net/src/engine/loaders.rs            38,266,268,283
crates/palantir-net/src/engine/piston.rs             44,270,297,299,300,321,324,342,351
crates/palantir-net/tests/live.rs                    23,24,25,26,27,53,110,363,433,495,599,617,692,700,
                                                     814,815,820,888,984,1129,1130,1141,1213,1280,
                                                     1330,1394,1424,1429,1481,1565,1585,1667,1670,
                                                     1689,1757,1860,1954
```

---

## 3. Crate names outside Rust

Same five pairs, in prose, comment paths, `-p` flags, working directories and
tool defaults. Mechanical; no exceptions. Files and counts:

| File | Occurrences | Note |
| --- | ---: | --- |
| `AGENTS.md` | 5 | `-p palantir-desktop`, `-p palantir-net`, `palantir-desktop.exe` |
| `docs/LIGHTNESS.md` | 3 | `crates/palantir-egui` (×3) |
| `docs/PARITY-LOG.md` | 1 | |
| `docs/REVENDOR-0.22.md` | 1 | |
| `docs/modrinth-research/*.md` (§12) | 6 | research history |
| `docs/superpowers/specs/*.md` | 4 | design history |
| `.gitattributes` | 1 | `crates/palantir-desktop/assets/fonts/** -text` — **a real path glob**, not a comment |
| `THIRD_PARTY_NOTICES.md` | 9 | path references into the crate |
| `REFERENCE.md` | 4 | path references |
| `TRANSFER.md` | 4 | `-p palantir-desktop`, `crates/…` |
| `HANDOFF.md`, `NEXT_STEPS.md`, `GATES.md`, `UI-HANDOFF.md`, `PARITY-HANDOFF.md`, `MODRINTH-RESEARCH-PROMPT.md` | rest | history (§12) |
| `tools/*.py`, `tools/vps_setup.sh`, `tools/parity/compose.py` | 46 | default paths into `crates/palantir-desktop`, `crates/palantir-egui` — a count that keeps growing: `tools/gen_icons_egui.py` arrived mid-rename and the script picks it up without being told |
| `.github/workflows/ci.yml` | 7 | §8 |

The `-p` flags that change: `-p palantir-desktop` (×4: `AGENTS.md`, `installer/sift.nsi`,
`ci.yml:237`, `release.yml:106`), `-p palantir-net` (×2: `AGENTS.md`,
`ci.yml:176`).

---

## 4. Binary target and Windows file properties

The executable is named by `[[bin]]`, and cargo target names are path-shaped, so
the target becomes **`SiftLauncher`** (exe `SiftLauncher.exe`) and not
`Sift Launcher`: a target name with a space would have to be quoted in every
`cargo test --bin` line, and every artifact path would need one too. The
installer already renames the payload to `Sift Launcher.exe` at install time
(`File /oname=` + `Rename`), which stays true.

| File | Line | Exact text today | Exact replacement |
| --- | ---: | --- | --- |
| `crates/palantir-desktop/Cargo.toml` | 15 | `name = "PalantirMC"` | `name = "SiftLauncher"` |
| `crates/palantir-egui/Cargo.toml` | 27 | `name = "PalantirEgui"` | `name = "SiftUi"` |
| `crates/palantir-desktop/build.rs` | 33 | `const PRODUCT: &str = "PalantirMC";` | `const PRODUCT: &str = "Sift Launcher";` |
| `crates/palantir-desktop/build.rs` | 36 | `const ORIGINAL_FILENAME: &str = "PalantirMC.exe";` | `const ORIGINAL_FILENAME: &str = "SiftLauncher.exe";` |
| `crates/palantir-desktop/build.rs` | 42 | `const COPYRIGHT: &str = "Copyright (c) 2026 Palantir Studios. All rights reserved.";` | **unchanged** (§10) |
| `crates/palantir-desktop/build.rs` | 69 | `resource.set("CompanyName", "Palantir Studios");` | **unchanged** (§10) |
| `crates/palantir-desktop/build.rs` | 30 | ``/// `palantir_core::PRODUCT_NAME` cannot be read from here without making the`` | ``/// `sift_core::PRODUCT_NAME` …`` |
| `crates/palantir-desktop/src/main.rs` | 2 | `//! PalantirMC desktop entry point (iced 0.12).` | `//! Sift Launcher desktop entry point (iced 0.12).` |
| `crates/palantir-desktop/src/main.rs` | 10 | ``//! decorations and carries the PalantirMC icon. `cargo build`/`cargo test` `` | `//! decorations and carries the Sift Launcher icon. …` |
| `crates/palantir-desktop/src/main.rs` | 361 | `eprintln!("PalantirMC hit an internal error and had to close.\n  {message}\n  at {location}\n  written to {}", panic_log_path().display());` | `eprintln!("Sift Launcher hit an internal error and had to close. …")` |
| `crates/palantir-egui/src/main.rs` | 37 | `.with_title("PalantirMC — egui slice"),` | `.with_title("Sift Launcher — egui slice"),` |
| `crates/palantir-egui/src/main.rs` | 41 | `"PalantirMC",` | `"Sift Launcher",` |
| `crates/palantir-egui/src/main.rs` | 69 | `ui.heading("PalantirMC");` | `ui.heading("Sift Launcher");` |
| `.github/scripts/check_exe.py` | 4 | `` `PalantirMC.exe` is downloaded by people who will not install a toolchain, so`` | `` `SiftLauncher.exe` is downloaded … `` |
| `.github/scripts/check_exe.py` | 15 | `Usage:  python .github/scripts/check_exe.py path/to/PalantirMC.exe` | `… path/to/SiftLauncher.exe` |

`build.rs`'s `PRODUCT` and `brand::APP_NAME` are held equal by
`the_resource_name_is_the_product_name` (`brand.rs:338`), so both must move in
the same commit or that test fails.

---

## 5. `PRODUCT_NAME`, window title, app id, single instance

| File | Line | Exact text today | Exact replacement |
| --- | ---: | --- | --- |
| `crates/palantir-core/src/lib.rs` | 59 | `pub const PRODUCT_NAME: &str = "PalantirMC";` | `pub const PRODUCT_NAME: &str = "Sift Launcher";` |
| `crates/palantir-desktop/src/brand.rs` | 40 | `pub const APP_NAME: &str = palantir_core::PRODUCT_NAME;` | `pub const APP_NAME: &str = sift_core::PRODUCT_NAME;` |
| `crates/palantir-desktop/src/brand.rs` | 1 | `//! PalantirMC branding: the launcher logo and the window/taskbar icon.` | `//! Sift Launcher branding: …` |
| `crates/palantir-desktop/src/brand.rs` | 53 | ``/// `PalantirMC 0.1.0` — used on the About page and in logs.`` | ``/// `Sift Launcher 0.1.0` — …`` |
| `crates/palantir-desktop/src/brand.rs` | 264 | `assert_eq!(APP_NAME, "PalantirMC");` | `assert_eq!(APP_NAME, "Sift Launcher");` |
| `crates/palantir-desktop/src/brand.rs` | 265 | `assert_eq!(window_title(), "PalantirMC");` | `assert_eq!(window_title(), "Sift Launcher");` |
| `crates/palantir-desktop/src/brand.rs` | 266 | `assert!(full_name().starts_with("PalantirMC "));` | `assert!(full_name().starts_with("Sift Launcher "));` |
| `crates/palantir-desktop/src/shell.rs` | 11742 | `format!("PalantirMC {}", brand::version()),` | `format!("Sift Launcher {}", brand::version()),` |
| `crates/palantir-desktop/src/theme.rs` | 1 | `//! PalantirMC look & feel.` | `//! Sift Launcher look & feel.` |
| `crates/palantir-desktop/src/theme.rs` | 725 | `"PalantirMC".to_string(),` | `"Sift Launcher".to_string(),` |
| `crates/palantir-desktop/src/theme.rs` | 2063 | `assert!(format!("{theme:?}").contains("PalantirMC"));` | `… contains("Sift Launcher")` |
| `crates/palantir-core/src/launch.rs` | 773 | `window_title: "PalantirMC: T".into(),` | `window_title: "Sift Launcher: T".into(),` |
| `crates/palantir-core/src/launch.rs` | 919 | `assert!(script.contains("windowTitle PalantirMC: My Inst\n"));` | `… "windowTitle Sift Launcher: My Inst\n"` |
| `crates/palantir-core/src/launch.rs` | 921 | `assert!(script.contains("launcherBrand PalantirMC\n"));` | `… "launcherBrand Sift Launcher\n"` |
| `crates/palantir-core/tests/compat.rs` | 347 | `"windowTitle PalantirMC: Fabric 1.21.1\n",` | `"windowTitle Sift Launcher: Fabric 1.21.1\n",` |
| `crates/palantir-core/tests/compat.rs` | 349 | `"launcherBrand PalantirMC\n",` | `"launcherBrand Sift Launcher\n",` |
| `crates/palantir-desktop/src/single.rs` | 40 | `pub const LOCK_FILE: &str = "palantir.pid";` | `pub const LOCK_FILE: &str = "sift.pid";` |
| `crates/palantir-desktop/src/single.rs` | 105 | `` `"PalantirMC is already running (process {pid}).` `` | `` `"Sift Launcher is already running (process {pid}).` `` |
| `crates/palantir-desktop/src/single.rs` | 109 | `let title: Vec<u16> = "PalantirMC"` | `let title: Vec<u16> = "Sift Launcher"` |
| `crates/palantir-desktop/src/single.rs` | 126 | `eprintln!("PalantirMC is already running (process {pid}).");` | `eprintln!("Sift Launcher is already running (process {pid}).");` |

**Window title.** `shell.rs:11813` returns `brand::window_title()`, which is
`APP_NAME`, which is `PRODUCT_NAME` — one literal decides the title bar, the
About page, the iced widget theme's name and the `windowTitle` /
`launcherBrand` lines of the launch script. It is not repeated anywhere else.

**App id: there is none.** No `SetCurrentProcessExplicitAppUserModelID`,
`AppUserModelID`, `app_id` or `AppId` exists anywhere in the tree, so nothing
renames. The window class is winit's own (`iced_winit` registers it), so
taskbar grouping falls back to the exe path. If a stable app id is wanted after
the rename it has to be *added* — that is a behaviour change, not a rename, and it
is not in this inventory.

**Single instance is not a mutex.** `single.rs` writes a pid into
`LOCK_FILE` with an exclusive create and asks the OS whether that pid is alive;
there is no named mutex and no named event. The only name that changes is
`palantir.pid` → `sift.pid`, and it lives at `PalantirPaths::home()` — so
`%APPDATA%\SiftLauncher\sift.pid` after §6.

---

## 6. Data directories, cache paths, prefs file, lock and logs

| File | Line | Exact text today | Exact replacement |
| --- | ---: | --- | --- |
| `crates/palantir-core/src/paths.rs` | 42 | `pub const DATA_DIR_NAME: &str = "PalantirMC";` | `pub const DATA_DIR_NAME: &str = "SiftLauncher";` |
| `crates/palantir-core/src/paths.rs` | 15 | ``//!   `%APPDATA%\PalantirMC` and its macOS/Linux equivalents. This launcher's own`` | ``//!   `%APPDATA%\SiftLauncher` …`` |
| `crates/palantir-core/src/paths.rs` | 333 | ``/// This product's platform data directory (`%APPDATA%\PalantirMC`, ...).`` | ``… (`%APPDATA%\SiftLauncher`, ...).`` |
| `crates/palantir-core/src/paths.rs` | 530 | `Some(appdata) => PathBuf::from(appdata).join("PalantirMC"),` | `… .join("SiftLauncher"),` |
| `crates/palantir-core/src/paths.rs` | 535 | `.join("PalantirMC"),` | `.join("SiftLauncher"),` |
| `crates/palantir-core/src/paths.rs` | 542 | `.join("Library/Application Support/PalantirMC")` | `.join("Library/Application Support/SiftLauncher")` |
| `crates/palantir-core/src/paths.rs` | 545 | `Some(v) => PathBuf::from(v).join("PalantirMC"),` | `… .join("SiftLauncher"),` |
| `crates/palantir-core/src/paths.rs` | 548 | `.join(".local/share/PalantirMC"),` | `.join(".local/share/SiftLauncher"),` |
| `crates/palantir-core/src/paths.rs` | 636 | `let home = PalantirPaths::at("/home/u/.local/share/PalantirMC");` | `… "/home/u/.local/share/SiftLauncher");` |
| `crates/palantir-core/src/paths.rs` | 648 | (same) | (same) |
| `crates/palantir-core/src/paths.rs` | 658 | (same) | (same) |
| `crates/palantir-core/src/paths.rs` | 68 | `pub const PREFS_FILE: &str = "palantirmc-desktop.json";` | `pub const PREFS_FILE: &str = "sift-desktop.json";` |
| `crates/palantir-desktop/src/prefs.rs` | 455 | ``/// `palantirmc-desktop.json` on the next save: a machine that has never`` | ``/// `sift-desktop.json` on the next save: …`` |
| `crates/palantir-desktop/src/single.rs` | 246 | `.join("palantirmc-single-tests")` | `.join("sift-single-tests")` |
| `crates/palantir-desktop/src/shell.rs` | 807 | `const FRAME_ID: &str = "palantirmc-shell-frames";` | `"sift-shell-frames"` |
| `crates/palantir-desktop/src/shell.rs` | 11598 | `const WINDOW_STATE_ID: &str = "palantirmc-window-state";` | `"sift-window-state"` |
| `crates/palantir-desktop/src/shell.rs` | 11600 | `const SHOT_ID: &str = "palantirmc-shot";` | `"sift-shot"` |
| `crates/palantir-desktop/src/shell.rs` | 11602 | `const REPAINT_ID: &str = "palantirmc-repaint";` | `"sift-repaint"` |
| `crates/palantir-desktop/src/wire.rs` | 256, 397 | `"palantirmc-wire-tests"`, `"palantirmc-script-wire"` | `"sift-wire-tests"`, `"sift-script-wire"` |
| `crates/palantir-desktop/src/store.rs` | 2358 | `.join("palantirmc-store-tests")` | `.join("sift-store-tests")` |
| `crates/palantir-desktop/src/scale.rs` | 73 | `.join("palantirmc-scale")` | `.join("sift-scale")` |
| `crates/palantir-desktop/src/update.rs` | 304 | `.join("palantirmc-update-tests")` | `.join("sift-update-tests")` |
| `crates/palantir-desktop/src/meta.rs` | 258 | `.join("palantirmc-meta-routing")` | `.join("sift-meta-routing")` |
| `crates/palantir-desktop/src/pages/screenshots.rs` | 289, 296 | `"palantirmc-shots-page"`, `"palantirmc-shots-draw"` | `"sift-shots-page"`, `"sift-shots-draw"` |
| `crates/palantir-desktop/src/pages/instance.rs` | 955 | `.join("palantirmc-instance-page")` | `.join("sift-instance-page")` |
| `crates/palantir-desktop/src/pages/home.rs` | 1365 | `.join("palantirmc-home-welcome")` | `.join("sift-home-welcome")` |
| `crates/palantir-net/src/engine/forge.rs` | 1715 … 2678 (12×) | `.join("palantirmc-engine-forge")` | `.join("sift-engine-forge")` |
| `crates/palantir-net/src/engine/{cache,content,download,loaders,modrinth,piston,schedule}.rs` | 453 / 509 / 335 / 550 / 716 / 424 / 429 | `.join("palantirmc-engine-…")` | `.join("sift-engine-…")` |
| `crates/palantir-net/tests/live.rs` | 1282, 1334, 1975 | `"palantirmc-live-metadata-cache"`, `"palantirmc-live-loaders"`, `…/tag/palantirmc-live-no-such-loader` | `"sift-live-…"` |
| `installer/sift.nsi` | 45 | `!define DATA_DIR "$APPDATA\PalantirMC"` | `!define DATA_DIR "$APPDATA\SiftLauncher"` |
| `installer/sift.nsi` | 6 | `; %APPDATA%\PalantirMC — the uninstaller asks before touching that.` | `; %APPDATA%\SiftLauncher — …` |
| `.gitignore` | 7 | `.palantir-test-data/` | `.sift-test-data/` — nothing creates it any more; deleting the line is the better edit |

Three corrections to the brief's assumptions:

* **`%LOCALAPPDATA%\PalantirMC` does not exist.** The launcher writes nothing
  under `LOCALAPPDATA`; `core/java.rs:241` and `desktop/bedrock.rs:126` read it
  for *other* software's data (Java installs, Bedrock worlds). The only
  `LOCALAPPDATA` path in the project is the installer's program folder,
  `$LOCALAPPDATA\Programs\Sift Launcher`, which is already new-named.
* **`palantirmc.log` does not exist.** The launcher's logs are
  `logs/launcher.log` (`desktop/launch.rs:2219`), `logs/panic.log`
  (`desktop/main.rs:389`) and the game's own `logs/latest.log`
  (`desktop/store.rs:2319`) — all inside the instance or the data root, none
  carrying the product name. Nothing to rename.
* **The cache carries no product name.** `<data root>/cache`, `<root>/cache/meta`,
  `<root>/icons`, `<root>/assets`, `<root>/libraries`, `<root>/versions` are
  Prism-compatible on purpose; they move only because the *root* moves.

---

## 7. Environment variables and every place they are read

| Variable today | Replacement | Declared at | Read at |
| --- | --- | --- | --- |
| `PALANTIRMC_HOME` | `SIFT_HOME` | `core/paths.rs:55` | `core/paths.rs:135` (`var_os(HOME_ENV)`), `core/paths.rs:130` (doc) |
| `PALANTIRMC_FRAME_LOG` | `SIFT_FRAME_LOG` | `desktop/frame_log.rs:33`, `egui/frame_log.rs:19` | `desktop/frame_log.rs:112`, `:264`; `egui/frame_log.rs:76`; docs at `desktop/main.rs:55`, `desktop/shell.rs:4966`, `:11872`, `egui/main.rs:5`, `:18`, `frame_log.rs:7` |
| `PALANTIRMC_CURSEFORGE_API_KEY` | `SIFT_CURSEFORGE_API_KEY` | `desktop/prefs.rs:65` | `desktop/prefs.rs:440` (`var(CURSEFORGE_KEY_ENV)`), `desktop/shell.rs:12118` (`var_os(crate::prefs::CURSEFORGE_KEY_ENV)`); UI copy at `shell.rs:8686`, docs at `prefs.rs:113` |
| `PALANTIRMC_MSA_CLIENT_ID` | `SIFT_MSA_CLIENT_ID` | — (literal) | `desktop/prefs.rs:410` (`var("PALANTIRMC_MSA_CLIENT_ID")`), docs at `:93`, `:404` |
| `PALANTIRMC_SHOT` | **does not exist** | — | the capture is `--shot PATH`, a CLI flag: `shell.rs:11555`–`11575` (`Flags::from_args`), `main.rs:417`–`428` |

Totals: 27 occurrences in 11 files — 16 in the seven source files above, 11 in
documents (`NOTES.md` 5, `HANDOFF.md` 4, `docs/PLAN-SIFT.md` 1,
`docs/LIGHTNESS.md` 1). Of the documents the script replaces only
`docs/LIGHTNESS.md`; `NOTES.md` and `docs/PLAN-SIFT.md` are excluded, and
`docs/LIGHTNESS.md:42` is the one place its substitution reads badly —
`` `SIFT_FRAME_LOG` (today `PALANTIRMC_FRAME_LOG`; slice R1 …) `` becomes a
sentence that says `SIFT_FRAME_LOG` twice. **Hand-edit that line** to
"`SIFT_FRAME_LOG`; slice R1".

The script renames the literals; it does **not** add the "read the old name once"
fallback that `docs/PLAN-SIFT.md` R1 asks for — that is a code change (a second
`var_os` and a choice between two answers) and belongs in the same commit as a
hand edit, most naturally in `paths.rs` and `prefs.rs`.

`PALANTIRMC_MSA_CLIENT_ID` is not in the brief's list and is real: it is the
fourth variable, read at `prefs.rs:410`.

---

## 8. CI: artifacts, `-p` flags, hard-coded exe names

| File | Line | Exact text today | Exact replacement |
| --- | ---: | --- | --- |
| `.github/workflows/ci.yml` | 176 | `run: cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1` | `-p sift-net` |
| `.github/workflows/ci.yml` | 237 | `run: cargo build --release --locked -p palantir-desktop --target ${{ matrix.target }}` | `-p sift-desktop` |
| `.github/workflows/ci.yml` | 243 | `Copy-Item -Path "target/${{ matrix.target }}/release/PalantirMC.exe" -Destination dist/PalantirMC.exe` | both → `SiftLauncher.exe` |
| `.github/workflows/ci.yml` | 245 | `$exe = Get-Item -Path dist/PalantirMC.exe` | `dist/SiftLauncher.exe` |
| `.github/workflows/ci.yml` | 251 | `throw "Unexpected PalantirMC.exe size: $($exe.Length) bytes"` | `"Unexpected SiftLauncher.exe size: …"` |
| `.github/workflows/ci.yml` | 257 | ``…WriteAllText("dist/PalantirMC.exe.sha256", "$hash  PalantirMC.exe`n")`` | both → `SiftLauncher.exe` |
| `.github/workflows/ci.yml` | 259 | `$sidecar = …ReadAllText("dist/PalantirMC.exe.sha256")` | `dist/SiftLauncher.exe.sha256` |
| `.github/workflows/ci.yml` | 260 | ``if ($sidecar -ne "$hash  PalantirMC.exe`n") {`` | ``"$hash  SiftLauncher.exe`n"`` |
| `.github/workflows/ci.yml` | 264 | `"### PalantirMC.exe (${{ matrix.target }})" \| Out-File …` | `"### SiftLauncher.exe (…)"` |
| `.github/workflows/ci.yml` | 275 | `run: python .github/scripts/check_exe.py dist/PalantirMC.exe` | `dist/SiftLauncher.exe` |
| `.github/workflows/ci.yml` | 280 | `name: palantirmc-${{ matrix.target }}` | `name: sift-${{ matrix.target }}` |
| `.github/workflows/ci.yml` | 310 | `key: ${{ runner.os }}-egui-${{ hashFiles('crates/palantir-egui/Cargo.toml') }}` | `crates/sift-ui/Cargo.toml` |
| `.github/workflows/ci.yml` | 315 | `working-directory: crates/palantir-egui` | `crates/sift-ui` |
| `.github/workflows/ci.yml` | 319 | `working-directory: crates/palantir-egui` | `crates/sift-ui` |
| `.github/workflows/ci.yml` | 324 | `name: palantir-egui-slice` | `name: sift-ui-slice` |
| `.github/workflows/ci.yml` | 325 | `path: crates/palantir-egui/target/release/PalantirEgui.exe` | `crates/sift-ui/target/release/SiftUi.exe` |
| `.github/workflows/release.yml` | 106 | `run: cargo build --release --locked -p palantir-desktop --target ${{ matrix.target }}` | `-p sift-desktop` |
| `.github/workflows/release.yml` | 112 | `Copy-Item "target/${{ matrix.target }}/release/PalantirMC.exe" dist/PalantirMC.exe` | both → `SiftLauncher.exe` |
| `.github/workflows/release.yml` | 114 | `$exe = Get-Item dist/PalantirMC.exe` | `dist/SiftLauncher.exe` |
| `.github/workflows/release.yml` | 116 | `throw "Unexpected PalantirMC.exe size: …"` | `"Unexpected SiftLauncher.exe size: …"` |
| `.github/workflows/release.yml` | 120 | `run: python .github/scripts/check_exe.py dist/PalantirMC.exe` | `dist/SiftLauncher.exe` |
| `.github/workflows/release.yml` | 125 | `name: palantirmc-${{ matrix.target }}` | `name: sift-${{ matrix.target }}` |
| `.github/workflows/release.yml` | 166 | `$name = "PalantirMC-$label.exe"` | `$name = "SiftLauncher-$label.exe"` |
| `.github/workflows/release.yml` | 210 | `` --title "PalantirMC $tag" ` `` | `` --title "Sift Launcher $tag" ` `` |

Artifact names: `palantirmc-x86_64-pc-windows-msvc`, `palantirmc-x86_64-pc-windows-gnu`
→ `sift-x86_64-pc-windows-msvc`, `sift-x86_64-pc-windows-gnu`; `palantir-egui-slice`
→ `sift-ui-slice`. `sift-` rather than `siftlauncher-` so the artifact prefix
matches the crate prefix used everywhere else (`sift-core`, `sift-net`, …); the
download commands in `AGENTS.md:114`–`116` and `README.md:76` change with it.

`.github/dependabot.yml` has no occurrence. The `.sha256` sidecar name follows
the exe automatically because both are built from `$name`/the literal path.

---

## 9. Installer

`installer/sift.nsi` is **already** named for the new product (`sift.nsi`,
`sift.ico`, `anim/sift.avi`), and its product strings already read "Sift
Launcher". Five lines are old; a sixth carries the old word as the publisher's
name and stays:

| Line | Exact text today | Exact replacement |
| ---: | --- | --- |
| 6 | `; %APPDATA%\PalantirMC — the uninstaller asks before touching that.` | `; %APPDATA%\SiftLauncher — …` |
| 9 | `;   cargo build --release --locked -p palantir-desktop` | `;   cargo build --release --locked -p sift-desktop` |
| 37 | `!define APP_EXE "../target/release/PalantirMC.exe"` | `!define APP_EXE "../target/release/SiftLauncher.exe"` |
| 43 | ``; already made: `palantir-core`'s data root is %APPDATA%\<name>. It is *not*`` | ``; already made: `sift-core`'s data root is %APPDATA%\<name>. …`` |
| 45 | `!define DATA_DIR "$APPDATA\PalantirMC"` | `!define DATA_DIR "$APPDATA\SiftLauncher"` |
| 41 | `!define PUBLISHER "Palantir Studios"` | **unchanged** (§10) |

Output filename is already `dist/SiftLauncher-${VERSION}-x64-setup.exe` (line 63),
the App Paths key is already `Sift Launcher.exe` (line 47), and the install dir is
already `$LOCALAPPDATA\Programs\Sift Launcher` (line 64). No file under
`installer/` needs renaming.

---

## 10. Replacements that are not plain substrings

Five of the eleven crate pairs are plain; six things are not. Each is a rule of
its own in the script, in this order.

| Today | After | Why it is not a substring rule |
| --- | --- | --- |
| `palantir-egui`, `palantir_egui` | `sift-ui`, `sift_ui` | the tail is dropped: it is not `sift-egui`. The crate is the *UI*, and `sift-ui` matches the other four (`-core`, `-net`, `-loader`, `-desktop`). |
| `PalantirMC` (prose) | `Sift Launcher` | display name, with the space — title bar, About page, message box, launch script, installer text. |
| `PalantirMC` (identifier) | `SiftLauncher` | repository, artifact, exe and folder names, where a space is illegal or has to be quoted. |
| `PalantirMC.exe` | `SiftLauncher.exe` | the exe is the identifier spelling, not the display name. |
| `PalantirMC/` | `SiftLauncher/` | HTTP `User-Agent` token (`engine/http.rs:37`, `concat!("PalantirMC/", version)`); a space would split it into two tokens. |
| `PalantirMCSkinBoundary` | `SiftSkinBoundary` | multipart boundary (`net/auth.rs:367`); a space in a boundary is an illegal header value. |
| `PalantirEgui` | `SiftUi` | the egui slice's bin target, following its crate to `sift-ui`. |
| `PalantirPaths` | `SiftPaths` | a Rust *type* named in 166 places; `Palantir` cannot survive as a type prefix or `PLAN-SIFT`'s "palantir survives only in NOTES.md and vendor/" is false. |
| `palantirmc` (lower case) | `sift` | `palantirmc-desktop.json` → `sift-desktop.json`, `palantirmc.pid`-shaped names, test temp dirs, artifact prefixes. **Not** `siftlauncher`: the crate prefix is `sift-`. |
| `PALANTIRMC_` | `SIFT_` | the environment-variable prefix (all four variables). |
| `palantir` (bare lower case) | `sift` | leftovers: `palantir.pid` → `sift.pid`, `.palantir-test-data/`, `palantir-cli`/`palantir-gui` in old comments. |

Ordering matters and the script enforces it: `palantirmc` is replaced before
`palantir` (or `palantirmc-desktop.json` would become `siftmc-desktop.json`), and
every longer `PalantirMC…` form before the bare `PalantirMC`.

---

## 11. Ambiguous: places "Palantir" is not the product name

The "script" column says what `tools/rename_sift.py` does to each row. **`kept`**
rows are untouched; **`rewrites`** rows are changed by a rule that cannot tell
them from the product name, and want a human's eye after `--apply`. Nothing here
should be changed by adding a rule — if one of them really does need to move, it
is a decision with an owner attached, not a substitution.

| Where | Count | Script | What it is |
| --- | ---: | --- | --- |
| `Cargo.toml:25` `authors = ["Palantir Studios"]` | 1 | kept | the **publisher**, a legal entity. Not a product. |
| `LICENSE:2`, `:6` `Palantir Studios` | 2 | kept | copyright holder and the party the licence grants from. |
| `build.rs:42` `COPYRIGHT`, `:69` `CompanyName` | 2 | kept | the same legal entity, in the exe's file properties. |
| `installer/sift.nsi:41` `PUBLISHER` | 1 | kept | the same, in Add/Remove Programs. |
| `README.md:3`, `:152` | 2 | kept | author line and the copyright line. |
| `NOTES.md:1488` | 1 | kept | explains why the studio name is drawn in the About hero. |
| `PalantirApp` (`main.rs:192`) | 1 | kept | a **deleted** state type, named in a doc comment about the shell that was removed. Left as history. |
| `vendor/modrinth-app/UPSTREAM.md:90` | 1 | kept | the vendored reference's provenance note pointing at `crates/palantir-desktop/src/native.rs`. `vendor/` is excluded, so this **survives the rename and becomes stale** — the one place a human should edit by hand, after checking that the vendored tree's own rules allow it. |
| `palantir-gui`, `palantir-cli` (`main.rs:109`, `paths.rs:29`) | 2 | rewrites | crates that no longer exist. Become `sift-gui`/`sift-cli`, which is arguably *wrong*: they are historical names, not future ones. Harmless either way. |
| `LICENSE:1` `PalantirMC` | 1 | rewrites | the product name at the top of its own licence; becomes `Sift Launcher`. The copyright holder does not change. **Owner should confirm** — a licence heading is legal text. |
| `docs/modrinth-research/*`, `docs/superpowers/specs/*` | 92 | rewrites | "the PalantirMC 2.0 rewrite" — research and design *history*. A mechanical rewrite makes the documents claim a name they were not written under. |
| `GATES.md` (645), `HANDOFF.md` (37), `UI-HANDOFF.md` (37), `NEXT_STEPS.md` (34), `MODRINTH-RESEARCH-PROMPT.md` (22), `TRANSFER.md`, `PARITY-HANDOFF.md`, `REFERENCE.md` | 800+ | rewrites | records: gate ledgers and handoffs that quote real command output (`Checking palantir-desktop v0.1.0`, `bin "PalantirMC"`). A rename here turns evidence into fiction. |
| `crates/palantir-desktop/src/update.rs:34`–`39` | 2 | rewrites | a risk note whose whole premise is that the installer installs `Sift Launcher.exe` **from a payload called `PalantirMC.exe`**. After the rename the two names converge and the paragraph argues about nothing. **Needs a hand rewrite**, not a substitution. |
| `crates/palantir-desktop/Cargo.toml:3`, `crates/palantir-core/Cargo.toml:3` | 2 | rewrites | descriptions where the mechanical result ("Sift Launcher's launcher core") reads badly. Cosmetic; worth a hand edit. |
| `docs/LIGHTNESS.md:42` | 1 | rewrites | `SIFT_FRAME_LOG` (today `PALANTIRMC_FRAME_LOG`) becomes a line that says `SIFT_FRAME_LOG` twice. **Hand-edit** to "`SIFT_FRAME_LOG`; slice R1". |

The script's exclusion list is exactly what the brief specified. If the record
documents above should keep their old names, add them to `EXCLUDED_FILES` in
`tools/rename_sift.py` before `--apply`: one tuple entry each, and the dry run
will show their counts drop out.

---

## 12. Out of scope by decision

| Path | Why | Occurrences |
| --- | --- | ---: |
| `Cargo.lock` | derived data; `cargo` regenerates it | 9 |
| `NOTES.md` | the project's history | 58 |
| `docs/PLAN-SIFT.md` | the plan that names the target spellings | 2 |
| `docs/RENAME.md` | this file; it quotes every old string on purpose | — |
| `tools/rename_sift.py` | the tool; it contains every pattern it searches for | — |
| `.git/`, `target/`, `dist/`, `.scratch/`, `vendor/` | not source | — |

---

## 13. Directory renames — for `git mv`, not for the script

The script rewrites file *contents*. The five directories it makes the manifests
point at still have the old names until someone runs these, in this order, from
the repository root:

```sh
git mv crates/palantir-core    crates/sift-core
git mv crates/palantir-net     crates/sift-net
git mv crates/palantir-loader  crates/sift-loader
git mv crates/palantir-desktop crates/sift-desktop
git mv crates/palantir-egui    crates/sift-ui
```

Run `--apply` **first**, then the `git mv` commands, then `cargo test --workspace`
and let cargo rewrite `Cargo.lock`. Applying without the moves leaves the
manifests naming directories that do not exist, which is why the script prints
this list every time it runs.
