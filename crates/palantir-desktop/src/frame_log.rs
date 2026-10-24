//! What a frame costs, written down only when somebody asks for it.
//!
//! Three attempts at the scrolling were tuned by reasoning, and the reasoning kept
//! being wrong because nobody had the number the reader was feeling: **the interval
//! between frames, on their machine, while scrolling**. This module is that number.
//!
//! It is off unless `PALANTIRMC_FRAME_LOG` names a file. When it does, every frame
//! records two durations -- how long this shell spent building the view, and how long
//! it was since the previous frame -- and every [`FLUSH_EVERY`] frames appends one
//! line of summary to that file. A summary rather than a line per frame: writing to
//! a disk from inside `view` would be a per-frame I/O cost in the very code being
//! measured.
//!
//! **What the two numbers tell apart.** A frame interval far above 16.7 ms with a
//! *small* view time means the cost is outside this crate -- iced's layout, its draw
//! command encoding, the rasteriser or the presentation -- and the lever is the
//! renderer, not the widgets. A large view time means the page being drawn is the
//! cost, and the lever is that page. Without this split every fix is a guess, which
//! is what three scrolling revisions were.

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// How many frames one summary covers.
///
/// Sixty is about a second at the rate a smooth window runs, so a scrolling session
/// produces a line per second and a two-second scroll produces two or three.
pub const FLUSH_EVERY: usize = 60;

/// The environment variable that turns this on, naming the file to append to.
pub const PATH_ENV: &str = "PALANTIRMC_FRAME_LOG";

/// One frame's two durations.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sample {
    /// How long the shell spent building this frame's view.
    pub view: Duration,
    /// How long it was since the previous frame's view.
    pub interval: Duration,
}

/// A window of samples, summarised.
#[derive(Debug, Default)]
pub struct Window {
    samples: Vec<Sample>,
}

impl Window {
    /// Record one frame.
    pub fn push(&mut self, sample: Sample) {
        self.samples.push(sample);
    }

    /// Whether there are enough samples to be worth writing.
    pub fn full(&self) -> bool {
        self.samples.len() >= FLUSH_EVERY
    }

    /// Take the samples, leaving the window empty.
    pub fn take(&mut self) -> Vec<Sample> {
        std::mem::take(&mut self.samples)
    }

    /// The summary line for `samples`, or `None` when there are none.
    ///
    /// Means and 95th percentiles rather than a list: what a stutter looks like in
    /// aggregate is a p95 well above the mean, and what a slow machine looks like is
    /// a mean well above 16.7.
    pub fn summary(samples: &[Sample]) -> Option<String> {
        if samples.is_empty() {
            return None;
        }
        let interval = stats(samples.iter().map(|sample| sample.interval));
        let view = stats(samples.iter().map(|sample| sample.view));
        Some(format!(
            "frames={} interval_ms mean={:.2} p50={:.2} p95={:.2} max={:.2} | \
             view_ms mean={:.2} p95={:.2} max={:.2}",
            samples.len(),
            interval.0,
            interval.1,
            interval.2,
            interval.3,
            view.0,
            view.2,
            view.3,
        ))
    }
}

/// `(mean, p50, p95, max)` in milliseconds.
fn stats(durations: impl Iterator<Item = Duration>) -> (f32, f32, f32, f32) {
    let mut ms: Vec<f32> = durations.map(|d| d.as_secs_f32() * 1000.0).collect();
    if ms.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mean = ms.iter().sum::<f32>() / ms.len() as f32;
    let at = |fraction: f32| {
        let index = ((ms.len() as f32 - 1.0) * fraction).round() as usize;
        ms[index.min(ms.len() - 1)]
    };
    (mean, at(0.5), at(0.95), *ms.last().unwrap_or(&0.0))
}

/// The process-wide recorder, built once from the environment.
fn log() -> Option<&'static std::sync::Mutex<Recorder>> {
    static RECORDER: OnceLock<Option<std::sync::Mutex<Recorder>>> = OnceLock::new();
    RECORDER
        .get_or_init(|| {
            let path = std::env::var_os(PATH_ENV).map(PathBuf::from)?;
            Some(std::sync::Mutex::new(Recorder::new(path)))
        })
        .as_ref()
}

/// The file, the window, and the clock the intervals are measured against.
struct Recorder {
    path: PathBuf,
    window: Window,
    last: Instant,
}

impl Recorder {
    fn new(path: PathBuf) -> Recorder {
        Recorder {
            path,
            window: Window::default(),
            last: Instant::now(),
        }
    }

    /// Record one finished frame, and write a summary when the window fills.
    fn record(&mut self, sample: Sample) {
        self.window.push(sample);
        if !self.window.full() {
            return;
        }
        let samples = self.window.take();
        let Some(line) = Window::summary(&samples) else {
            return;
        };
        // Append rather than truncate: a session that restarts the window keeps the
        // earlier evidence, and the file is a scratch file by convention.
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = writeln!(file, "{line}");
        }
    }
}

/// One frame being measured: built by [`begin`], recorded when it is dropped.
pub struct Frame {
    began: Instant,
    previous: Option<Instant>,
}

impl Drop for Frame {
    fn drop(&mut self) {
        let Some(log) = log() else {
            return;
        };
        let now = Instant::now();
        let view = now.saturating_duration_since(self.began);
        if let Ok(mut recorder) = log.lock() {
            let interval = self
                .previous
                .map(|previous| self.began.saturating_duration_since(previous))
                .unwrap_or_default();
            recorder.last = now;
            recorder.record(Sample { view, interval });
        }
    }
}

/// Start measuring a frame. The returned value records it when dropped.
pub fn begin() -> Frame {
    // The previous frame's start, read before this one's, so the interval is the
    // gap the reader experiences rather than this frame's own build time.
    let previous = log().and_then(|log| log.lock().ok().map(|recorder| recorder.last));
    Frame {
        began: Instant::now(),
        previous,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(view_ms: f32, interval_ms: f32) -> Sample {
        Sample {
            view: Duration::from_secs_f32(view_ms / 1000.0),
            interval: Duration::from_secs_f32(interval_ms / 1000.0),
        }
    }

    /// The summary is what makes a stutter and a slow machine tell themselves apart.
    #[test]
    fn a_summary_says_what_the_mean_and_the_tail_were() {
        let samples: Vec<Sample> = (0..100).map(|_| sample(1.0, 16.0)).collect();
        let line = Window::summary(&samples).expect("a summary");
        assert!(line.contains("frames=100"), "{line}");
        assert!(line.contains("interval_ms mean=16.00"), "{line}");
        assert!(line.contains("view_ms mean=1.00"), "{line}");

        // One frame in twenty at four times the interval is a stutter: the mean
        // barely moves and the p95 does.
        let mut janky: Vec<Sample> = (0..100).map(|_| sample(1.0, 16.0)).collect();
        for index in (0..100).step_by(5) {
            janky[index] = sample(1.0, 64.0);
        }
        let line = Window::summary(&janky).expect("a summary");
        assert!(line.contains("p95=64.00"), "the tail is visible: {line}");
        assert!(line.contains("max=64.00"), "{line}");
    }

    #[test]
    fn an_empty_window_has_no_line_and_a_full_one_is_flushed() {
        assert!(Window::summary(&[]).is_none());
        let mut window = Window::default();
        for _ in 0..(FLUSH_EVERY - 1) {
            window.push(sample(1.0, 16.0));
        }
        assert!(!window.full(), "one short of a window");
        window.push(sample(1.0, 16.0));
        assert!(window.full(), "and then it is full");
        assert_eq!(window.take().len(), FLUSH_EVERY);
        assert!(!window.full(), "taking the window empties it");
    }

    /// Off means off: with the variable unset nothing is built and nothing is
    /// written, which is what keeps the shipped launcher free of this.
    #[test]
    fn nothing_is_recorded_unless_the_variable_names_a_file() {
        if std::env::var_os(PATH_ENV).is_some() {
            // A machine running the measurement is not the place to assert its
            // absence; the guard is the variable itself.
            return;
        }
        assert!(log().is_none(), "a launcher without the variable keeps no log");
    }
}
