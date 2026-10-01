# Moving this work to the Ubuntu machine

Why this document exists: the machine this tree has been built on **cannot read its
own screen**. Every capture tool in `tools/` here is Win32 — `winshot.py`,
`refwalk.py` and `appshot.py` all reach `user32` through `ctypes` — and the three
gaps this campaign has had to accept in writing each end with the same sentence: a
machine whose screen can be read settles it in one command.

* **G135**: "no gate photographs the shaped text... `PrintWindow` on this tree's
  window comes back without the page in it."
* **G138**: "nothing here builds a renderer and reads the nodes back."
* **G141**: "there is no capture of the dialog after the change."

The Ubuntu box is that machine. It can also run the reference client beside ours,
which this one cannot, and a panic there can be caught in a file instead of a window
that vanishes.

## What travels

| what | how | why |
| --- | --- | --- |
| the tree | `git clone` (below) | both remotes carry every commit, the vendored reference, the fonts and the lock file; nothing else is needed to build |
| the evidence | `python tools/pack_transfer.py`, then copy `transfer/*.tar.gz` | the two recordings, the 1 fps stills and the two 180-frame bursts the scroll measurement read. None of it is in git |
| your own data | optional, see below | instances, account and preferences, so the pages have something to draw |
| `dist/` | **no** | Windows exes; useless there, and the Linux binary is built locally |
| `target/`, `.scratch/` | **no** | working directories |

## On the Ubuntu box, in order

1. **Dependencies, toolchain and the reference client:**

   ```bash
   bash tools/vps_setup.sh
   ```

2. **The tree, built and tested:**

   ```bash
   git clone https://github.com/msedge-projects/PalantirMC.git
   cd PalantirMC
   CARGO_BUILD_JOBS=1 cargo test -p palantir-desktop --offline --locked
   cargo build --release -p palantir-desktop        # -> target/release/PalantirMC
   ```

3. **A display that is not a person's desktop, and the tool that reads it:**

   ```bash
   Xvfb :99 -screen 0 1920x1080x24 &
   export DISPLAY=:99
   python tools/xshot.py --launch target/release/PalantirMC --client 1280x720 --out shot.png
   ```

4. **The reference beside it**, at the same client size, with the WebKitGTK headless
   workarounds Tauri documents (`WEBKIT_DISABLE_DMABUF_RENDERER=1` first, and
   `WEBKIT_DISABLE_COMPOSITING_MODE=1` if the window is still blank — the known
   symptom is a window that opens and renders nothing, with no error):

   ```bash
   export WEBKIT_DISABLE_DMABUF_RENDERER=1
   modrinth-app &
   python tools/xshot.py --title "Modrinth App" --client 1280x720 --out reference.png
   ```

5. **Compare** the two captures at 1280x720, page by page. That is the comparison
   every gate in this tree has been reasoning about instead of looking at.

## Your own data, if you want the pages populated

The launcher keeps its data root in the platform data directory, and this tree's
`palantir_core::paths` has a tested `System::Linux` branch for it: on Ubuntu that is
`$XDG_DATA_HOME/PalantirMC`, which is `~/.local/share/PalantirMC`. Copy the Windows
folder there:

```bash
# from the Windows machine
scp -r "$APPDATA/PalantirMC" user@vps:~/.local/share/PalantirMC
```

For a *clean* comparison, do not copy it: put an empty file named `portable.dat`
next to the binary and the whole data root becomes that directory
(`palantir_core::paths::portable_dir`) — which is what `tools/xshot.py --portable`
does, and what the page gates here already use.

## What is different about the tooling there

* `winshot.py`, `refwalk.py` and `appshot.py` do not run on Linux; `winshot.py` now
  says so instead of failing on `ctypes.windll`. `tools/xshot.py` is the X11
  counterpart: the same flags, the same `--script` format (`click X Y`, `wait
  SECONDS`, `shot OUT.png`), with `xdotool` for input and `ffmpeg`'s `x11grab` for
  pixels.
* `tools/progress.cmd` is the Windows wrapper; on Linux run `python tools/progress.py`.
* `dist/` is CI's Windows staging. Locally the Linux binary is
  `target/release/PalantirMC`, and `cargo build --release --locked -p palantir-desktop`
  is the local equivalent of the `package` job.
* **CI still builds and gates on Windows.** Pushing still has to go green there and
  the exes still come from the runner. A change verified on Ubuntu is verified, not
  shipped.

## The first jobs there, in the order they matter

1. **Read the screens this tree has never seen.** Run `xshot.py` once, then capture
   the four surfaces the report named — Library, Discover, *Create instance*,
   *Settings > Appearance* — and three gaps close with a picture instead of an
   argument: G141's dialog reservation (the body drawn ten pixels narrower than its
   padding, so the bar iced draws over the content covers nothing), G138's grids, and
   G135's shaped text.
2. **The crash.** Launch with stderr to a file, drive `click <a Discover result
   card>`, `wait`, `click <a blank area>` with `xshot.py --script`, and fix what the
   panic names. Then keep that sequence as a test, because a crash nobody can
   reproduce by hand is a crash that comes back.
3. **The lag that is left.** `crate::scroll::window` has to reach the Discover and
   Home lists, which needs the scroll geometry `page::body` does not currently hand
   out (it returns the region already converted to an `Element`). The card's height
   is already a constant: 100 (`avatar::ICON_SIDE`) + 8 (`CARD_ROW_GAP`) + 24
   (`TAG_HEIGHT`) + 32 (`p-4` twice) = 164.
4. **The parity inventory** in `NOTES.md` §31, each item against a paired capture.

## Honest notes about this move

* **`tools/xshot.py` has never been run.** It was written on Windows, where it
  cannot be tested, so the first session on the Ubuntu box should treat it as a
  draft: run it once, fix what is wrong with it, and commit that fix. Everything
  else in this document is either a command this tree already runs or a fact from
  the reference's own release page.
* **A VPS has no GPU**, so this launcher takes the same software rasteriser path the
  Windows box has been measured on — comparable in kind, but the absolute frame
  costs there are that machine's and should be measured rather than assumed.
* **The reference under Xvfb is not the reference in the recordings.** WebKitGTK on
  software rendering is slower, and its scroll timing there says nothing about the
  recording's timing. What the Ubuntu box settles for the reference is layout,
  spacing, colour and pixels; motion numbers stay with the two recordings, measured
  by `tools/scroll_lag.py`.
* **The recordings are the only irreplaceable evidence.** If only one thing is
  copied, copy `transfer/palantirmc-evidence-*.tar.gz`: every derived frame in it can
  be rebuilt from the two `.mp4`s with the `ffmpeg` commands in
  `tools/scroll_lag.py`'s docstring, and the `.mp4`s cannot be rebuilt from anything.
