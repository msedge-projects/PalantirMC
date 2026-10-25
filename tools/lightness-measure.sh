#!/bin/bash
# Measure the three budgets a machine can answer for the egui build: startup to first window,
# idle RSS after five seconds, and idle CPU over a ten-second window.
#
# The binary is an argument because the answer differs by build: the debug binary is what every
# capture is taken from, and the release one is what ships. Measuring the wrong one silently is
# how a budget appears to pass while the shipped artifact was never checked.
set -u
export DISPLAY=:99
binary="${1:-/data/workspace/PalantirMC/crates/sift-ui/target/release/SiftUi}"
name=$(basename "$binary")

pkill -x SiftUi 2>/dev/null
sleep 1

start=$(date +%s%N)
setsid "$binary" > /tmp/measure.log 2>&1 < /dev/null &
pid=$!

# Startup: poll for the window rather than assuming a sleep.
window=""
for _ in $(seq 1 300); do
  window=$(xdotool search --name "Sift Launcher" 2>/dev/null | head -1)
  if [ -n "$window" ]; then break; fi
  sleep 0.02
done
mapped=$(date +%s%N)
echo "binary=$name"
echo "startup_ms=$(( (mapped - start) / 1000000 )) window=${window:-none}"

# Idle: five seconds to settle, then RSS and a CPU delta over ten seconds.
sleep 5
rss_kb=$(awk '/VmRSS/ {print $2}' /proc/$pid/status 2>/dev/null)
echo "idle_rss_mb=$(awk -v kb="${rss_kb:-0}" 'BEGIN {printf "%.1f", kb/1024}')"

cpu_start=$(awk '{print $14+$15}' /proc/$pid/stat 2>/dev/null)
sleep 10
cpu_end=$(awk '{print $14+$15}' /proc/$pid/stat 2>/dev/null)
ticks=$(( ${cpu_end:-0} - ${cpu_start:-0} ))
echo "idle_cpu_ticks_over_10s=$ticks  (0 means the window drew nothing while idle)"

stat -c 'binary_bytes=%s' "$binary"
