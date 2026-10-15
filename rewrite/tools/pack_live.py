"""Live acceptance: a catalogue modpack downloaded into a new collection.

Drives the actual window against the live catalogue -- Explore, search, the
project page, pack setup, the review -- and asserts the pack's own indexed
files land on disk in a brand-new collection. This is the acceptance the
work order left open ("catalogue-to-new-pack download remains"), and unlike
tools/ui_smoke.py it needs the network and a real catalogue project.

Run from rewrite/: python3 tools/pack_live.py --binary target/release/PalantirMC
Requires a private, ready X11 DISPLAY, xdotool, ImageMagick import and tesseract.
"""

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time


def command(*args):
    return subprocess.run(args, check=True, text=True, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE).stdout


def edit_distance(a, b):
    previous = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        current = [i]
        for j, cb in enumerate(b, 1):
            current.append(min(current[-1] + 1, previous[j] + 1,
                               previous[j - 1] + (ca != cb)))
        previous = current
    return previous[-1]


def ocr_match(phrase, text, fuzzy):
    """Exact containment, or (fuzzy) near-containment within OCR's error rate.

    The same contract as tools/ui_smoke.py: one word-count-exact window
    within two character edits, never more. "Install the pack archive" stays
    three edits from the pending "Installing the pack archive", so the
    disabled state can never be clicked as the button.
    """
    clean = lambda s: " ".join(
        "".join(c if c.isalnum() else " " for c in s.lower()).split())
    phrase, text = clean(phrase), clean(text)
    if not phrase or not text:
        return False
    if phrase in text:
        return True
    if not fuzzy or len(text.split()) != len(phrase.split()):
        return False
    return edit_distance(phrase, text) <= 2


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--tesseract", default="tesseract")
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--data", type=Path,
                        help="Data directory; must not already hold the collection name.")
    parser.add_argument("--name", default="Live pack acceptance")
    parser.add_argument("--pack", default="Simply Optimized",
                        help="Catalogue search phrase; the top result's modpack is the one installed.")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    if not os.environ.get("DISPLAY"):
        parser.error("Set DISPLAY to a private ready X11 server.")
    with tempfile.TemporaryDirectory(prefix="palantir-pack-live-") as temp:
        root = Path(temp)
        evidence = args.evidence.resolve() if args.evidence else root / "evidence"
        evidence.mkdir(parents=True, exist_ok=True)
        data = args.data.resolve() if args.data else root / "data"
        data.mkdir(parents=True, exist_ok=True)
        window = None
        preexisting = set(command("xdotool", "search", "--name", "PalantirMC").split()) \
            if subprocess.run(["xdotool", "search", "--name", "PalantirMC"],
                              stdout=subprocess.DEVNULL,
                              stderr=subprocess.DEVNULL).returncode == 0 else set()
        log = (evidence / "launcher.log").open("w")
        app = subprocess.Popen([str(binary)], env={**os.environ, "PALANTIR_DATA_DIR": str(data)},
                               stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline and window is None:
                if app.poll() is not None:
                    raise RuntimeError("Launcher exited before its window appeared.")
                for probe in (["xdotool", "search", "--pid", str(app.pid)],
                              ["xdotool", "search", "--name", "PalantirMC"]):
                    found = subprocess.run(probe, text=True, stdout=subprocess.PIPE,
                                           stderr=subprocess.DEVNULL)
                    fresh = [w for w in found.stdout.split() if w not in preexisting]
                    if found.returncode == 0 and fresh:
                        window = fresh[-1]
                        break
                if window is None:
                    time.sleep(.1)
            if window is None:
                raise RuntimeError("Launcher window did not appear within 15 seconds.")
            focus_deadline = time.monotonic() + 8
            while subprocess.run(["xdotool", "windowfocus", window],
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode != 0:
                if time.monotonic() > focus_deadline:
                    raise RuntimeError("Launcher window never accepted focus.")
                time.sleep(.2)
            # A taller window beats wheel sweeps: the pack flow's controls
            # run well past the default 820px fold (the first release card's
            # "Select release" lands around y=860), and a resize relayouts
            # the whole page deterministically while wheel events depend on
            # where the pointer happens to sit.
            command("xdotool", "windowsize", window, "1280", "1500")
            time.sleep(1.5)
            scale = 4

            def words(name="current"):
                """Six observations of the window, kept as separate passes.

                A word only has to be read once anywhere, but multi-word
                phrases must be matched *within* one pass: each pass emits
                reading order, where a phrase's tokens are contiguous, while
                merging passes first would interleave duplicate tokens and
                break every phrase. The two extra passes read the top strip
                at 6x: the search row's placeholder is mid-gray on dark and
                the 4x whole-window pass drops it often enough to strand the
                flow (measured: "Name, idea, or something unexpected" reads
                clean at 6x where 4x misses it entirely).
                """
                def observe(source, extra, magnify, tag):
                    scaled = source.parent / f"{source.stem}-{tag}.png"
                    command("convert", str(source), "-resize", f"{magnify * 100}%",
                            "-colorspace", "Gray", str(scaled))
                    output = command(args.tesseract, str(scaled), "stdout", *extra, "tsv")
                    entries = []
                    for line in output.splitlines()[1:]:
                        fields = line.split("\t", 11)
                        if len(fields) != 12:
                            continue
                        text = fields[11].strip()
                        if text:
                            entries.append({"left": str(int(fields[6]) // magnify),
                                            "top": str(int(fields[7]) // magnify),
                                            "width": str(int(fields[8]) // magnify),
                                            "height": str(int(fields[9]) // magnify),
                                            "text": text})
                    return entries

                # Settle before observing: on a WM-less X server the capture
                # can race the app's tiled repaint and come back stale (or
                # blank), which reads as a control that "does not exist".
                first = evidence / f"{name}-first.png"
                path = evidence / f"{name}.png"
                command("import", "-window", window, str(first))
                time.sleep(.5)
                command("import", "-window", window, str(path))
                top = evidence / f"{name}-top.png"
                # The crop starts at the window's origin, so the top passes'
                # boxes are already in window coordinates.
                command("convert", str(path), "-crop", "1280x240+0+0", "+repage", str(top))
                # The psm 6 passes are what read the accent-filled primary
                # buttons: their 14px labels drop out of psm 3 and psm 11
                # entirely (measured: "Add to a collection..." at x=848
                # y=321 reads clean at 4x/psm 6, absent from both others).
                return [observe(first, [], scale, "a"),
                        observe(first, ["--psm", "11"], scale, "b"),
                        observe(first, ["--psm", "6"], scale, "e"),
                        observe(path, [], scale, "c"),
                        observe(path, ["--psm", "11"], scale, "d"),
                        observe(path, ["--psm", "6"], scale, "f"),
                        observe(top, ["--psm", "6"], 6, "t6a"),
                        observe(top, ["--psm", "11"], 6, "t6b")]

            def locate(passes, word, below):
                """The control's click point, or None. Exact readings beat
                fuzzy ones; within a phase the *shortest* run wins, because
                containment can always extend a match rightwards ("Modpacks"
                also "matches" the run Modpacks Resource packs Shaders, whose
                smeared center lands between the two chips -- measured: the
                extended run clicked neither and the Mods filter stayed
                selected); among equal runs the topmost wins, so "Read more"
                is the top card's button whichever pass read it."""
                for fuzzy in (False, True):
                    for width in (1, 2, 3, 4, 5):
                        candidates = []
                        for entries in passes:
                            entries = [w for w in entries if int(w["top"]) >= below]
                            for start in range(len(entries) - width + 1):
                                run = entries[start:start + width]
                                if not ocr_match(word, " ".join(w["text"] for w in run), fuzzy):
                                    continue
                                x = sum(int(w["left"]) + int(w["width"]) // 2 for w in run) // len(run)
                                y = sum(int(w["top"]) + int(w["height"]) // 2 for w in run) // len(run)
                                candidates.append((y, x))
                        if candidates:
                            candidates.sort()
                            return candidates[0]
                return None

            def click(word, below=100, within=24):
                # Observe the control before clicking; never treat an assumed
                # coordinate as proof of behavior. A miss sweeps the viewport
                # instead of clicking whatever sits under the cursor. The top
                # navigation band is excluded: its labels shadow content
                # words otherwise.
                deadline = time.monotonic() + within
                attempt = 0
                passes = []
                while time.monotonic() < deadline:
                    passes = words()
                    found = locate(passes, word, below)
                    if found:
                        y, x = found
                        command("xdotool", "mousemove", str(x), str(y), "click", "1")
                        time.sleep(.8)
                        return
                    attempt += 1
                    if attempt > 2:
                        # One direction only: in this flow a missed control is
                        # below the fold, and the earlier down-then-up
                        # oscillation netted to no travel at all.
                        command("xdotool", "mousemove", "800", "550", "click", "5")
                    time.sleep(.2)
                seen = " ".join(w["text"] for entries in passes for w in entries)
                raise AssertionError(
                    f"Control {word!r} not found in actual window; saw {seen!r}")

            def fill(text):
                command("xdotool", "key", "ctrl+a")
                command("xdotool", "type", "--clearmodifiers", str(text))
                time.sleep(.8)

            def wait_text(phrase, within, name="wait"):
                # Multi-word phrases are found the same way as for a click:
                # per-pass runs, never per-token (a token holds one word).
                deadline = time.monotonic() + within
                while time.monotonic() < deadline:
                    if locate(words(name), phrase, 0) is not None:
                        return True
                    time.sleep(2)
                return False

            # 1 / Navigate to Explore. Ctrl+3 is the app's own shortcut: the
            # rail's "Explore" label shares its words with the Home screen's
            # "Explore additions", and a keyboard route cannot land on the
            # wrong one.
            command("xdotool", "key", "ctrl+3")
            time.sleep(1.5)

            # 2 / Search the live catalogue for the pack. The input is
            # clicked before anything that can scroll the page, and the type
            # chips default to Mods -- a modpack query under it legitimately
            # answers "No matching additions" -- so the Modpacks filter is
            # part of the route, not a workaround.
            click("Name, idea, or something unexpected", within=30)
            fill(args.pack)
            click("Modpacks", within=30)
            time.sleep(4)

            # 3 / Open the top result and start the pack flow. A card opens
            # through its "Read more" action; the title is not a control.
            click("Read more", within=60)
            click("Add to a collection", within=45)

            # 4 / Choose a release, then configure the new collection.
            click("Review pack releases", within=30)
            click("Select release", within=30)
            click("Configure new pack collection", within=30)
            click("Collection name", within=30)
            fill(args.name)

            # 5 / Download the archive for review. This fetches and verifies
            # the .mrpack only; the indexed game content comes with install.
            click("Download selected pack for review", within=30)
            if not wait_text("Install the pack archive", 180, "review"):
                raise AssertionError("The pack review never offered installation.")

            # 6 / Install: reads the archive's own index into the new
            # collection and downloads its indexed files.
            click("Install the pack archive", within=30)
            if not wait_text("Installed the pack as", 1500, "receipt"):
                raise AssertionError("The pack install never reported its receipt.")

            # 7 / The acceptance is on disk: a brand-new collection holding
            # the pack's own files, not a merge into anything existing.
            instances = data / "instances"
            found = None
            for entry in sorted(instances.iterdir()) if instances.exists() else []:
                descriptor = entry / "instance.json"
                if not descriptor.is_file():
                    continue
                item = json.loads(descriptor.read_text())
                if item.get("name") == args.name:
                    found = (entry, item)
            if found is None:
                raise AssertionError(f"No collection named {args.name!r} under {instances}.")
            entry, item = found
            placed = [p for p in entry.rglob("*") if p.is_file()]
            game_files = [p for p in (entry / "game").rglob("*") if p.is_file()] \
                if (entry / "game").exists() else []
            if not game_files:
                raise AssertionError(
                    f"Collection {entry} exists but its game directory holds no pack files.")
            print(f"collection: {item['id']} / {item['name']} / "
                  f"{item.get('game_version')} {item.get('loader')}")
            print(f"files on disk: {len(placed)} ({len(game_files)} under game/)")
            for sample in sorted(game_files)[:10]:
                print(f"  {sample.relative_to(entry)}")
            print(f"evidence: {evidence}")
            print("PASS: a live catalogue modpack reviewed, downloaded and installed "
                  "into a brand-new collection with its indexed files on disk")
        finally:
            app.terminate()
            try:
                app.wait(timeout=10)
            except subprocess.TimeoutExpired:
                app.kill()


if __name__ == "__main__":
    main()
