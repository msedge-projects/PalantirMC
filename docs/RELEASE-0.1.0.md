# Sift Launcher 0.1.0

The first release of the egui rewrite: **one UI, one engine, no iced.**

## What it does

- **Discover** with a live Modrinth search: the tab strip chooses the project type, the two
  selects choose the API's sort and page size, and the count is the API's own total.
- **Create an instance** — a real one, written into `%LOCALAPPDATA%\SiftLauncher\instances\`.
- **Play** — installs the Java runtime, libraries, assets and client jar, then launches the
  game. The title bar shows the launch's state (`Preparing…`, `Downloading 34/120`, `Running`,
  `Exited (0)`).
- **Every page the reference's rail has**: Home, Discover, Skins, Screenshots, Servers, plus a
  project page, a settings modal and an About view carrying the third-party notices.

## Measured

| Budget | Limit | This build |
| --- | --- | --- |
| Startup | ≤ 400 ms | **53 ms** |
| Idle CPU | 0 ticks | **0 / 10 s** |
| Idle RSS | ≤ 120 MB | **114.6 MB** |
| Scroll p95 | ≤ 20 ms | **18.53 ms** |
| Exe | ≤ 28.65 MB (previous iced build) | **10.79 MB** |

## Downloads

| File | What it is |
| --- | --- |
| `SiftLauncher-x64-msvc.exe` | static CRT; runs with no Visual C++ redistributable. Recommended. |
| `SiftLauncher-x64-gnu.exe` | the MinGW build, which the project is developed against locally |

Each has a `.sha256` beside it (`sha256sum -c` form) and a `.zip` holding the exe and its hash.

## Known limits, stated rather than discovered

- **Unsigned.** Windows SmartScreen will warn about an unknown publisher; check the hash.
- **Offline account only.** Every launch is the fixed offline `Player`, so online-mode servers
  will refuse it. The account flow is the next slice.
- **Eleven settings tabs are named but not built** (Appearance, Features and Privacy are
  complete). Their rows are listed as owed in `crates/sift-ui/src/pages/settings.rs`.
- **Skins, Screenshots and Servers show sample data** — no upload, no screenshot scan, no ping.
- **No log window yet**; the game's own output is discarded and the title bar carries the state.
