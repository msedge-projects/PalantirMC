#!/bin/bash
# Measure the three budgets a machine can answer for the egui build: startup to first window,
# idle RSS after five seconds, and idle CPU over a ten-second window.
set -u
export DISPLAY=:99
pkill -x SiftUi 2>/dev/null
sleep 1

start=$(date +%s%N)
setsid /data/workspace/PalantirMC/crates/sift-ui/target/debug/SiftUi > /tmp/measure.log 2>&1 < /dev/null &
pid=$!

# Startup: poll for the window rather than assuming a sleep.
window=""
for _ in $(seq 1 200); do
  window=$(xdotool search --name "Sift Launcher" 2>/dev/null | head -1)
  if [ -n "$window" ]; then break; fi
  sleep 0.02
done
mapped=$(date +%s%N)
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

# What the app is: size on disk and the release-profile question for later.
stat -c 'binary_bytes=%s' /data/workspace/PalantirMC/crates/sift-ui/target/debug/SiftUi
