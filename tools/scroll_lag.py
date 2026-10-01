#!/usr/bin/env python3
"""How far a scroll moves between two frames, read off a recording.

Why this exists: "the scrolling feels laggy" is not a measurement, and the
difference between a glide and a teleport is not a matter of degree -- a glide
moves over several frames with intermediate offsets, a teleport moves once and
then sits still. A screen recording has both, at 60 frames a second, and all this
tool does is read that out of it: one number per frame transition, plus the
summary a comparison needs (how many transitions moved, how far, and how long the
runs of consecutive moving frames were).

How a burst is made. Nothing here decodes video: `ffmpeg` does that, and the two
commands below are the ones the two recordings in `NOTES.md` §31 were measured
with. `-ss` selects the three seconds around the gesture, `fps=60` keeps every
drawn frame, and `format=gray` plus the `.pgm` extension get P5 frames, which are
readable without an image library -- which is the whole reason this file has no
dependencies. The two bursts land in different directories because the frames
are one client each:

    ffmpeg -hide_banner -loglevel error -ss 44 -t 3 -i official.mp4 \\
        -vf "fps=60,scale=320:180,format=gray" -f image2 burst_o/f_%04d.pgm
    ffmpeg -hide_banner -loglevel error -ss 19 -t 3 -i ours.mp4 \\
        -vf "fps=60,scale=320:180,format=gray" -f image2 burst_p/f_%04d.pgm
    python tools/scroll_lag.py burst_o burst_p

What it prints, and how to read it. `moving frames` is how much of the window
was spent moving at all; `mean |d| while moving` is how far an average moving
frame went; and `motion runs` is the shape that matters. A run of two to six
consecutive moving frames, with values like `-10, -6, 0, -5, -2, -2, -3` between
them, is a flick easing; fourteen runs of exactly one frame, each surrounded by
zeros, is fourteen wheel events teleporting. The reference and this launcher came
out that way around, which is how the scroll demotion in `crate::scroll` was
found (`GATES.md` G140).

What it cannot say. A row-mean profile is a vertical signal, so this measures
vertical movement only, and it is blind to anything that moves less than a pixel
of the 320-pixel-wide reduction. Two recordings are only comparable if both were
taken at the same size and frame rate, which is why the commands above fix
`scale` and rely on `fps` rather than on the file's own rate. And a burst that
contains no wheel at all prints the honest answer for it: no motion detected.
"""

import os
import sys

# The largest shift the correlation will consider, in reduced pixels. A wheel
# notch is 60 px at full size, which is 15 here; 40 is the whole height of the
# reduction, so anything past it is a different screen rather than a scroll.
LIMIT = 40


def read_pgm(path):
    """One P5 frame's row-mean profile: the brightness of each row, top first."""
    with open(path, "rb") as f:
        data = f.read()
    # P5 header: "P5\n<w> <h>\n<maxval>\n" and then width*height bytes.
    parts = data.split(b"\n", 3)
    if parts[0].strip() != b"P5":
        raise SystemExit(f"not a P5 pgm: {path}")
    w, h = map(int, parts[1].split())
    maxval = int(parts[2])
    if maxval > 255:
        raise SystemExit("16-bit pgm not supported")
    body = parts[3]
    rows = []
    for y in range(h):
        row = body[y * w:(y + 1) * w]
        rows.append(sum(row) / w)
    return rows


def best_shift(a, b, limit=LIMIT):
    """The shift `s` minimizing the difference between a[y] and b[y + s].

    `s` is how far the picture moved between the two frames, signed the same way
    on every frame of a burst, so a run of them is a direction. A count guard
    keeps the comparison from being decided by the rows that fell off one end:
    a shift that only overlaps a third of the height is not the answer.
    """
    best_s, best_err = 0, None
    n = len(a)
    for s in range(-limit, limit + 1):
        err = 0.0
        count = 0
        for y in range(n):
            yy = y + s
            if 0 <= yy < n:
                err += (a[y] - b[yy]) ** 2
                count += 1
        if count > n // 2:
            err /= count
            if best_err is None or err < best_err:
                best_err, best_s = err, s
    return best_s


def analyze(directory):
    """Print one burst's displacement series and the summary of it."""
    files = sorted(f for f in os.listdir(directory) if f.endswith(".pgm"))
    profiles = [read_pgm(os.path.join(directory, f)) for f in files]
    shifts = []
    for i in range(1, len(profiles)):
        shifts.append(best_shift(profiles[i - 1], profiles[i]))
    moving = [s for s in shifts if s != 0]
    jumps = [abs(s) for s in moving]
    print(f"== {directory} ==")
    print(f"frames: {len(profiles)}  transitions: {len(shifts)}")
    print(f"displacement series (px/frame): {shifts}")
    if not moving:
        print("no motion detected in this window\n")
        return
    print(
        f"moving frames: {len(moving)} ({100 * len(moving) / len(shifts):.0f}%)  "
        f"mean |d| while moving: {sum(jumps) / len(jumps):.2f}px  "
        f"max |d|: {max(jumps)}px"
    )
    # The two ends of the same distribution: a flick that eases is made of these.
    gentle = sum(1 for j in jumps if j <= 6)
    big = sum(1 for j in jumps if j >= 15)
    print(f"gentle steps (<=6px): {gentle}  large jumps (>=15px): {big}")
    # The shape: a glide is one long run, a teleport is a run of one.
    runs, cur = [], 0
    for s in shifts:
        if s != 0:
            cur += 1
        elif cur:
            runs.append(cur)
            cur = 0
    if cur:
        runs.append(cur)
    print(f"motion runs (consecutive moving frames): {runs}\n")


def main(argv):
    if len(argv) < 2:
        print(__doc__.split("\n\n")[1])
        return 2
    for directory in argv[1:]:
        analyze(directory)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
