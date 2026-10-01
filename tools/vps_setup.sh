#!/usr/bin/env bash
# Bootstrap an Ubuntu 24 machine for this tree: the toolchain, a display with no
# screen, the tools that read it, and the reference client to compare against.
#
# Why a script rather than a list of commands in a document: the list is long,
# three of its entries are non-obvious, and a machine that is set up from prose is
# a machine nobody can set up twice. The non-obvious ones are named where they are
# installed -- `libxkbcommon-x11-dev` is what makes an iced window open at all,
# `openbox` is what makes an undecorated one map when there is no desktop to place
# it, and `x11-utils` is what gives `xshot.py` the geometry it captures.
#
# Run it with `bash tools/vps_setup.sh` on the machine being set up. It is
# idempotent: apt and rustup both are, the reference's package is reinstalled at
# the pinned version, and nothing is written outside `$HOME/Apps` (the downloaded
# package) and the two things it installs.
#
# The reference is *pinned* rather than latest on purpose. `vendor/modrinth-app`'s
# `UPSTREAM.md` pins the source at commit 8966b5e2, and the two recordings the
# scroll measurement came from were made on 2026-10-01, four days after v0.21.6
# shipped. Comparing this launcher's window against a different generation of
# theirs would answer a question nobody asked.
set -euo pipefail

REFERENCE_TAG="v0.21.6"
REFERENCE_REPO="modrinth/code"

say() { printf '\n== %s\n' "$*"; }

# `sudo` only when this is not already root: a VPS is often logged into as root,
# where `sudo` may not even be installed.
as_root() {
  if [ "$(id -u)" -eq 0 ]; then
    "$@"
  else
    sudo "$@"
  fi
}

say "packages"
as_root apt-get update -qq
as_root apt-get install -y --no-install-recommends \
  build-essential pkg-config cmake curl git ca-certificates python3 unzip \
  libxkbcommon-dev libxkbcommon-x11-dev libx11-dev libxcb1-dev libxcb-shm0-dev \
  libxcb-render0-dev libxcb-xfixes0-dev libwayland-dev libfontconfig1-dev \
  libfreetype-dev \
  xvfb x11-utils xdotool ffmpeg imagemagick openbox

say "rust toolchain"
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable --profile minimal
fi
# `cargo` on PATH for this shell, whichever way it got here.
if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi
# The workspace's own commands need both: `cargo test` needs neither, but
# `AGENTS.md`'s three commands include a clippy sweep and CI runs a fmt check.
rustup component add clippy rustfmt >/dev/null 2>&1 || true

say "the reference client ($REFERENCE_TAG)"
mkdir -p "$HOME/Apps"
cd "$HOME/Apps"
# The release JSON carries every platform's asset, and the `.deb` is the one that
# matters here: `apt install ./x.deb` resolves its WebKitGTK dependencies, where an
# AppImage would need FUSE and a hand-written list of the same libraries.
deb_url=$(curl -fsSL "https://api.github.com/repos/$REFERENCE_REPO/releases/tags/$REFERENCE_TAG" \
  | grep -o '"browser_download_url": *"[^"]*\.deb"' \
  | sed 's/.*"\(https[^"]*\)"/\1/' \
  | head -1)
if [ -z "$deb_url" ]; then
  printf 'no .deb asset on %s of %s\n' "$REFERENCE_TAG" "$REFERENCE_REPO" >&2
  exit 1
fi
deb_name=$(basename "$deb_url")
curl -fL --retry 2 -o "$deb_name" "$deb_url"
as_root apt-get install -y "./$deb_name"

say "what is here now"
for tool in cargo rustc ffmpeg xdotool Xvfb convert openbox; do
  where=$(command -v "$tool" || true)
  printf '  %-9s %s\n' "$tool" "${where:-MISSING}"
done
cargo --version
rustc --version
ffmpeg -version | head -1
# WebKitGTK 4.1 is what a Tauri 2 build links, and the reference is one: without
# it the package installs and the window never appears.
if ldconfig -p | grep -q webkit2gtk-4.1; then
  printf '  %-9s %s\n' webkit2gtk "$(ldconfig -p | grep webkit2gtk-4.1 | head -1 | awk '{print $1}')"
else
  printf '  %-9s %s\n' webkit2gtk "MISSING -- the reference's window will not appear"
fi
if command -v modrinth-app >/dev/null 2>&1; then
  printf '  %-9s %s\n' reference "$(command -v modrinth-app)"
else
  printf '  %-9s %s\n' reference "not on PATH -- \`dpkg -L\` the package to find its binary"
fi

cat <<'NEXT'

Next, on this machine:

  git clone https://github.com/msedge-projects/PalantirMC.git
  cd PalantirMC
  CARGO_BUILD_JOBS=1 cargo test -p palantir-desktop --offline --locked
  cargo build --release -p palantir-desktop

  Xvfb :99 -screen 0 1920x1080x24 &
  export DISPLAY=:99
  python tools/xshot.py --launch target/release/PalantirMC --client 1280x720 --out ours.png

  export WEBKIT_DISABLE_DMABUF_RENDERER=1     # blank window without it, on software GL
  modrinth-app &
  python tools/xshot.py --title "Modrinth App" --client 1280x720 --out reference.png

`TRANSFER.md` is the rest of it: what travels, what changes about the tooling, and
the first four jobs in the order they matter.
NEXT
