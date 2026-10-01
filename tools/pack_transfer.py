#!/usr/bin/env python3
"""Pack the evidence this tree's numbers came from into one archive.

Why this exists: the tree moves by `git clone`, because both remotes carry every
commit, the vendored reference and the fonts. What does *not* travel that way is the
evidence — two screen recordings and the frames derived from them, none of which is
committed (AGENTS.md forbids committing captures). This puts all of it in one file
with a manifest and a digest, so the machine on the other side has the same
measurements rather than a description of them.

    python tools/pack_transfer.py                 # -> transfer/palantirmc-evidence-<commit>.tar.gz
    python tools/pack_transfer.py --recordings ~/oCam

What it packs, and why each one is worth its bytes:

* **the two recordings**, in full. They are the only irreplaceable evidence here:
  every derived frame can be rebuilt from them with the `ffmpeg` commands in
  `tools/scroll_lag.py`'s docstring, and they cannot be rebuilt from anything.
* **the derived frames**: `vidcmp_work/` (the 22 matched full-resolution stills and
  the two 180-frame bursts the scroll measurement correlated) and `.scratch/vidcmp/`
  (the 1 fps stills and contact sheets). Rebuilding these costs minutes and needs the
  recordings anyway; carrying them costs about thirty megabytes.
* **a manifest** naming the commit being packed, every file's size, which recording
  is which client, and the commands that rebuild the bursts.

`vidcmp_work/scroll_lag.py` is deliberately *not* packed: that script was promoted
into the tree as `tools/scroll_lag.py`, and a second copy of it would be a second
thing to keep in step.
"""

import argparse
import hashlib
import io
import os
import subprocess
import tarfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

# The two recordings the measurement read, named rather than globbed: the same
# directory holds several gigabytes of unrelated recordings, and a directory listing
# is not evidence about which client is which. The mapping is the report's own, and
# it was confirmed by sight — the reference's chrome reads `modrinth`, this
# launcher's reads `PalantirMC`.
RECORDINGS = (
    ("Record_2026_10_01_20_41_35_635.mp4", "reference", "the Modrinth App, 86.8 s, 5202 frames"),
    ("Record_2026_10_01_20_44_08_212.mp4", "palantir", "this launcher, 41.0 s, 2458 frames"),
)

# Derived frames, as whole directories, with the name each keeps inside the archive.
FRAMES = (
    ("vidcmp_work", "vidcmp_work"),
    (".scratch/vidcmp", "scratch-vidcmp"),
)

SKIP = {"__pycache__", "scroll_lag.py"}

REBUILD = """\
Every frame in this archive was derived from the two recordings with these two
commands (the times are the seconds each burst was taken from, and the fuller
account is in `tools/scroll_lag.py`'s own docstring):

  ffmpeg -hide_banner -loglevel error -ss 44 -t 3 -i <reference>.mp4 \\
      -vf "fps=60,scale=320:180,format=gray" -f image2 burst_o/f_%04d.pgm
  ffmpeg -hide_banner -loglevel error -ss 19 -t 3 -i <palantir>.mp4 \\
      -vf "fps=60,scale=320:180,format=gray" -f image2 burst_p/f_%04d.pgm
  python tools/scroll_lag.py burst_o burst_p

and the stills:

  ffmpeg -hide_banner -loglevel error -i <recording>.mp4 -vf fps=1,scale=854:-1 f_%03d.jpg
"""


def git(*args):
    """Answer a git question, or an empty string when git cannot."""
    done = subprocess.run(["git", *args], cwd=REPO, text=True, capture_output=True)
    return done.stdout.strip() if done.returncode == 0 else ""


def human(size):
    """A byte count as a person reads it."""
    value = float(size)
    for unit in ("B", "KB", "MB", "GB"):
        if value < 1024 or unit == "GB":
            return f"{int(value):,} B" if unit == "B" else f"{value:,.1f} {unit}"
        value /= 1024
    return f"{value:,.1f} GB"


def pack(archive, recordings_dir):
    """Write the archive and answer `(bytes, file count, missing)`."""
    commit = git("rev-parse", "--short", "HEAD") or "unknown"
    branch = git("rev-parse", "--abbrev-ref", "HEAD") or "unknown"
    dirty = bool(git("status", "--porcelain"))
    lines = [
        f"PalantirMC evidence, packed {time.strftime('%Y-%m-%d %H:%M:%S')}",
        f"commit      {commit} on {branch}" + (" (working tree dirty)" if dirty else ""),
        "",
        "recordings:",
    ]
    total = 0
    count = 0
    missing = []
    with tarfile.open(archive, "w:gz") as tar:
        for name, which, note in RECORDINGS:
            path = recordings_dir / name
            if not path.exists():
                missing.append(str(path))
                lines.append(f"  MISSING  {name}  ({which})")
                continue
            info = path.stat()
            total += info.st_size
            count += 1
            lines.append(f"  {info.st_size:>12,} B  recordings/{name}  ({which}: {note})")
            tar.add(path, arcname=f"recordings/{name}")
        lines.append("")
        lines.append("frames:")
        for source, arcname in FRAMES:
            root = REPO / source
            if not root.is_dir():
                missing.append(str(root))
                lines.append(f"  MISSING  {source}/")
                continue
            for path in sorted(root.rglob("*")):
                if not path.is_file() or path.name in SKIP:
                    continue
                if any(part in SKIP for part in path.parts):
                    continue
                size = path.stat().st_size
                total += size
                count += 1
                tar.add(path, arcname=f"{arcname}/{path.relative_to(root)}")
            lines.append(f"  {source}/ -> {arcname}/")
        lines.extend(["", REBUILD])
        manifest = ("\n".join(lines) + "\n").encode("utf-8")
        info = tarfile.TarInfo("MANIFEST.txt")
        info.size = len(manifest)
        info.mtime = int(time.time())
        tar.addfile(info, io.BytesIO(manifest))
    return total, count, missing


def digest(path):
    sha = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            sha.update(chunk)
    return sha.hexdigest()


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--recordings", default=os.path.expanduser("~/Documents/oCam"),
                        help="where the two recordings live (default ~/Documents/oCam)")
    parser.add_argument("--out", help="archive to write (default transfer/palantirmc-evidence-<commit>.tar.gz)")
    args = parser.parse_args()

    commit = git("rev-parse", "--short", "HEAD") or "unknown"
    out = Path(args.out) if args.out else REPO / "transfer" / f"palantirmc-evidence-{commit}.tar.gz"
    out.parent.mkdir(parents=True, exist_ok=True)

    total, count, missing = pack(out, Path(args.recordings).expanduser())
    packed = out.stat().st_size
    print(f"{out}")
    print(f"  {human(total)} of files ({count} of them) in {human(packed)} of archive")
    print(f"  sha256 {digest(out)}")
    if missing:
        print("  not found, so not packed:")
        for path in missing:
            print(f"    {path}")
        print("  pass --recordings DIR if they live somewhere else")
    print("  copy it, then verify: sha256sum " + out.name)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
