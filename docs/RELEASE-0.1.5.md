# Sift Launcher 0.1.5

The current build. Same product as 0.1.0, with the settings modal, the row menu, the worlds
block, the select popovers and the launch state in the title bar since then.

## What it does

- **Discover** with a live Modrinth search: the tab strip chooses the project type, the two
  selects open their options, and the count is the API's own total.
- **Create an instance** — a real one, in `%LOCALAPPDATA%\SiftLauncher\instances\`.
- **Play** — installs the Java runtime, libraries, assets and client jar, then launches the
  game. The title bar shows the state: `Preparing…`, `Downloading 34/120`, `Running`, `Exited (0)`.
- **Every page the reference's rail has**: Home, Discover, Skins, Screenshots, Servers, plus a
  project page, a settings modal with all fourteen tabs and an About view with the notices.
- **Instance rows** carry the reference's five verbs: Play, Edit, Duplicate instance, Open
  folder, Delete — with Delete refused while a launch is running.

## Measured on the shipped profile

| Budget | Limit | This build |
| --- | --- | --- |
| Startup | ≤ 400 ms | **53 ms** |
| Idle CPU | 0 ticks | **0 / 10 s** |
| Idle RSS | ≤ 120 MB | **114.6 MB** |
| Scroll p95 | ≤ 20 ms | **18.53 ms** |
| Exe | ≤ 28.65 MB (the previous iced build) | **10.79 MB** |

## Downloads

| File | What it is |
| --- | --- |
| `SiftLauncher-x64-msvc.exe` | static CRT; runs with no Visual C++ redistributable. Recommended. |
| `SiftLauncher-x64-gnu.exe` | the MinGW build, which the project is developed against locally |

Each has a `.sha256` beside it (`sha256sum -c` form) and a `.zip` holding the exe and its hash.

**There is no installer in this release.** `installer/sift.nsi` exists with the product's icon,
the Start-menu entry and the uninstall record, and compiles under POSIX `makensis`; on the
runner's NSIS 3.13 it aborts at its animated welcome banner. Recorded as owed rather than
blocking the product.

## Known limits, stated rather than discovered

- **Unsigned.** SmartScreen warns about an unknown publisher; check the hash.
- **Offline account only.** Every launch is the fixed offline `Player`, so online-mode servers
  refuse it.
- **Skins, Screenshots and Servers show sample or no data** — no upload, no renderer, no ping.
- **Settings:** sliders, text fields and chip groups draw as the select plate, and rich-text
  strings keep their markup unrendered. 24 `owed:` notes in that file say which row is which.
- **Worlds** list their folder names and do not launch; real names live in `level.dat`.
