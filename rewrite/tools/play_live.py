"""Live acceptance: the real game launched from the desktop Play button.

Creates a vanilla collection through the actual window, presses Play with
offline identity, and asserts the game's own process appears and stays
running -- the desktop half of what tools and tests already prove at the
engine level (the loader suite's watched 20 s launches). Network required:
the first Play downloads the game, its libraries and the managed Java
runtime before the session starts.

Run from rewrite/: python3 tools/play_live.py --binary target/release/PalantirMC
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

    The same contract as tools/ui_smoke.py and tools/pack_live.py: one
    word-count-exact window within two character edits, never more.
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


def game_processes():
    """Processes that are the game itself: a java executable running a
    Minecraft main class.

    Both conditions are load bearing. A loose substring rule -- "java"
    anywhere plus this run's data path -- matched the *observer's own shell*
    (a poll command that merely echoed the word "java") and once passed a
    game that was never watched, so the acceptance must name java as argv0
    and a game main class among the tokens. Version documents name their
    own class: `net.minecraft.launchwrapper.Launch` for the 1.5.2 the live
    loader suite watches, `net.minecraft.client.main.Main` for modern ones."""
    listing = subprocess.run(["ps", "-eo", "args"], text=True,
                             stdout=subprocess.PIPE).stdout
    found = []
    for line in listing.splitlines():
        parts = line.split()
        if not parts or Path(parts[0]).name not in ("java", "javaw"):
            continue
        if any(token.startswith("net.minecraft.") or token == "Minecraft"
               for token in parts[1:]):
            found.append(line.strip())
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--tesseract", default="tesseract")
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--data", type=Path,
                        help="Data directory; must be fresh enough to hold no such collection.")
    parser.add_argument("--name", default="Real game acceptance")
    parser.add_argument("--game", default="1.5.2",
                        help="Game version to create; the live loader suite's watched vanilla.")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    if not os.environ.get("DISPLAY"):
        parser.error("Set DISPLAY to a private ready X11 server.")
    with tempfile.TemporaryDirectory(prefix="palantir-play-live-") as temp:
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
            command("xdotool", "windowsize", window, "1280", "1500")
            time.sleep(1.5)
            scale = 4

            def words(name="current"):
                """Eight observations kept as separate passes: phrases must be
                matched within one pass, where reading order keeps their
                tokens contiguous. The psm 6 passes read the accent-filled
                primary buttons (measured: 4x/psm 3 and 4x/psm 11 both miss
                \"Add to a collection...\"; 4x/psm 6 reads it clean), and the
                6x top passes read the faint search-row placeholder."""
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

                first = evidence / f"{name}-first.png"
                path = evidence / f"{name}.png"
                command("import", "-window", window, str(first))
                time.sleep(.5)
                command("import", "-window", window, str(path))
                top = evidence / f"{name}-top.png"
                command("convert", str(path), "-crop", "1280x240+0+0", "+repage", str(top))
                return [observe(first, [], scale, "a"),
                        observe(first, ["--psm", "11"], scale, "b"),
                        observe(first, ["--psm", "6"], scale, "e"),
                        observe(path, [], scale, "c"),
                        observe(path, ["--psm", "11"], scale, "d"),
                        observe(path, ["--psm", "6"], scale, "f"),
                        observe(top, ["--psm", "6"], 6, "t6a"),
                        observe(top, ["--psm", "11"], 6, "t6b")]

            def locate(passes, word, below):
                """Exact beats fuzzy; the shortest run beats a smeared longer
                one; the topmost match wins among equals."""
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
                deadline = time.monotonic() + within
                while time.monotonic() < deadline:
                    if locate(words(name), phrase, 0) is not None:
                        return True
                    time.sleep(2)
                return False

            # 1 / Create the collection. Ctrl+2 is the app's own shortcut to
            # Collections; the empty shelf offers "New collection" (the
            # populated header's equivalent reads "Create"), and the create
            # dialog wants a name and a version.
            command("xdotool", "key", "ctrl+2")
            time.sleep(1.5)
            click("New collection", within=30)
            # The dialog focuses its first input on open (the app's
            # "dialog-first" id), so the name types straight in -- the
            # placeholder "Quiet valley" is another faint label the
            # whole-page passes drop. Tab reaches the version input the same
            # deterministic way, and its prefilled default is replaced.
            fill(args.name)
            command("xdotool", "key", "Tab")
            time.sleep(.5)
            fill(args.game)
            click("Create collection", within=30)

            # 2 / Play. The card's own button opens the session panel, whose
            # offline identity is the launch that needs no account.
            if not wait_text("Installs on first play", 60, "card"):
                raise AssertionError("The new collection card never appeared.")
            click("Play offline", within=30)
            click("Play with offline identity", within=30)

            # 3 / The acceptance is a living game process: the engine
            # downloads the game, its libraries and the managed runtime
            # first, then spawns. A first Play can take minutes.
            deadline = time.monotonic() + 900
            while time.monotonic() < deadline:
                if game_processes():
                    break
                time.sleep(5)
            else:
                raise AssertionError("The game process never appeared after Play.")
            # 4 / Watched running, the same 20 s bar as the loader suite's
            # live launches: alive at the start and still alive at the end.
            watched = 0
            while watched < 20:
                time.sleep(5)
                watched += 5
                if not game_processes():
                    raise AssertionError(f"The game exited after {watched}s of the watched 20s.")
            processes = game_processes()
            command("import", "-window", "root", str(evidence / "game-screen.png"))
            instances = data / "instances"
            placed = [p for p in instances.rglob("*") if p.is_file()] \
                if instances.exists() else []
            item = None
            for entry in sorted(instances.iterdir()) if instances.exists() else []:
                descriptor = entry / "instance.json"
                if descriptor.is_file():
                    record = json.loads(descriptor.read_text())
                    if record.get("name") == args.name:
                        item = record
            if item is None:
                raise AssertionError(f"No collection named {args.name!r} under {instances}.")
            print(f"collection: {item['id']} / {item['name']} / "
                  f"{item.get('game_version')} {item.get('loader')}")
            for java in processes:
                print(f"game process: {java[:160]}")
            print(f"files on disk: {len(placed)}")
            print(f"evidence: {evidence}")
            print("PASS: a vanilla collection created in the actual window, played with "
                  "offline identity, with the real game process watched running for 20s")
        finally:
            app.terminate()
            try:
                app.wait(timeout=10)
            except subprocess.TimeoutExpired:
                app.kill()


if __name__ == "__main__":
    main()
