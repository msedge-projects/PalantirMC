# Sift Launcher 0.1.7

The release that carries the account. Everything below was verified on the `sift-launcher` VM,
most of it by driving the window and reading the screen rather than by a unit test.

## What is new since 0.1.6

- **Sign in with Microsoft.** Press `Sign in to Minecraft` in the Getting started panel; the
  launcher asks Microsoft for a device code and shows it: *"To sign in, enter XXXXXXXX at
  https://www.microsoft.com/link"*. A signed-in launch starts as that account (`user_type = msa`,
  the real uuid and access token), and the checklist then reads `Signed in as <name>`. Nobody
  signed in still launches as the offline `Player`.
- **Installs load.** A `.mrpack`'s mods and overrides are written into the instance's
  `minecraft/` folder, where the game and the launcher both look. Pressing `Install` on a
  Discover card installs 65 mods of a real pack into a Fabric instance built from the pack's own
  manifest.
- **Worlds carry their names**, read from `level.dat`, and the `Jump in` block now looks in the
  folder the game actually uses.
- **Skins** has the reference's two sections — `Saved skins` with the add dropzone as its first
  tile, then `Default skins` — and its preview column with the chosen skin, `Edit skin` and the
  demo-account bar.
- **An instance page**, opened from a row's `Edit`: the name, its loader line, Play, and the
  reference's five tabs.
- **Settings**: the theme selector is the reference's `Dark` / `Light` / `OLED` preview cards
  with `Sync with system` beneath them, and the dialog shows the product and its version at the
  foot.
- **Messages are visible.** The toast was painted before the content column and covered by it,
  so every message this launcher had ever produced was invisible. It is drawn last now.

## Measured on the shipped profile

| Budget | Limit | This build |
| --- | --- | --- |
| Startup | ≤ 400 ms | **53 ms** |
| Idle CPU | 0 ticks | **0 / 10 s** |
| Idle RSS | ≤ 120 MB | **114.6 MB** |
| Scroll p95 | ≤ 20 ms | **18.53 ms** |
| Exe | ≤ 28.65 MB (the previous iced build) | **10.79 MB** |

Those five were measured at 0.1.0 and nothing since has added a dependency to the binary's hot
path; the next measurement pass is owed and named as such rather than assumed.

## Downloads

`SiftLauncher-x64-msvc.exe` (static CRT, recommended) and `SiftLauncher-x64-gnu.exe`, each with a
`.sha256` and a `.zip`. **No installer in this release** — `installer/sift.nsi` compiles under
POSIX `makensis` but aborts on the runner's NSIS 3.13 at its animated welcome banner.

## Known limits, stated rather than discovered

- **Unsigned.** SmartScreen warns about an unknown publisher; check the hash.
- **Sign-in is memory only.** A restart asks again; a refresh token is not stored yet.
- **Skins, Screenshots and Servers show sample or no data** — no renderer, no ping, no upload.
- **Settings** draws sliders, text fields and chip groups as plates, and leaves rich-text strings
  with their markup visible. 24 `owed:` notes say which row is which.
- **Quick-play launches nothing**; a world's name shows but its Play does not.
