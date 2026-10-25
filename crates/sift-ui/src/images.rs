//! Project icons, fetched off the frame thread and cached as textures.
//!
//! The reference's cards carry a project's icon at 64 points. Drawing that from a URL means
//! three separate costs -- the network, the decode, and the texture upload -- and the one
//! rule that matters is that **none of them happen on the frame thread**. So a request is a
//! thread, the result arrives over a channel, and the frame that finds it ready uploads once
//! and keeps the `TextureHandle` for ever after.
//!
//! Three states, keyed by URL: `Loading` (a worker is out, and the caller keeps asking for
//! frames so the upload happens promptly), `Ready` (a texture), `Failed` (no retry this run
//! -- a missing icon is a grey tile, not a loop of requests).
//!
//! The fetch goes through `palantir_net::download_file`, the launcher's own client, so the
//! timeout, the user agent and the proxy settings are the ones the rest of the app uses
//! rather than a second opinion.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use eframe::egui::{self, ColorImage, TextureHandle};

/// What the cache knows about one URL.
enum Entry {
    /// A worker is fetching it.
    Loading,
    /// The texture, uploaded.
    Ready(TextureHandle),
    /// The fetch or the decode failed. Not retried this run.
    Failed,
}

/// The icon cache: one entry per URL, and the channel its workers answer on.
pub struct Images {
    entries: HashMap<String, Entry>,
    sender: Sender<(String, Result<ColorImage, String>)>,
    receiver: Receiver<(String, Result<ColorImage, String>)>,
    /// Requests still out, so the caller can keep frames coming while they are.
    inflight: usize,
}

impl Default for Images {
    fn default() -> Images {
        let (sender, receiver) = channel();
        Images {
            entries: HashMap::new(),
            sender,
            receiver,
            inflight: 0,
        }
    }
}

impl Images {
    /// The icon for `url`, if it is ready. Starts the fetch the first time it is asked.
    ///
    /// `None` while loading and after a failure, deliberately: a card does not care *why*
    /// it has no picture, only that it has none, and both cases draw the same placeholder.
    pub fn get(&mut self, ctx: &egui::Context, url: &str) -> Option<TextureHandle> {
        self.pump(ctx);

        match self.entries.get(url) {
            Some(Entry::Ready(texture)) => return Some(texture.clone()),
            Some(Entry::Loading | Entry::Failed) => return None,
            None => {}
        }

        let url_owned = url.to_owned();
        let reply = url_owned.clone();
        let sender = self.sender.clone();
        let spawned = std::thread::Builder::new()
            .name("sift-icon".to_owned())
            .spawn(move || {
                let image = fetch(&url_owned);
                // A closed receiver means the window is gone; dropping the result is right.
                let _ = sender.send((reply, image));
            });
        if spawned.is_ok() {
            self.inflight += 1;
            self.entries.insert(url.to_owned(), Entry::Loading);
            // Keep the frame loop alive while the fetch is out, or the reply would sit in
            // the channel until something else happened to ask for a repaint.
            ctx.request_repaint();
        } else {
            self.entries.insert(url.to_owned(), Entry::Failed);
        }
        None
    }

    /// Whether any fetch is still outstanding: the caller uses it the way it uses a running
    /// animation, to know that the frame it just drew is not the last one.
    pub fn loading(&self) -> bool {
        self.inflight > 0
    }

    /// Take whatever the workers have finished and upload it.
    fn pump(&mut self, ctx: &egui::Context) {
        let mut finished = Vec::new();
        while let Ok((url, result)) = self.receiver.try_recv() {
            finished.push((url, result));
        }
        for (url, result) in finished {
            self.inflight = self.inflight.saturating_sub(1);
            match result {
                Ok(image) => {
                    let texture = ctx.load_texture(
                        format!("icon:{url}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    );
                    self.entries.insert(url, Entry::Ready(texture));
                }
                Err(_) => {
                    self.entries.insert(url, Entry::Failed);
                }
            }
        }
        if self.inflight > 0 {
            ctx.request_repaint();
        }
    }
}

/// Find and decode one icon. Runs on a worker thread.
fn fetch(url: &str) -> Result<ColorImage, String> {
    // A temporary file rather than an in-memory read: `download_file` streams to disk, which
    // is the launcher's own client and keeps a large icon off this thread's stack. The file
    // is removed whatever happens.
    let path = std::env::temp_dir().join(format!("sift-icon-{:016x}.bin", hash(url)));
    let result = palantir_net::download_file(url, &path, Duration::from_secs(20))
        .map_err(|error| error.to_string());
    let bytes = match result {
        Ok(()) => std::fs::read(&path).map_err(|error| error.to_string()),
        Err(error) => Err(error),
    };
    let _ = std::fs::remove_file(&path);

    let bytes = bytes?;
    let decoded = image::load_from_memory(&bytes).map_err(|error| error.to_string())?;
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

/// A stable name for a URL, used for the temporary file and the texture.
fn hash(url: &str) -> u64 {
    // FNV-1a: this is a cache key, not a security boundary, and a dependency for one 8-byte
    // value would be a dependency for nothing.
    let mut value: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in url.as_bytes() {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x0000_0100_0000_01b3);
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two different URLs must not share a temporary file, which is the only thing this hash
    /// is for.
    #[test]
    fn the_cache_key_separates_the_urls_a_launcher_sees() {
        let a = hash("https://cdn.modrinth.com/data/1KVo5zza/icon.png");
        let b = hash("https://cdn.modrinth.com/data/1KVo5zzb/icon.png");
        assert_ne!(a, b);
        assert_eq!(a, hash("https://cdn.modrinth.com/data/1KVo5zza/icon.png"));
    }

    /// A cache starts empty and idle: no fetch is out until something asks for a picture.
    #[test]
    fn a_fresh_cache_is_idle() {
        let images = Images::default();
        assert!(!images.loading());
    }
}
