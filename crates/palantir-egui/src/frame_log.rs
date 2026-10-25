//! Frame timing, in the same shape the iced shell writes it, so the two stacks can
//! be compared on one machine.
//!
//! Off unless `PALANTIRMC_FRAME_LOG` names a file. Every [`FLUSH_EVERY`] frames it
//! appends one line: the interval between frames and the time spent in this frame's
//! body, each as mean/p50/p95/max in milliseconds. `crates/palantir-desktop`'s
//! `frame_log` writes the identical format, which is what makes "egui is smoother"
//! a number rather than an impression.

use std::io::Write;
use std::path::PathBuf;

use std::time::{Duration, Instant};

/// How many frames one summary covers: half a second at 60 fps.
pub const FLUSH_EVERY: usize = 30;

/// The environment variable that turns this on, naming the file to append to.
pub const PATH_ENV: &str = "PALANTIRMC_FRAME_LOG";

/// One frame's two durations.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sample {
    /// How long the frame's body took.
    pub body: Duration,
    /// How long it was since the previous frame began.
    pub interval: Duration,
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

/// The summary line for `samples`, in the format the iced shell writes.
pub fn summary(samples: &[Sample]) -> Option<String> {
    if samples.is_empty() {
        return None;
    }
    let interval = stats(samples.iter().map(|sample| sample.interval));
    let body = stats(samples.iter().map(|sample| sample.body));
    Some(format!(
        "frames={} interval_ms mean={:.2} p50={:.2} p95={:.2} max={:.2} | \
         view_ms mean={:.2} p95={:.2} max={:.2}",
        samples.len(),
        interval.0,
        interval.1,
        interval.2,
        interval.3,
        body.0,
        body.2,
        body.3,
    ))
}

/// The recorder: the file, the window of samples, and the last frame's start.
pub struct FrameLog {
    path: Option<PathBuf>,
    window: Vec<Sample>,
    last: Option<Instant>,
    began: Option<Instant>,
}

impl Default for FrameLog {
    fn default() -> FrameLog {
        let path = std::env::var_os(PATH_ENV).map(PathBuf::from);
        if let Some(path) = &path {
            append(path, "opened");
        }
        FrameLog {
            path,
            window: Vec::new(),
            last: None,
            began: None,
        }
    }
}

impl FrameLog {
    /// Mark the start of a frame.
    pub fn begin(&mut self) {
        let now = Instant::now();
        self.began = Some(now);
        if self.last.is_none() {
            self.last = Some(now);
        }
    }

    /// Mark the end of a frame, and write a summary every [`FLUSH_EVERY`].
    pub fn end(&mut self) {
        let Some(began) = self.began.take() else {
            return;
        };
        let now = Instant::now();
        let interval = self
            .last
            .map(|last| began.saturating_duration_since(last))
            .unwrap_or_default();
        self.last = Some(began);
        self.window.push(Sample {
            body: now.saturating_duration_since(began),
            interval,
        });
        if self.window.len() >= FLUSH_EVERY {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if let (Some(path), Some(line)) = (&self.path, summary(&self.window)) {
            append(path, &line);
        }
        self.window.clear();
    }
}

impl Drop for FrameLog {
    fn drop(&mut self) {
        self.flush();
    }
}

/// Append one line, ignoring a failure: a measurement must not take the app with it.
fn append(path: &std::path::Path, line: &str) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(body_ms: f32, interval_ms: f32) -> Sample {
        Sample {
            body: Duration::from_secs_f32(body_ms / 1000.0),
            interval: Duration::from_secs_f32(interval_ms / 1000.0),
        }
    }

    /// The line is the same shape the iced shell writes, which is the point: two
    /// stacks, one instrument.
    #[test]
    fn the_summary_matches_the_iced_format() {
        let samples: Vec<Sample> = (0..40).map(|_| sample(0.4, 16.0)).collect();
        let line = summary(&samples).expect("a summary");
        assert!(line.starts_with("frames=40 interval_ms mean=16.00 p50=16.00"), "{line}");
        assert!(line.contains("view_ms mean=0.40"), "{line}");
    }

    #[test]
    fn an_empty_window_has_no_line() {
        assert!(summary(&[]).is_none());
    }
}
