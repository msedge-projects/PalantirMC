#!/usr/bin/env python3
"""Drive the real desktop with xdotool; verify artifacts, not just screenshots.

Run from rewrite/: python3 tools/ui_smoke.py --binary target/debug/PalantirMC
Requires a private, ready X11 DISPLAY, xdotool, ImageMagick import and tesseract.
The caller owns the display; the script owns only its launcher child/temp data.
Never point this at the owner's working display -- windows and keystrokes land
wherever DISPLAY says. Start a private Xvfb on a display number the owner is
not using (setsid nohup Xvfb :103 -screen 0 1920x1080x24 -nolisten tcp) and
run with DISPLAY=:103.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import zipfile


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

    OCR garbles short words ("Resolve" reads "Recnive"), so a control must be
    findable by its real label. The fuzzy allowance is one word-count-exact
    window within two character edits -- never a window of a different word
    count, and never more: two edits covers one or two garbled letters
    ("Recnive") while too few for one phrase to pose as another (body copy
    "it for launches" stays three edits from the "Use for launches" button).
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
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    if not os.environ.get("DISPLAY"):
        parser.error("Set DISPLAY to a private ready X11 server.")
    with tempfile.TemporaryDirectory(prefix="palantir-ui-smoke-") as temp:
        root = Path(temp)
        evidence = args.evidence.resolve() if args.evidence else root / "evidence"
        evidence.mkdir(parents=True, exist_ok=True)
        instance = root / "data/instances/smoke"
        game = instance / "game"
        world = game / "saves/Smoke world"
        (world / "region").mkdir(parents=True)
        original = {"level.dat": b"unchanged smoke metadata",
                    "region/r.0.0.mca": b"opaque smoke region"}
        for name, content in original.items():
            (world / name).write_bytes(content)
        (instance / "instance.json").write_text(json.dumps({
            "id": "smoke", "name": "Smoke collection", "game_version": "1.20.1",
            "loader": "Fabric", "loader_version": None, "installed_version": None,
        }))
        resource = root / "smoke-resource.zip"
        with zipfile.ZipFile(resource, "w") as archive:
            archive.writestr("pack.mcmeta", '{"pack":{"pack_format":15,"description":"Smoke"}}')
        window = None
        # Windows named like the launcher that predate this run belong to
        # other apps on the caller's display; the child's own window must be
        # told apart from them, never by creation order.
        preexisting = set(command("xdotool", "search", "--name", "PalantirMC").split()) \
            if subprocess.run(["xdotool", "search", "--name", "PalantirMC"],
                              stdout=subprocess.DEVNULL,
                              stderr=subprocess.DEVNULL).returncode == 0 else set()
        log = (evidence / "launcher.log").open("w")
        app = subprocess.Popen([str(binary)], env={**os.environ, "PALANTIR_DATA_DIR": str(root / "data")}, stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline and window is None:
                if app.poll() is not None:
                    raise RuntimeError("Launcher exited before its window appeared.")
                # _NET_WM_PID is not reliable on every X server; the title is
                # the fallback (the caller owns a private display).
                for probe in (["xdotool", "search", "--pid", str(app.pid)],
                              ["xdotool", "search", "--name", "PalantirMC"]):
                    found = subprocess.run(probe, text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
                    fresh = [w for w in found.stdout.split() if w not in preexisting]
                    if found.returncode == 0 and fresh:
                        window = fresh[-1]
                        break
                if window is None:
                    time.sleep(.1)
            if window is None:
                raise RuntimeError("Launcher window did not appear within 15 seconds.")
            # Focus can race the window becoming viewable on a WM-less display.
            focus_deadline = time.monotonic() + 8
            while subprocess.run(["xdotool", "windowfocus", window], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode != 0:
                if time.monotonic() > focus_deadline:
                    raise RuntimeError("Launcher window never accepted focus.")
                time.sleep(.2)
            time.sleep(.8)

            # Magnification for OCR. 14px labels -- primary buttons especially
            # -- sit below tesseract's comfortable read size; 3x still garbled
            # short button words ("Actions" into "ae"), 4x reads them.
            scale = 4

            def words(name="current"):
                # Read a magnified copy of a capture and scale the observed
                # boxes back: the observation is still of the real window,
                # just legible.
                def observe(path, extra):
                    scaled = path.parent / f"{path.stem}-{scale}x.png"
                    # Grayscale after magnification -- never -normalize. On
                    # this dark UI, normalization amplifies whatever the
                    # modal scrim lets through and tesseract reads the
                    # window behind the dialog instead of it (measured on a
                    # real capture: 0 of the dialog's words with
                    # -normalize, 7 without it).
                    command("convert", str(path), "-resize", f"{scale * 100}%",
                            "-colorspace", "Gray", str(scaled))
                    output = command(args.tesseract, str(scaled), "stdout", *extra, "tsv")
                    entries = []
                    # Split by hand rather than csv: OCR text may start with a
                    # quote character, and csv would then swallow following rows
                    # into one field. Text is the final, tab-free column.
                    for line in output.splitlines()[1:]:
                        fields = line.split("\t", 11)
                        if len(fields) != 12:
                            continue
                        text = fields[11].strip()
                        if text:
                            entries.append({"left": str(int(fields[6]) // scale),
                                            "top": str(int(fields[7]) // scale),
                                            "width": str(int(fields[8]) // scale),
                                            "height": str(int(fields[9]) // scale),
                                            "text": text})
                    return entries

                # Settle before observing: on a WM-less X server the capture
                # can race the app's tiled repaint and come back with a
                # stale band (or blank), which reads as a control that
                # "does not exist". Capture, let the paint finish, capture
                # again.
                first = evidence / f"{name}-first.png"
                path = evidence / f"{name}.png"
                command("import", "-window", window, str(first))
                time.sleep(.5)
                command("import", "-window", window, str(path))
                # Union of several observations of the same window: no single
                # OCR pass reads every word (short faint words drop at random
                # per frame, which is what made a control "not exist"), and
                # the default page pass and the sparse pass mis-segment
                # different words (the settings rail's "Features" comes out
                # of the default pass as "ean eee"). Two grabs x two passes,
                # all unioned: a word only has to be read once anywhere.
                return (observe(first, [])
                        + observe(first, ["--psm", "11"])
                        + observe(path, [])
                        + observe(path, ["--psm", "11"]))

            def click(word, below=100):
                # Observe the control before clicking; never treat an assumed
                # coordinate or successful input command as proof of behavior.
                # The top navigation band is excluded: this script drives the
                # content controls, and its labels shadow tab names otherwise.
                # OCR splits multi-word labels into per-word tokens, so match
                # the phrase over the tightest run of adjacent tokens. Runs go
                # up to five tokens: button labels like "Add a Java
                # executable" are that long. Matching is two-phase: an exact
                # reading always wins over a fuzzy one (see ocr_match), or the
                # "Saved preferences" status line would shadow its own
                # "Save preferences" button.
                deadline = time.monotonic() + 24
                attempt = 0
                while time.monotonic() < deadline:
                    entries = [w for w in words() if int(w["top"]) >= below]
                    for fuzzy in (False, True):
                        for width in (1, 2, 3, 4, 5):
                            for start in range(len(entries) - width + 1):
                                run = entries[start:start + width]
                                if not ocr_match(word, " ".join(w["text"] for w in run), fuzzy):
                                    continue
                                x = sum(int(w["left"]) + int(w["width"]) // 2 for w in run) // len(run)
                                y = sum(int(w["top"]) + int(w["height"]) // 2 for w in run) // len(run)
                                command("xdotool", "mousemove", str(x), str(y), "click", "1")
                                time.sleep(.8)
                                return
                    # A miss is one bad OCR frame or content outside the
                    # fold. Settle first (a bad frame corrects itself), then
                    # sweep: several notches one way before flipping, so
                    # recovery travels instead of oscillating in place. The
                    # click point stays inside the window.
                    attempt += 1
                    if attempt > 2:
                        sweep = attempt - 2
                        wheel = "5" if (sweep // 4) % 2 == 0 else "4"
                        command("xdotool", "mousemove", "800", "550", "click", wheel)
                    time.sleep(.2)
                raise AssertionError(
                    f"Control {word!r} not found in actual window; saw {' '.join(w['text'] for w in entries)!r}")

            def fill(text):
                command("xdotool", "key", "ctrl+a")
                command("xdotool", "type", "--clearmodifiers", str(text))
                time.sleep(.8)

            def activate(steps=1):
                # Keyboard traversal reaches the primary action, even one
                # below the window edge. Focus moves asynchronously in the
                # toolkit, so every activation key needs its own beat or it
                # lands on the previous control.
                for _ in range(steps):
                    command("xdotool", "key", "Tab")
                    time.sleep(.5)
                command("xdotool", "key", "Return")
                time.sleep(.8)

            def assert_text(expected, name):
                # Receipts appear when the worker finishes; wait for them.
                # Text is unioned across observations: a word dropped by one
                # frame's OCR is still seen once any frame reads it.
                deadline = time.monotonic() + 14
                seen = set()
                while time.monotonic() < deadline:
                    seen.update(w["text"] for w in words(name))
                    if expected.lower() in " ".join(seen).lower():
                        return
                    time.sleep(.2)
                raise AssertionError((expected, " ".join(seen)))

            # Locate the collection card by its name, then click that card's
            # own Inspect control — observed in the actual window, never an
            # assumed coordinate. One capture is not an observation: the home
            # pane paints after the window appears -- and the worker's
            # instance list lands a moment later -- so look until it is there.
            deadline = time.monotonic() + 25
            while True:
                entries = words("home")
                found = [w for w in entries if "smoke" in w["text"].lower()]
                # An empty capture is an unpainted frame, not an empty page;
                # it simply earns another look until the deadline.
                if found or time.monotonic() > deadline:
                    break
                time.sleep(.2)
            if not found:
                raise AssertionError(
                    "No 'Smoke collection' card in the home pane; saw "
                    + " ".join(w["text"] for w in entries))
            smoke = found[0]
            inspect = next(w for w in entries
                           if w["text"].lower().startswith("inspect")
                           and abs(int(w["top"]) - int(smoke["top"])) < 40)
            x = int(inspect["left"]) + int(inspect["width"]) // 2
            y = int(inspect["top"]) + int(inspect["height"]) // 2
            command("xdotool", "mousemove", str(x), str(y), "click", "1")
            time.sleep(.8)
            click("Worlds")
            click("Backup")
            # The destination field is located through its visible placeholder.
            click("Absolute path")
            output = root / "smoke-world.zip"
            fill(output)
            # Primary-button labels may be missed by OCR at 14px; keyboard
            # traversal also verifies the user-facing activation interface.
            activate()
            # The button reads "Resolve and review files"; at 14px OCR turns
            # "Resolve" into "Recnive" with the short word the least stable
            # part, so observe the reliably read half of the same label.
            click("and review")
            click("closed")
            activate()
            deadline = time.monotonic() + 8
            while not output.exists() and time.monotonic() < deadline:
                time.sleep(.1)
            with zipfile.ZipFile(output) as archive:
                assert sorted(archive.namelist()) == sorted(original)
                for name, content in original.items():
                    assert archive.read(name) == content
                    assert (world / name).read_bytes() == content
            assert_text("Saved", "world-success")
            before = output.read_bytes()
            click("and review")
            assert_text("already exists", "overwrite-refused")
            assert output.read_bytes() == before
            command("xdotool", "key", "Escape", "F1")
            time.sleep(.8)
            assert_text("Keyboard", "keyboard-help")
            command("xdotool", "key", "Escape")
            time.sleep(.8)
            click("Content")
            click("Import local")
            # The content-type control is a selector: open it and use its own
            # keyboard contract to choose "Resource pack" (Mod, Resource, Shader).
            click("Mod")
            command("xdotool", "key", "Down")
            time.sleep(.3)
            command("xdotool", "key", "Return")
            time.sleep(.5)
            # The source field is located through its visible placeholder.
            click("jar or")
            fill(resource)
            activate()
            click("and review")
            command("xdotool", "windowsize", window, "840", "600")
            time.sleep(.8)
            click("closed")
            activate()
            target = game / "resourcepacks" / resource.name
            deadline = time.monotonic() + 8
            while not target.exists() and time.monotonic() < deadline:
                time.sleep(.1)
            assert target.read_bytes() == resource.read_bytes()
            assert_text("Imported", "small-import-success")

            # --- the settings surfaces: Window, Features, launch settings
            # The import review dialog stays open on its receipt; close it
            # before the app will take a global keyboard shortcut again.
            command("xdotool", "key", "Escape")
            time.sleep(.5)
            # The settings dialog is taller than the small window, and the
            # Java roster and Manage danger zone put their actions near the
            # bottom of the pane: give the window room so those controls sit
            # clearly inside the capture rather than on its clip edge.
            command("xdotool", "windowsize", window, "1280", "1000")
            time.sleep(.8)
            # Preferences opens on its own keyboard contract (Ctrl+,).
            command("xdotool", "key", "ctrl+comma")
            time.sleep(.8)
            # The settings tab list sits over the collection page; single
            # labels are unique in this state and land on the tab itself.
            click("Window")
            # The width field is located through its visible placeholder;
            # after it holds a value the remaining placeholder is the height.
            click("Unset")
            fill("854")
            click("Unset")
            fill("480")
            click("Save preferences")
            assert_text("Saved", "window-settings-saved")
            # A half-filled pair must be refused, not saved: clear the width
            # and try again with only the height set.
            click("854")
            command("xdotool", "key", "ctrl+a")
            command("xdotool", "key", "Delete")
            time.sleep(.3)
            click("Save preferences")
            # The refusal notice's own words, not the pane's standing copy.
            assert_text("width and height", "window-half-refused")
            # Put the pair back complete before leaving the pane.
            click("Unset")
            fill("854")
            click("Save preferences")
            assert_text("Saved", "window-settings-restored")

            # A runtime that is not there is refused by name before it can
            # join the roster.
            click("Java")
            click("Add a Java executable")
            fill("/nowhere/java")
            activate()
            assert_text("is gone", "java-probe-refused")
            # A runtime that answers joins the roster under the version line
            # it reported, and removing it empties the list again.
            mock = root / "mock-java"
            mock.write_text("#!/bin/sh\necho 'openjdk 21.0.3 smoke-mock' >&2\n")
            mock.chmod(0o755)
            click("Add a Java executable")
            fill(str(mock))
            activate()
            assert_text("smoke-mock", "java-roster-added")
            click("Use for launches")
            assert_text("selected for launches", "java-roster-selected")
            click("Remove from list")
            assert_text("removed from the list", "java-roster-removed")
            assert_text("No runtimes added yet", "java-roster-empty")

            # Features hides a collection tab everywhere.
            click("Behaviour")
            click("Features")
            click("Worlds tab")
            click("Save preferences")
            assert_text("Saved", "features-saved")
            command("xdotool", "key", "Escape")
            time.sleep(.5)
            # The same band exclusion the clicks use: the chrome's own words
            # are not the collection page's tab strip.
            entries = [w["text"].lower() for w in words("features-hidden")
                       if int(w["top"]) >= 100]
            assert "worlds" not in entries, f"hidden tab still visible: {entries}"

            # The launch-settings dialog saves and restores its override.
            click("Manage")
            click("Set memory")
            # The dialog focuses its first field on open (the same convention
            # the rename and account dialogs get); typing goes there. Two
            # fields share the same placeholder here, so a placeholder click
            # is the ambiguous move, not the safe one.
            fill("2048")
            # Seven fields stand between the first one and the save action;
            # traversal crosses each of them to reach it. The proof is the
            # override the collection page then shows -- an artifact of the
            # save -- not pixels inside an input field.
            activate(7)
            assert_text("Launch settings saved", "runtime-saved")
            assert_text("2048 MiB memory", "runtime-stored")
            # Reopen and clear it: the prefilled value is the memory field,
            # and with it blank the launcher preference rules again.
            click("Set memory")
            command("xdotool", "key", "ctrl+a")
            command("xdotool", "key", "Delete")
            time.sleep(.3)
            activate(7)
            assert_text("Launch settings saved", "runtime-cleared")
            assert_text("Follows the launcher", "runtime-cleared-shown")

            # Duplicate: the dialog prefills the copy's name and focuses it.
            click("Duplicate")
            activate()
            assert_text("Duplicated", "duplicate-receipt")

            # The actions menu: the complete per-collection set, every entry a
            # keyboard-reachable button in a dialog. On the Overview tab the
            # danger entry is unique to the menu, so the click cannot land on
            # the Manage tab's own copy behind the scrim.
            click("Overview")
            click("Actions")
            assert_text("Collection actions", "actions-menu-open")
            assert_text("Open collection", "actions-menu-open-entry")
            assert_text("Back up or export", "actions-menu-export-entry")
            assert_text("Delete collection", "actions-menu-delete-entry")
            click("Delete collection")
            # Delete confirms first and names what goes away.
            assert_text("This removes a whole collection", "delete-confirm")
            assert_text("will be deleted", "delete-confirm-names")
            command("xdotool", "key", "Escape")
            time.sleep(.5)

            # Turning the tab back on restores it exactly.
            command("xdotool", "key", "ctrl+comma")
            time.sleep(.8)
            click("Behaviour")
            click("Features")
            click("Worlds tab")
            click("Save preferences")
            assert_text("Saved", "features-restored")
            command("xdotool", "key", "Escape")
            time.sleep(.5)
            click("Worlds")

            # The deletion receipt renders inside the open dialog, not as a
            # banner behind the scrim. This is the last step: it consumes the
            # fixture collection, so nothing after it may need one.
            click("Manage")
            click("Delete collection")
            assert_text("This removes a whole collection", "delete-confirm-final")
            click("Delete permanently")
            # "Collection deleted" is the dialog's own header for the receipt
            # state; the banner alone could not say it.
            assert_text("Collection deleted", "delete-receipt-in-dialog")
            assert_text("Instance deleted", "delete-receipt-text")
            print("PASS: actual window world export, unchanged source, overwrite refusal, keyboard help/activation, 840x600 byte-exact local import, the Window/Features/launch-settings dialogs, duplicate-with-name, the Java roster (refusal by name, add, select, remove), the per-collection actions menu with its named delete confirmation, and the deletion receipt rendered inside its open dialog")
        finally:
            if app.poll() is None:
                app.terminate()
                try:
                    app.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait()
            log.close()


if __name__ == "__main__":
    main()
